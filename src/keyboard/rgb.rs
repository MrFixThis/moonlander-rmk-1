mod palette;

use embassy_futures::select;
use embassy_time::{Duration, Instant, Ticker, Timer};
use rmk::{
    core_traits::Runnable,
    event::{ActionEvent, KeyPos, KeyboardEvent, KeyboardEventPos, SubscribableEvent},
    input_device::InputDevice,
    types::action::{Action, LightAction},
};
use rmk_palettefx::{
    color::{self, Hsv, Rgb},
    effects::{FlowState, FrameParams, Lcg, ReactiveState, RippleState, SparkleState, VortexState},
    layout::LedLayout,
};
use sequential_storage::{
    cache::{Cache as CacheIn, Uncached},
    map::{MapConfig, MapStorage, PostcardValue},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::keyboard::i2c::SharedI2c;

const REG_COMMAND: u8 = 0xFD;
const CMD_FRAME_1: u8 = 0x00;
const CMD_FUNCTION: u8 = 0x0B;

const CONFIG_MODE_PICTURE: u8 = 0x00;

const FN_REG_CONFIG: u8 = 0x00;
const FN_REG_SHUTDOWN: u8 = 0x0A;
const FN_REG_PICTURE_DISPLAY: u8 = 0x01;
const FN_REG_AUDIO_SYNC: u8 = 0x06;

const FRAME_REG_LED_CTRL: u8 = 0x00;
const FRAME_REG_BLINK_CTRL: u8 = 0x12;
const FRAME_REG_PWM: u8 = 0x24;

const LED_CTRL_COUNT: usize = 18;
const PWM_CHUNKS: usize = 9;
const PWM_CHUNK_BYTES: usize = 16;
const PWM_BYTES: usize = 144;

const LEFT_DRIVER_ADDR: u8 = 0x74;
#[cfg(not(feature = "halfmoon"))]
const RIGHT_DRIVER_ADDR: u8 = 0x77;
const LED_COUNT: usize = ltable!(__@keys);
const DRIVERS: usize = if cfg!(feature = "halfmoon") { 1 } else { 2 };
const LEDS_PER_DRIVER: usize = LED_COUNT / DRIVERS;

const NO_LED: u8 = 0xFF;
const EFFECT_TICK_MS: u64 = 80;
const REACTIVE_HITS: usize = 8;
const STORAGE_START_OFFSET: u32 = 0x1F000;
const STORAGE_SIZE: u32 = 2 * 2 * 1024;
const KEY_RGB_STATE: u8 = 0x01;

const LAYOUT: Layout = Layout;

type ActionSubscriber = <ActionEvent as SubscribableEvent>::Subscriber;

type Cache = CacheIn<Uncached, Uncached, Uncached, u8>;
type Map = MapStorage<u8, super::StorageFlash, Cache>;

pub struct RgbController {
    i2c: &'static SharedI2c,
    action_sub: ActionSubscriber,
    state: State,
    storage: Storage,
}

impl RgbController {
    pub fn new(i2c: &'static SharedI2c, flash: super::StorageFlash) -> Self {
        Self {
            i2c,
            action_sub: ActionEvent::subscriber(),
            state: State::default(),
            storage: Storage::new(flash),
        }
    }

    async fn handle_action(&mut self, action: LightAction) {
        match action {
            LightAction::RgbTog => self.state.toggle(),
            LightAction::RgbModeReverse => self.state.prev_effect(),
            LightAction::RgbModeForward => self.state.next_effect(),
            LightAction::RgbHui => self.state.next_palette(),
            LightAction::RgbHud => self.state.prev_palette(),
            LightAction::RgbVai => self.state.inc_brightness(),
            LightAction::RgbVad => self.state.dec_brightness(),
            LightAction::RgbSai => self.state.inc_saturation(),
            LightAction::RgbSad => self.state.dec_saturation(),
            LightAction::RgbSpi => self.state.inc_speed(),
            LightAction::RgbSpd => self.state.dec_speed(),
            _other => {
                warn!("[RGB]: Unsupported action {:?}", _other);
                return;
            }
        }
        self.storage.try_store(&self.state).await;
    }

    async fn configure_driver(&mut self, addr: u8) -> bool {
        // Shutdown
        for cmd in &[[REG_COMMAND, CMD_FUNCTION], [FN_REG_SHUTDOWN, 0x00]] {
            if !self.i2c.lock().await.write(addr, cmd).await {
                return false;
            }
        }

        Timer::after_millis(10).await;

        // Enable picture mode and frame 1
        for cmd in &[
            [FN_REG_CONFIG, CONFIG_MODE_PICTURE],
            [FN_REG_PICTURE_DISPLAY, 0x00],
            [FN_REG_AUDIO_SYNC, 0x00],
            [REG_COMMAND, CMD_FRAME_1],
        ] {
            if !self.i2c.lock().await.write(addr, cmd).await {
                return false;
            }
        }

        // Every LED enabled, blink off, brightness zero.
        {
            let mut ctrl = [0xFFu8; LED_CTRL_COUNT + 1];
            ctrl[0] = FRAME_REG_LED_CTRL;
            let mut blink = [0u8; LED_CTRL_COUNT + 1];
            blink[0] = FRAME_REG_BLINK_CTRL;

            for cmd in &[ctrl, blink] {
                if !self.i2c.lock().await.write(addr, cmd).await {
                    return false;
                }
            }

            let mut pwm = [0u8; PWM_CHUNK_BYTES + 1];
            for chunk in 0..PWM_CHUNKS {
                pwm[0] = FRAME_REG_PWM + chunk as u8 * (PWM_CHUNK_BYTES as u8);
                if !self.i2c.lock().await.write(addr, &pwm).await {
                    return false;
                }
            }
        }

        // Wake up and leave frame 1 selected
        for cmd in &[
            [REG_COMMAND, CMD_FUNCTION],
            [FN_REG_SHUTDOWN, 0x01],
            [REG_COMMAND, CMD_FRAME_1],
        ] {
            if !self.i2c.lock().await.write(addr, cmd).await {
                return false;
            }
        }

        true
    }

    async fn flush_state(&mut self) -> bool {
        #![allow(unused_mut)]

        let mut addr = LEFT_DRIVER_ADDR;
        for (idx, buf) in self.state.buff.pwm.chunks(PWM_BYTES).enumerate() {
            // one block per chip
            let dirty_bit = 1 << idx;
            let is_dirty = (self.state.buff.dirty & dirty_bit) != 0;

            if !is_dirty {
                #[cfg(feature = "halfmoon")]
                {
                    break;
                }
                #[cfg(not(feature = "halfmoon"))]
                {
                    addr = RIGHT_DRIVER_ADDR;
                    continue;
                }
            }

            let mut cmd = [0; PWM_CHUNK_BYTES + 1];
            for chunk in 0..PWM_CHUNKS {
                let section = chunk * PWM_CHUNK_BYTES;
                cmd[0] = FRAME_REG_PWM + chunk as u8 * PWM_CHUNK_BYTES as u8;
                cmd[1..].copy_from_slice(&buf[section..(section + PWM_CHUNK_BYTES)]);

                if !self.i2c.lock().await.write(addr, &cmd).await {
                    return false;
                }
                // let the matrix scan use the bus between chunks
                embassy_futures::yield_now().await;
            }

            self.state.buff.dirty &= !dirty_bit;
            #[cfg(not(feature = "halfmoon"))]
            {
                addr = RIGHT_DRIVER_ADDR;
            }
            #[cfg(feature = "halfmoon")]
            {
                break;
            }
        }

        true
    }
}

impl InputDevice for RgbController {
    type Event = Option<LightAction>;

    async fn read_event(&mut self) -> Self::Event {
        let ActionEvent {
            action,
            keyboard_event: KeyboardEvent { pressed: true, pos },
        } = self.action_sub.next_message_pure().await
        else {
            return None;
        };

        self.state.try_record_hit(&action, pos);
        match action {
            Action::Light(light_action) => Some(light_action),
            _ => None,
        }
    }
}

impl Runnable for RgbController {
    async fn run(&mut self) -> ! {
        info!("[Moonlander]: RGB controller initialized");

        if let Some(state) = self.storage.load().await {
            self.state = state;
        }

        let mut frame = [Hsv::default(); LED_COUNT];
        let mut ticker = Ticker::every(Duration::from_millis(EFFECT_TICK_MS));

        #[cfg(not(feature = "halfmoon"))]
        let mut right_driver_init = false;

        // pretty unlikely to fail
        self.configure_driver(LEFT_DRIVER_ADDR).await;
        loop {
            #[cfg(not(feature = "halfmoon"))]
            if !right_driver_init {
                right_driver_init = self.configure_driver(RIGHT_DRIVER_ADDR).await;
                if right_driver_init {
                    self.state.buff.dirty |= 0b10;
                }
            }

            match select::select(ticker.next(), self.read_event()).await {
                select::Either::First(()) => {
                    self.state.tick(self.state.as_frame_params(), &mut frame);
                    if !self.flush_state().await {
                        #[cfg(not(feature = "halfmoon"))]
                        {
                            right_driver_init = false
                        }
                    }
                }
                select::Either::Second(Some(action)) => {
                    debug!("[RGB]: Action read {:?}", action);
                    self.handle_action(action).await;
                }
                _ => {}
            }

            embassy_futures::yield_now().await;
        }
    }
}

struct Storage {
    inner: Map,
    buffer: [u8; 16],
}

impl Storage {
    fn new(flash: super::StorageFlash) -> Self {
        Self {
            buffer: [0; 16],
            inner: Map::new(
                flash,
                MapConfig::new(STORAGE_START_OFFSET..(STORAGE_START_OFFSET + STORAGE_SIZE)),
                Cache::new_uncached(),
            ),
        }
    }

    #[inline(always)]
    async fn load(&mut self) -> Option<State> {
        self.inner
            .fetch_item(&mut self.buffer, &KEY_RGB_STATE)
            .await
            .inspect_err(|_err| debug!("Could not load RGB state: {:?}", _err))
            .ok()
            .flatten()
    }

    #[inline(always)]
    async fn try_store(&mut self, state: &State) {
        _ = self
            .inner
            .store_item(&mut self.buffer, &KEY_RGB_STATE, state)
            .await
            .inspect_err(|_err| debug!("Could not store RGB state: {:?}", _err))
    }
}

struct Buff {
    pwm: [u8; PWM_BYTES * DRIVERS],
    dirty: u8,
}

impl Buff {
    fn new() -> Self {
        Self {
            pwm: [0; PWM_BYTES * DRIVERS],
            dirty: 0b00,
        }
    }

    #[inline]
    fn write_all(&mut self, r: u8, g: u8, b: u8) {
        for (idx, led) in LED_TABLE.iter().enumerate() {
            self.write_to(idx, led, r, g, b);
        }
    }

    fn write_hsv(&mut self, frame: &[Hsv]) {
        for (idx, led) in LED_TABLE.iter().enumerate() {
            // SAFETY: Always in bounds
            let hsv = *unsafe { frame.get_unchecked(idx) };
            let Rgb { r, g, b } = color::hsv_to_rgb(hsv);
            self.write_to(idx, led, r, g, b);
        }
    }

    #[inline]
    fn write_to(&mut self, idx: usize, led: &Led, r: u8, g: u8, b: u8) {
        let driver = idx / LEDS_PER_DRIVER;
        let base = driver * PWM_BYTES;
        for (reg, value) in [(led.r, r), (led.g, g), (led.b, b)] {
            let slot = &mut self.pwm[base + reg as usize];
            if *slot != value {
                *slot = value;
                self.dirty |= 1 << driver;
            }
        }
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

struct State {
    enabled: bool,
    effect: Effect,
    palette_id: usize,
    speed: u8,
    saturation: u8,
    brightness: u8,
    buff: Buff,
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

    const fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    #[inline]
    fn inc_brightness(&mut self) {
        self.brightness = self.brightness.saturating_add(Self::CTRL_STEP);
    }

    const fn dec_brightness(&mut self) {
        self.brightness = self.brightness.saturating_sub(Self::CTRL_STEP);
    }

    #[inline]
    fn inc_saturation(&mut self) {
        self.saturation = self.saturation.saturating_add(Self::CTRL_STEP);
    }

    const fn dec_saturation(&mut self) {
        self.saturation = self.saturation.saturating_sub(Self::CTRL_STEP);
    }

    #[inline]
    fn inc_speed(&mut self) {
        self.speed = self.speed.saturating_add(Self::CTRL_STEP);
    }

    const fn dec_speed(&mut self) {
        self.speed = self.speed.saturating_sub(Self::CTRL_STEP);
    }

    const fn prev_palette(&mut self) {
        self.palette_id = (self.palette_id + 1) % palette::COLORS.len();
    }

    const fn next_palette(&mut self) {
        self.palette_id = if self.palette_id == 0 {
            palette::COLORS.len() - 1
        } else {
            self.palette_id - 1
        };
    }

    fn prev_effect(&mut self) {
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

    fn next_effect(&mut self) {
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

    fn try_record_hit(&mut self, action: &Action, position: KeyboardEventPos) {
        let Effect::Reactive(ref mut state) = self.effect else {
            return;
        };

        if matches!(action, Action::Key(_) | Action::KeyWithModifier(_, _))
            && let KeyboardEventPos::Key(KeyPos { row, col }) = position
            && let Some(led) = key_to_led(row, col)
        {
            let (x, y) = LAYOUT.position(led as _);
            state.record_hit(x, y, Instant::now().as_millis() as _);
        }
    }

    fn tick(&mut self, params: FrameParams<'_>, frame: &mut [Hsv]) {
        if !self.enabled {
            if let Effect::Plain(plain_state) = &mut self.effect {
                plain_state.dirty = true;
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

    fn as_frame_params(&self) -> FrameParams<'static> {
        let curr_palette = unsafe { palette::COLORS.get_unchecked(self.palette_id) };
        FrameParams {
            palette: *curr_palette,
            speed: self.speed,
            sat: self.saturation,
            val: self.brightness,
            timer_ms: Instant::now().as_millis() as _,
        }
    }
}

struct PlainState {
    last_sat: u8,
    last_val: u8,
    dirty: bool,
}

impl PlainState {
    pub fn tick(&mut self, params: FrameParams<'_>) -> Option<Rgb> {
        // For first flush
        if self.dirty {
            self.dirty = false;
            return Some(Self::determine_rgb(params));
        }

        if self.last_sat != params.sat || self.last_val != params.val {
            self.last_sat = params.sat;
            self.last_val = params.val;
            self.dirty = true;
            return Some(Self::determine_rgb(params));
        }
        None
    }

    #[inline(always)]
    fn determine_rgb(params: FrameParams<'_>) -> Rgb {
        color::hsv_to_rgb(Hsv {
            h: params.sat,
            s: u8::MAX,
            v: params.val,
        })
    }
}

enum Effect {
    Gradient,
    Plain(PlainState),
    Flow(FlowState),
    Vortex(VortexState),
    Sparkle(SparkleState),
    Ripple(RippleState, Lcg),
    Reactive(ReactiveState<REACTIVE_HITS>),
}

impl Effect {
    const RIPPLE_BASE_SEED: u64 = 0xDEAD_C0DE_CAFE_F00D;

    #[inline]
    const fn plain() -> Self {
        Self::Plain(PlainState {
            last_sat: u8::MAX,
            last_val: u8::MAX,
            dirty: true,
        })
    }

    #[inline]
    const fn flow() -> Self {
        Self::Flow(FlowState::new())
    }

    #[inline]
    const fn vortex() -> Self {
        Self::Vortex(VortexState::new())
    }

    #[inline]
    const fn sparkle() -> Self {
        Self::Sparkle(SparkleState::new())
    }

    #[inline]
    fn ripple() -> Self {
        Self::Ripple(
            RippleState::new(),
            Lcg::new((Self::RIPPLE_BASE_SEED ^ Instant::now().as_millis()) as _),
        )
    }

    #[inline]
    const fn reactive() -> Self {
        Self::Reactive(ReactiveState::new())
    }
}

struct Layout;

impl LedLayout for Layout {
    #[inline]
    fn count(&self) -> usize {
        LED_COUNT
    }

    #[inline]
    fn position(&self, index: usize) -> (u8, u8) {
        unsafe { *LED_COORD.get_unchecked(index) }
    }
}

struct Led {
    r: u8,
    g: u8,
    b: u8,
}

macro_rules! led {
    (_@($n:expr, $m:expr)) => {
        ($n - 1) * 16 + ($m - 1)
    };
    ({
        r: ($rn:expr, $rm:expr),
        g: ($gn:expr, $gm:expr),
        b: ($bn:expr, $bm:expr) $(,)?
    }) => {
        Led {
            r: led!(_@($rn, $rm)),
            g: led!(_@($gn, $gm)),
            b: led!(_@($bn, $bm)),
        }
    };
}

ltable!(LED_TABLE[Led] => {
    left: [
        led!({ r: (3, 2), g: (1, 1), b: (4, 2) }),
        led!({ r: (2, 2), g: (1, 2), b: (4, 3) }),
        led!({ r: (2, 3), g: (1, 3), b: (3, 3) }),
        led!({ r: (2, 4), g: (1, 4), b: (3, 4) }),
        led!({ r: (2, 5), g: (1, 5), b: (3, 5) }),
        led!({ r: (2, 6), g: (1, 6), b: (3, 6) }),
        led!({ r: (2, 7), g: (1, 7), b: (3, 7) }),
        led!({ r: (2, 8), g: (1, 8), b: (3, 8) }),
        led!({ r: (3, 1), g: (2, 1), b: (4, 1) }),

        led!({ r: (7, 8), g: (6, 8), b: (8, 8) }),
        led!({ r: (7, 7), g: (6, 7), b: (9, 8) }),
        led!({ r: (8, 7), g: (6, 6), b: (9, 7) }),
        led!({ r: (8, 6), g: (7, 6), b: (9, 6) }),
        led!({ r: (8, 5), g: (7, 5), b: (9, 5) }),
        led!({ r: (8, 4), g: (7, 4), b: (9, 4) }),
        led!({ r: (8, 3), g: (7, 3), b: (9, 3) }),
        led!({ r: (8, 2), g: (7, 2), b: (9, 2) }),
        led!({ r: (8, 1), g: (7, 1), b: (9, 1) }),

        led!({ r: (3, 10), g: (1, 9), b: (4, 10) }),
        led!({ r: (2, 10), g: (1, 10), b: (4, 11) }),
        led!({ r: (2, 11), g: (1, 11), b: (3, 11) }),
        led!({ r: (2, 12), g: (1, 12), b: (3, 12) }),
        led!({ r: (2, 13), g: (1, 13), b: (3, 13) }),
        led!({ r: (2, 14), g: (1, 14), b: (3, 14) }),
        led!({ r: (2, 15), g: (1, 15), b: (3, 15) }),
        led!({ r: (2, 16), g: (1, 16), b: (3, 16) }),
        led!({ r: (3, 9),  g: (2, 9),  b: (4, 9)  }),

        led!({ r: (7, 16), g: (6, 16), b: (8, 16) }),
        led!({ r: (7, 15), g: (6, 15), b: (9, 16) }),
        led!({ r: (8, 15), g: (6, 14), b: (9, 15) }),
        led!({ r: (8, 10), g: (7, 10), b: (9, 10) }),
        led!({ r: (8, 9),  g: (7, 9),  b: (9, 9)  }),
        led!({ r: (8, 11), g: (7, 11), b: (9, 11) }),
        led!({ r: (8, 12), g: (7, 12), b: (9, 12) }),
        led!({ r: (8, 13), g: (7, 13), b: (9, 13) }),
        led!({ r: (8, 14), g: (7, 14), b: (9, 14) }),
    ],
    right: [
        led!({ r: (3, 2), g: (1, 1), b: (4, 2) }),
        led!({ r: (2, 2), g: (1, 2), b: (4, 3) }),
        led!({ r: (2, 3), g: (1, 3), b: (3, 3) }),
        led!({ r: (2, 4), g: (1, 4), b: (3, 4) }),
        led!({ r: (2, 5), g: (1, 5), b: (3, 5) }),
        led!({ r: (2, 6), g: (1, 6), b: (3, 6) }),
        led!({ r: (2, 7), g: (1, 7), b: (3, 7) }),
        led!({ r: (2, 8), g: (1, 8), b: (3, 8) }),
        led!({ r: (3, 1), g: (2, 1), b: (4, 1) }),

        led!({ r: (7, 8), g: (6, 8), b: (8, 8) }),
        led!({ r: (7, 7), g: (6, 7), b: (9, 8) }),
        led!({ r: (8, 7), g: (6, 6), b: (9, 7) }),
        led!({ r: (8, 6), g: (7, 6), b: (9, 6) }),
        led!({ r: (8, 5), g: (7, 5), b: (9, 5) }),
        led!({ r: (8, 4), g: (7, 4), b: (9, 4) }),
        led!({ r: (8, 3), g: (7, 3), b: (9, 3) }),
        led!({ r: (8, 2), g: (7, 2), b: (9, 2) }),
        led!({ r: (8, 1), g: (7, 1), b: (9, 1) }),

        led!({ r: (3, 10), g: (1, 9), b: (4, 10) }),
        led!({ r: (2, 10), g: (1, 10), b: (4, 11) }),
        led!({ r: (2, 11), g: (1, 11), b: (3, 11) }),
        led!({ r: (2, 12), g: (1, 12), b: (3, 12) }),
        led!({ r: (2, 13), g: (1, 13), b: (3, 13) }),
        led!({ r: (2, 14), g: (1, 14), b: (3, 14) }),
        led!({ r: (2, 15), g: (1, 15), b: (3, 15) }),
        led!({ r: (2, 16), g: (1, 16), b: (3, 16) }),
        led!({ r: (3, 9),  g: (2, 9),  b: (4, 9)  }),

        led!({ r: (7, 16), g: (6, 16), b: (8, 16) }),
        led!({ r: (7, 15), g: (6, 15), b: (9, 16) }),
        led!({ r: (8, 15), g: (6, 14), b: (9, 15) }),
        led!({ r: (8, 10), g: (7, 10), b: (9, 10) }),
        led!({ r: (8, 9),  g: (7, 9),  b: (9, 9)  }),
        led!({ r: (8, 11), g: (7, 11), b: (9, 11) }),
        led!({ r: (8, 12), g: (7, 12), b: (9, 12) }),
        led!({ r: (8, 13), g: (7, 13), b: (9, 13) }),
        led!({ r: (8, 14), g: (7, 14), b: (9, 14) }),
    ],
});

ltable!(LED_COORD[(u8, u8)] => {
    left: [
        // Row 0 (Numbers & function area)
        (  0, 11), ( 19, 11), ( 39,  9), ( 59,  6), ( 79,  9), ( 98, 11),
        // Row 1
        (  0, 24), ( 19, 24), ( 39, 22), ( 59, 19), ( 79, 22), ( 98, 24),
        // Row 2 (Home row)
        (  0, 37), ( 19, 37), ( 39, 34), ( 59, 32), ( 79, 34), ( 98, 37),
        // Row 3 (Bottom row & inner columns)
        (  0, 49), ( 19, 49), ( 39, 47), ( 59, 44), ( 79, 47), ( 98, 49),
        // Row 4 & 5 (Thumb cluster & lower extensions)
        ( 98, 60), (109, 66), ( 59, 55), ( 19, 55), ( 39, 55), ( 79, 55),
        ( 19, 60), ( 39, 60), ( 59, 60), ( 79, 60), ( 19, 66), ( 39, 66),
    ],
    right: [
        (157, 11), (176, 11), (196,  9), (216,  6), (236,  9), (255, 11),
        // Row 1
        (157, 24), (176, 24), (196, 22), (216, 19), (236, 22), (255, 24),
        // Row 2 (Home row)
        (157, 37), (176, 37), (196, 34), (216, 32), (236, 34), (255, 37),
        // Row 3 (Bottom row & inner columns)
        (157, 49), (176, 49), (196, 47), (216, 44), (236, 47), (255, 49),
        // Row 4 & 5 (Thumb cluster & lower extensions)
        (146, 66), (157, 60), (196, 55), (176, 55), (216, 55), (236, 55),
        (176, 60), (196, 60), (216, 60), (236, 60), (176, 66), (196, 66),
    ]
});

matrix!(KEY_LED_MAP[u8] => {
    left: [
        [     0,      1,      2,      3,      4,      5,      6],
        [     7,      8,      9,     10,     11,     12,     13],
        [    14,     15,     16,     17,     18,     19,     20],
        [    21,     22,     23,     24,     25,     26, NO_LED],
        [    27,     28,     29,     30,     31, NO_LED, NO_LED],
        [    32,     33,     34,     35, NO_LED, NO_LED, NO_LED],
    ],
    right: [
        [    36,     37,     38,     39,     40,     41,     42],
        [    43,     44,     45,     46,     47,     48,     49],
        [    50,     51,     52,     53,     54,     55,     56],
        [NO_LED,     57,     58,     59,     60,     61,     62],
        [NO_LED, NO_LED,     63,     64,     65,     66,     67],
        [NO_LED, NO_LED, NO_LED,     68,     69,     70,     71],
    ]
});

#[inline]
const fn key_to_led(row: u8, col: u8) -> Option<u8> {
    match KEY_LED_MAP[row as usize][col as usize] {
        NO_LED => None,
        idx => Some(idx),
    }
}
