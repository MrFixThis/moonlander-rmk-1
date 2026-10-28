use embassy_stm32::gpio::{Input, Output};
use embassy_time::Timer;
use rmk::{
    core_traits::Runnable,
    debounce::{DebounceState, DebouncerTrait},
    event::{self, KeyboardEvent},
    input_device::InputDevice,
    matrix::KeyState,
};

#[cfg(not(feature = "halfmoon"))]
pub(super) use right::Right;

#[macro_export]
macro_rules! pins {
    (input($p:ident): [$($in:ident),+ $(,)?]) => {
        [$(pins!(input($p): $in)),+]
    };
    (output($p:ident): [$($out:ident),+ $(,)?]) => {
        [$(pins!(output($p): $out)),+]
    };
    (input($p:ident): $name:ident) => {
        ::embassy_stm32::gpio::Input::new($p.$name, ::embassy_stm32::gpio::Pull::Down)
    };
    (output($p:ident): $name:ident) => {{
        let mut pin = ::embassy_stm32::gpio::Output::new(
            $p.$name,
            ::embassy_stm32::gpio::Level::Low,
            ::embassy_stm32::gpio::Speed::VeryHigh
        );
        pin.set_low();
        pin
    }}
}

#[cfg(feature = "debounce_fast")]
pub(super) type Debouncer =
    rmk::debounce::fast_debouncer::FastDebouncer<{ super::ROWS / 2 }, { super::COLS }>;

#[cfg(not(feature = "debounce_fast"))]
pub(super) type Debouncer =
    rmk::debounce::default_debouncer::DefaultDebouncer<{ super::ROWS / 2 }, { super::COLS }>;

