use embassy_time::Instant;
use rmk::{
    event::{KeyPos, KeyboardEventPos},
    types::action::Action,
};
use rmk_palettefx::{
    color::{self, Hsv, Rgb},
    effects::FrameParams,
    layout::LedLayout,
};
use sequential_storage::{
    cache::{Cache as CacheIn, Uncached},
    map::{MapConfig, MapStorage, PostcardValue},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Layout, Led, animation::Effect, palette};

const DRIVERS: usize = if cfg!(feature = "halfmoon") { 1 } else { 2 };
const LEDS_PER_DRIVER: usize = super::LED_COUNT / DRIVERS;

const STORAGE_START_OFFSET: u32 = 0x1F000;
const STORAGE_SIZE: u32 = 2 * 2 * 1024;
const KEY_RGB_STATE: u8 = 0x01;

const LAYOUT: Layout = Layout;

type Cache = CacheIn<Uncached, Uncached, Uncached, u8>;
type Map = MapStorage<u8, crate::keyboard::StorageFlash, Cache>;

pub(super) struct Storage {
    inner: Map,
    buffer: [u8; 16],
}

impl Storage {
    pub(super) fn new(flash: crate::keyboard::StorageFlash) -> Self {
        Self {
            buffer: [0; 16],
            inner: Map::new(
                flash,
                MapConfig::new(STORAGE_START_OFFSET..(STORAGE_START_OFFSET + STORAGE_SIZE)),
                Cache::new_uncached(),
            ),
        }
    }

    #[inline]
    pub(super) async fn load(&mut self) -> Option<State> {
        self.inner
            .fetch_item(&mut self.buffer, &KEY_RGB_STATE)
            .await
            .inspect_err(|_err| debug!("Could not load RGB state: {:?}", _err))
            .ok()
            .flatten()
    }

    #[inline]
    pub(super) async fn try_store(&mut self, state: &State) {
        _ = self
            .inner
            .store_item(&mut self.buffer, &KEY_RGB_STATE, state)
            .await
            .inspect_err(|_err| debug!("Could not store RGB state: {:?}", _err))
    }
}

#[derive(Serialize, Deserialize)]
struct SerializableState {
    enabled: bool,
    effect_idx: u8,
    palette_id: u8,
    speed: u8,
    saturation: u8,
    brightness: u8,
}

pub(super) struct State {
    pub(super) enabled: bool,
    pub(super) effect: Effect,
    pub(super) palette_id: usize,
    pub(super) speed: u8,
    pub(super) saturation: u8,
    pub(super) brightness: u8,
    pub(super) buff: Buff,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: false,
            effect: Effect::sparkle(),
            palette_id: 0,
            speed: 128,
            saturation: u8::MAX,
            brightness: u8::MAX,
            buff: Buff::new(),
        }
    }
}

impl PostcardValue<'_> for State {}

impl<'de> Deserialize<'de> for State {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = SerializableState::deserialize(deserializer)?;
        let effect = match data.effect_idx {
            0 => Effect::Gradient,
            1 => Effect::plain(),
            2 => Effect::flow(),
            3 => Effect::vortex(),
            4 => Effect::sparkle(),
            5 => Effect::ripple(),
            _ => Effect::reactive(),
        };

        let palette_id = (data.palette_id as usize).clamp(0, palette::COLORS.len() - 1);
        Ok(State {
            enabled: data.enabled,
            effect,
            palette_id,
            speed: data.speed,
            saturation: data.saturation,
            brightness: data.brightness,
            buff: Buff::new(),
        })
    }
}

impl Serialize for State {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let effect_idx = match &self.effect {
            Effect::Gradient => 0,
            Effect::Plain(_) => 1,
            Effect::Flow(_) => 2,
            Effect::Vortex(_) => 3,
            Effect::Sparkle(_) => 4,
            Effect::Ripple(_, _) => 5,
            Effect::Reactive(_) => 6,
        };

        let data = SerializableState {
            enabled: self.enabled,
            effect_idx,
            palette_id: self.palette_id as _,
            speed: self.speed,
            saturation: self.saturation,
            brightness: self.brightness,
        };

        data.serialize(serializer)
    }
}

impl State {
    const CTRL_STEP: u8 = 17;

    pub(super) const fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    #[inline]
    pub(super) fn inc_brightness(&mut self) {
        self.brightness = self.brightness.saturating_add(Self::CTRL_STEP);
    }

    pub(super) const fn dec_brightness(&mut self) {
        self.brightness = self.brightness.saturating_sub(Self::CTRL_STEP);
    }

    #[inline]
    pub(super) fn inc_saturation(&mut self) {
        self.saturation = self.saturation.saturating_add(Self::CTRL_STEP);
    }

    pub(super) const fn dec_saturation(&mut self) {
        self.saturation = self.saturation.saturating_sub(Self::CTRL_STEP);
    }

    #[inline]
    pub(super) fn inc_speed(&mut self) {
        self.speed = self.speed.saturating_add(Self::CTRL_STEP);
    }

