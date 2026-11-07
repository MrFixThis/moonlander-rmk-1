mod animation;
mod palette;
mod state;

use embassy_futures::select;
use embassy_time::{Duration, Ticker, Timer};
use rmk::{
    core_traits::Runnable,
    event::{ActionEvent, KeyboardEvent, SubscribableEvent},
    input_device::InputDevice,
    types::action::{Action, LightAction},
};
use rmk_palettefx::{color::Hsv, layout::LedLayout};

use crate::keyboard::i2c::SharedI2c;

use state::{State, Storage};

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

const LED_COUNT: usize = ltable!(__@keys);
const LEFT_DRIVER_ADDR: u8 = 0x74;
#[cfg(not(feature = "halfmoon"))]
const RIGHT_DRIVER_ADDR: u8 = 0x77;

const NO_LED: u8 = 0xFF;
const EFFECT_TICK_MS: u64 = 80;

type ActionSubscriber = <ActionEvent as SubscribableEvent>::Subscriber;

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