pub type ColPins = [Input<'static>; super::COLS];
pub type RowPins = [Output<'static>; super::ROWS / 2];

const MAX_COL_MASK: u8 = 0x7F;

#[repr(transparent)]
struct KeysState([u8; super::ROWS / 2]);

impl KeysState {
    #[inline(always)]
    fn new() -> Self {
        Self([0; super::ROWS / 2])
    }

    #[inline(always)]
    fn get(&self, row: usize, col: usize) -> bool {
        unsafe { (self.0.get_unchecked(row) & (1 << col)) != 0 }
    }

    #[inline(always)]
    fn press(&mut self, row: usize, col: usize) {
        let row_mut = unsafe { self.0.get_unchecked_mut(row) };
        *row_mut = (*row_mut | (1 << col)) & MAX_COL_MASK;
    }

    #[inline(always)]
    fn release(&mut self, row: usize, col: usize) {
        unsafe {
            *self.0.get_unchecked_mut(row) &= !(1 << col);
        }
    }
}

#[cfg(feature = "indicators")]
const INDICATOR_BOOT_STEP: u64 = 200;

#[cfg(feature = "indicators")]
pub type IndicatorPins = [Output<'static>; 3];

#[cfg(feature = "indicators")]
type LayerChangeSub = <rmk::event::LayerChangeEvent as rmk::event::SubscribableEvent>::Subscriber;

#[cfg(feature = "locks")]
type LocksSubscriber = <rmk::event::LedIndicatorEvent as rmk::event::SubscribableEvent>::Subscriber;

#[cfg(feature = "indicators")]
struct LeftIndicators {
    pins: IndicatorPins,
    layer_sub: LayerChangeSub,
    #[cfg(feature = "locks")]
    locks_sub: LocksSubscriber,
    last_layer: u8,
    #[cfg(feature = "locks")]
    last_il: rmk::types::led_indicator::LedIndicator,
}

#[cfg(feature = "indicators")]
impl LeftIndicators {
    fn new(pins: IndicatorPins) -> Self {
        use rmk::event::SubscribableEvent;
        Self {
            pins,
            layer_sub: rmk::event::LayerChangeEvent::subscriber(),
            last_layer: 0,
            #[cfg(feature = "locks")]
            locks_sub: rmk::event::LedIndicatorEvent::subscriber(),
            #[cfg(feature = "locks")]
            last_il: rmk::types::led_indicator::LedIndicator::new(),
        }
    }

    async fn init(&mut self) {
        for pin in self.pins.iter_mut() {
            pin.set_high();
            Timer::after_millis(INDICATOR_BOOT_STEP).await;
        }

        for pin in self.pins.iter_mut().rev() {
            pin.set_low();
            Timer::after_millis(INDICATOR_BOOT_STEP).await;
        }
    }

    fn sync(&mut self) {
        while let Some(rmk::event::LayerChangeEvent(layer_num)) =
            self.layer_sub.try_next_message_pure()
        {
            self.last_layer = layer_num;
        }

        #[cfg(feature = "locks")]
        while let Some(rmk::event::LedIndicatorEvent(indicator)) =
            self.locks_sub.try_next_message_pure()
        {
            self.last_il = indicator;
        }

        let is_active = self.last_layer > 0;
        let idx = (self.last_layer.saturating_sub(1)) % 3;

        let l1_layer = is_active && idx == 0;
        let l2_layer = is_active && idx == 1;
        let l3_layer = is_active && idx == 2;

        #[cfg(not(feature = "locks"))]
        let (l1, l2, l3) = (l1_layer, l2_layer, l3_layer);
        #[cfg(feature = "locks")]
        let (l1, l2, l3) = (
            l1_layer || self.last_il.num_lock() || self.last_il.kana(),
            l2_layer || self.last_il.caps_lock(),
            l3_layer || self.last_il.scroll_lock() || self.last_il.compose(),
        );

        let [led1, led2, led3] = &mut self.pins;
        led1.set_level(l1.into());
        led2.set_level(l2.into());
        led3.set_level(l3.into());
    }
}

pub struct Left {
    debouncer: Debouncer,
    keys_state: KeysState,
    col_pins: ColPins,
    row_pins: RowPins,
    #[cfg(feature = "indicators")]
    indicators: LeftIndicators,
}

impl Left {
    pub fn new(
        col_pins: ColPins,
        row_pins: RowPins,
        #[cfg(feature = "indicators")] indic_pins: IndicatorPins,
    ) -> Self {
        Self {
            debouncer: Debouncer::new(),
            keys_state: KeysState::new(),
            col_pins,
            row_pins,
            #[cfg(feature = "indicators")]
            indicators: LeftIndicators::new(indic_pins),
        }
    }
}

impl InputDevice for Left {
    type Event = Option<KeyboardEvent>;

    async fn read_event(&mut self) -> Self::Event {
        for row in 0..(super::ROWS / 2) {
            unsafe { self.row_pins.get_unchecked_mut(row) }.set_high();
            Timer::after_micros(1).await;

            for col in 0..super::COLS {
                let key_active = unsafe { self.col_pins.get_unchecked_mut(col) }.is_high();
                let key_state_tmp = self.keys_state.get(row, col).into_key_state();

                if let DebounceState::Debounced =
                    self.debouncer
                        .detect_change_with_debounce(row, col, key_active, &key_state_tmp)
                {
                    if key_active {
                        self.keys_state.press(row, col);
                    } else {
                        self.keys_state.release(row, col);
                    }

                    unsafe { self.row_pins.get_unchecked_mut(row) }.set_low();
                    return Some(KeyboardEvent::key(
                        row as _,
                        col as _,
                        self.keys_state.get(row, col),
                    ));
                }
            }
            unsafe { self.row_pins.get_unchecked_mut(row) }.set_low();
        }

        None
    }
}

impl Runnable for Left {
    async fn run(&mut self) -> ! {
        #[cfg(feature = "indicators")]
        self.indicators.init().await;

        loop {
            if let Some(event) = self.read_event().await {
                event::publish_event_async(event).await;
            }

            #[cfg(feature = "indicators")]
            self.indicators.sync();

            embassy_futures::yield_now().await;
        }
    }
}

trait IntoKeyState {
    fn into_key_state(self) -> KeyState;
}

impl IntoKeyState for bool {
    fn into_key_state(self) -> KeyState {
        let mut state = KeyState::new();
        if self {
            state.toggle_pressed();
        }
        state
    }
}

#[cfg(not(feature = "halfmoon"))]
mod right {
    use embassy_time::Timer;
    use rmk::{
        core_traits::Runnable,
        debounce::{DebounceState, DebouncerTrait},
        event::{self, KeyboardEvent},
        input_device::InputDevice,
    };

    use crate::keyboard::{self, i2c::SharedI2c};

    use super::{Debouncer, IntoKeyState, KeysState, MAX_COL_MASK};

    const MCP23018_ADDR: u8 = 0x20;

    const IO_DIREC_A: u8 = 0x00;
    const IO_DIREC_B: u8 = 0x01;
    const GPPU_A: u8 = 0x0C;
    const GPPU_B: u8 = 0x0D;
    const GPIO_A: u8 = 0x12;
    const GPIO_B: u8 = 0x13;

    const LED6_BIT_POS: u8 = 7;
    const PORT_B_INPUT_MASK: u8 = 0x3F;
    const PORT_A_IDLE: u8 = 0x7F;
    const PORT_B_IDLE: u8 = 0x3F;

    #[cfg(feature = "indicators")]
    struct RightIndicators {
        i2c: &'static SharedI2c,
        sub: super::LayerChangeSub,
        layer: u8,
        states: u8,
        launched: bool,
    }

    #[cfg(feature = "indicators")]
    impl RightIndicators {
        fn new(i2c: &'static SharedI2c) -> Self {
            use rmk::event::SubscribableEvent;
            Self {
                i2c,
                sub: rmk::event::LayerChangeEvent::subscriber(),
                layer: 0,
                states: 0x07,
                launched: false,
            }
        }

        async fn init(&mut self) -> bool {
            const A_LED6: u8 = 1 << LED6_BIT_POS;
            const B_LED5: u8 = 1 << 6;
            const B_LED4: u8 = 1 << 7;

            if self.launched {
                return true;
            }

            let steps = [
                [GPIO_A, PORT_A_IDLE],
                [GPIO_B, PORT_B_IDLE | B_LED4],
                [GPIO_B, PORT_B_IDLE],
                [GPIO_B, PORT_B_IDLE | B_LED4],
                [GPIO_B, PORT_B_IDLE | B_LED4 | B_LED5],
                [GPIO_A, PORT_A_IDLE | A_LED6],
            ];

            for cmd in &steps {
                if !self.i2c.lock().await.write(MCP23018_ADDR, cmd).await {
                    return false;
                }
                Timer::after_millis(super::INDICATOR_BOOT_STEP).await;
            }

            self.launched = true;
            true
        }

        fn sync(&mut self) {
            while let Some(rmk::event::LayerChangeEvent(layer_num)) =
                self.sub.try_next_message_pure()
            {
                self.layer = layer_num;
            }

            let should_light = self.layer > 0;
            let idx = (self.layer.saturating_sub(1)) % 3;

            let led4 = (should_light && idx == 0) as u8;
            let led5 = (should_light && idx == 1) as u8;
            let led6 = (should_light && idx == 2) as u8;

            self.states = !(led6 | (led5 << 1) | (led4 << 2));
        }

        #[inline(always)]
        fn led_a_bits(&self) -> u8 {
            (self.states & 0x01) << LED6_BIT_POS
        }

        #[inline(always)]
        fn led_b_bits(&self) -> u8 {
            ((self.states >> 1) & 0x03) << (LED6_BIT_POS - 1)
        }
    }

    pub struct Right {
        is_init: bool,
        i2c: &'static SharedI2c,
        debouncer: Debouncer,
        keys_state: KeysState,
        #[cfg(feature = "indicators")]
        indicators: RightIndicators,
    }

    impl Right {
        pub fn new(i2c: &'static SharedI2c) -> Self {
            Self {
                i2c,
                is_init: false,
                debouncer: Debouncer::new(),
                keys_state: KeysState::new(),
                #[cfg(feature = "indicators")]
                indicators: RightIndicators::new(i2c),
            }
        }

        #[cold]
        async fn configure_expander(&mut self) -> bool {
            let (led_a, led_b) = self.led_bits();
            let cmds = [
                [IO_DIREC_A, 0x00],
                [IO_DIREC_B, PORT_B_INPUT_MASK],
                [GPPU_A, 0x00],
                [GPPU_B, PORT_B_INPUT_MASK],
                [GPIO_A, PORT_A_IDLE | led_a],
                [GPIO_B, PORT_B_IDLE | led_b],
            ];

            let mut bus = self.i2c.lock().await;
            for cmd in &cmds {
                if !bus.write(MCP23018_ADDR, cmd).await {
                    return false;
                }
            }

            self.is_init = true;
            self.is_init
        }

        async fn release_all(&mut self) {
            for sense in 0..(keyboard::ROWS / 2) {
                for col in 0..keyboard::COLS {
                    if self.keys_state.get(sense, col) {
                        self.keys_state.release(sense, col);
                        let global_row = keyboard::ROWS - 1 - sense;
                        event::publish_event_async(KeyboardEvent::key(
                            global_row as _,
                            col as _,
                            false,
                        ))
                        .await;
                    }
                }
            }
            self.debouncer = Debouncer::new();
        }

        #[inline]
        async fn sunset(&mut self) {
            self.is_init = false;
            self.release_all().await;
        }

        #[inline]
        fn led_bits(&self) -> (u8, u8) {
            #[cfg(feature = "indicators")]
            let (led_a, led_b) = (self.indicators.led_a_bits(), self.indicators.led_b_bits());
            #[cfg(not(feature = "indicators"))]
            let (led_a, led_b) = (1 << LED6_BIT_POS, 0x03 << (LED6_BIT_POS - 1));
            (led_a, led_b)
        }
    }

    impl InputDevice for Right {
        type Event = Option<Option<KeyboardEvent>>;

        async fn read_event(&mut self) -> Self::Event {
            for strobe in 0..keyboard::COLS {
                let state = {
                    let mut bus = self.i2c.lock().await;
                    let (led_a, led_b) = self.led_bits();
                    let row_mask = (MAX_COL_MASK & !(1 << strobe)) | led_a;

                    if !bus
                        .write(MCP23018_ADDR, &[GPIO_A, row_mask, PORT_B_IDLE | led_b])
                        .await
                    {
                        return None;
                    }
                    Timer::after_micros(10).await;

                    let mut rx = [0u8; 1];
                    if !bus.write_read(MCP23018_ADDR, &[GPIO_B], &mut rx).await {
                        return None;
                    }
                    unsafe { *rx.get_unchecked(0) }
                };

                let col = keyboard::COLS - 1 - strobe;
                for sense in 0..(keyboard::ROWS / 2) {
                    let key_active = (state & (1 << sense)) == 0;
                    let global_row = keyboard::ROWS - 1 - sense;
                    let key_state_tmp = self.keys_state.get(sense, col).into_key_state();

                    if let DebounceState::Debounced = self.debouncer.detect_change_with_debounce(
                        sense,
                        col,
                        key_active,
                        &key_state_tmp,
                    ) {
                        if key_active {
                            self.keys_state.press(sense, col);
                        } else {
                            self.keys_state.release(sense, col);
                        }

                        return Some(Some(KeyboardEvent::key(
                            global_row as _,
                            col as _,
                            self.keys_state.get(sense, col),
                        )));
                    }
                }
            }

            Some(None)
        }
    }

    impl Runnable for Right {
        async fn run(&mut self) -> ! {
            loop {
                if !self.is_init {
                    if !self.configure_expander().await {
                        self.i2c.lock().await.recover().await;
                        Timer::after_millis(100).await;
                        continue;
                    }

                    #[cfg(feature = "indicators")]
                    if !self.indicators.init().await {
                        self.sunset().await;
                        continue;
                    }
                }

                #[cfg(feature = "indicators")]
                self.indicators.sync();

                match self.read_event().await {
                    Some(None) => {}
                    Some(Some(event)) => event::publish_event_async(event).await,
                    None => {
                        self.sunset().await;
                        continue;
                    }
                }

                embassy_futures::yield_now().await;
            }
        }
    }
}