    pub(super) const fn dec_speed(&mut self) {
        self.speed = self.speed.saturating_sub(Self::CTRL_STEP);
    }

    pub(super) const fn prev_palette(&mut self) {
        self.palette_id = (self.palette_id + 1) % palette::COLORS.len();
    }

    pub(super) const fn next_palette(&mut self) {
        self.palette_id = if self.palette_id == 0 {
            palette::COLORS.len() - 1
        } else {
            self.palette_id - 1
        };
    }

    pub(super) fn prev_effect(&mut self) {
        self.effect = match &self.effect {
            Effect::Gradient => Effect::reactive(),
            Effect::Plain(_) => Effect::Gradient,
            Effect::Flow(_) => Effect::plain(),
            Effect::Vortex(_) => Effect::flow(),
            Effect::Sparkle(_) => Effect::vortex(),
            Effect::Ripple(_, _) => Effect::sparkle(),
            Effect::Reactive(_) => Effect::ripple(),
        };
    }

    pub(super) fn next_effect(&mut self) {
        self.effect = match &self.effect {
            Effect::Gradient => Effect::plain(),
            Effect::Plain(_) => Effect::flow(),
            Effect::Flow(_) => Effect::vortex(),
            Effect::Vortex(_) => Effect::sparkle(),
            Effect::Sparkle(_) => Effect::ripple(),
            Effect::Ripple(_, _) => Effect::reactive(),
            Effect::Reactive(_) => Effect::Gradient,
        };
    }

    pub(super) fn try_record_hit(&mut self, action: &Action, position: KeyboardEventPos) {
        let Effect::Reactive(ref mut state) = self.effect else {
            return;
        };

        if matches!(action, Action::Key(_) | Action::KeyWithModifier(_, _))
            && let KeyboardEventPos::Key(KeyPos { row, col }) = position
            && let Some(led) = super::key_to_led(row, col)
        {
            let (x, y) = LAYOUT.position(led as _);
            state.record_hit(x, y, Instant::now().as_millis() as _);
        }
    }

    pub(super) fn tick(&mut self, params: FrameParams<'_>, frame: &mut [Hsv]) {
        if !self.enabled {
            if let Effect::Plain(plain_state) = &mut self.effect {
                plain_state.dirty(true);
            }
            self.buff.write_all(0, 0, 0);
            return;
        }

        match &mut self.effect {
            Effect::Gradient => rmk_palettefx::effects::gradient(&LAYOUT, params, frame),
            Effect::Flow(flow_state) => flow_state.tick(&LAYOUT, params, frame),
            Effect::Vortex(vortex_state) => vortex_state.tick(&LAYOUT, params, frame),
            Effect::Sparkle(sparkle_state) => sparkle_state.tick(&LAYOUT, params, frame),
            Effect::Reactive(reactive_state) => reactive_state.tick(&LAYOUT, params, frame),
            Effect::Ripple(ripple_state, lcg) => {
                ripple_state.tick(&LAYOUT, params, || lcg.next_u8(), frame)
            }
            Effect::Plain(plain_state) => {
                if let Some(Rgb { r, g, b }) = plain_state.tick(params) {
                    self.buff.write_all(r, g, b);
                }
                return;
            }
        }
        self.buff.write_hsv(frame);
    }

    pub(super) fn as_frame_params(&self) -> FrameParams<'static> {
        let curr_palette = unsafe { palette::COLORS.get_unchecked(self.palette_id) };
        FrameParams {
            palette: curr_palette,
            speed: self.speed,
            sat: self.saturation,
            val: self.brightness,
            timer_ms: Instant::now().as_millis() as _,
        }
    }
}

pub(super) struct Buff {
    pub(super) pwm: [u8; super::PWM_BYTES * DRIVERS],
    pub(super) dirty: u8,
}

impl Buff {
    fn new() -> Self {
        Self {
            pwm: [0; super::PWM_BYTES * DRIVERS],
            dirty: 0b00,
        }
    }

    #[inline]
    fn write_all(&mut self, r: u8, g: u8, b: u8) {
        for (idx, led) in super::LED_TABLE.iter().enumerate() {
            self.write_to(idx, led, r, g, b);
        }
    }

    fn write_hsv(&mut self, frame: &[Hsv]) {
        for (idx, led) in super::LED_TABLE.iter().enumerate() {
            // SAFETY: Always in bounds
            let hsv = *unsafe { frame.get_unchecked(idx) };
            let Rgb { r, g, b } = color::hsv_to_rgb(hsv);
            self.write_to(idx, led, r, g, b);
        }
    }

    #[inline]
    fn write_to(&mut self, idx: usize, led: &Led, r: u8, g: u8, b: u8) {
        let driver = idx / LEDS_PER_DRIVER;
        let base = driver * super::PWM_BYTES;
        for (reg, value) in [(led.r, r), (led.g, g), (led.b, b)] {
            let slot = &mut self.pwm[base + reg as usize];
            if *slot != value {
                *slot = value;
                self.dirty |= 1 << driver;
            }
        }
    }
}
