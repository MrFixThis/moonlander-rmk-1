#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keyboard;

#[cfg(feature = "defmt")]
use defmt_rtt as _;

use embassy_executor::Spawner;
use embassy_stm32::{bind_interrupts, peripherals};
use panic_halt as _;

use keyboard::{BuildDescriptor, Moonlander, UsbDriver, Watchdog};

#[cfg(all(not(feature = "rgb"), feature = "halfmoon"))]
bind_interrupts!(pub struct Irqs {
    USB_LP_CAN_RX0 => embassy_stm32::usb::InterruptHandler<peripherals::USB>;
});

#[cfg(any(feature = "rgb", not(feature = "halfmoon")))]
bind_interrupts!(pub struct Irqs {
    USB_LP_CAN_RX0 => embassy_stm32::usb::InterruptHandler<peripherals::USB>;
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<peripherals::I2C1>;
    DMA1_CHANNEL6 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH6>;
    DMA1_CHANNEL7 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH7>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let peripherals = embassy_stm32::init({
        use embassy_stm32::rcc::*;

        let mut config = embassy_stm32::Config::default();

        config.enable_debug_during_sleep = cfg!(feature = "defmt");
        config.rcc.hse = Some(Hse {
            freq: embassy_stm32::time::mhz(8),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            src: PllSource::HSE,
            prediv: PllPreDiv::DIV1,
            mul: PllMul::MUL9,
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;

        config
    });

    Moonlander::build(BuildDescriptor {
        usb_driver: UsbDriver::new(peripherals.USB, Irqs, peripherals.PA12, peripherals.PA11),
        watchdog: Watchdog::new(peripherals.IWDG, 3_000_000),
        col_pins: pins!(input(peripherals): [PA0, PA1, PA2, PA3, PA6, PA7, PB0]),
        row_pins: pins!(output(peripherals): [PB10, PB11, PB12, PB13, PB14, PB15]),
        #[cfg(feature = "indicators")]
        indic_pins: pins!(output(peripherals): [PB5, PB4, PB3]),
        #[cfg(feature = "audio")]
        audio_pins: pins!(output(peripherals): [PA5, PA4]),
        #[cfg(feature = "storage")]
        flash: peripherals.FLASH,
        #[cfg(any(feature = "rgb", not(feature = "halfmoon")))]
        i2c: keyboard::i2c::init(
            peripherals.I2C1,
            peripherals.PB6,
            peripherals.PB7,
            peripherals.DMA1_CH6,
            peripherals.DMA1_CH7,
        ),
    })
    .run()
    .await
}
