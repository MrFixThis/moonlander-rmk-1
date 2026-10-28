#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "rgb")]
mod rgb;

mod keymaps;
mod matrix;

use embassy_futures::join;
use embassy_stm32::{peripherals, usb::Driver, wdg::IndependentWatchdog};
use embassy_time::Timer;
use rmk::{
    KeymapData,
    config::{DeviceConfig, PositionalConfig, RmkConfig},
    core_traits::Runnable,
    keyboard::Keyboard,
    usb::UsbTransport,
};

use matrix::{ColPins, Left, RowPins};

pub const COLS: usize = 7;
pub const ROWS: usize = 12;

#[cfg(feature = "storage")]
pub type Flash = embassy_stm32::Peri<'static, peripherals::FLASH>;

#[cfg(feature = "storage")]
pub type StorageFlash = embassy_embedded_hal::adapter::BlockingAsync<
    embassy_stm32::flash::Flash<'static, embassy_stm32::flash::Blocking>,
>;

pub type UsbDriver = Driver<'static, peripherals::USB>;
pub type Watchdog = IndependentWatchdog<'static, peripherals::IWDG>;

pub struct BuildDescriptor {
    pub usb_driver: UsbDriver,
    pub watchdog: Watchdog,
    pub col_pins: ColPins,
    pub row_pins: RowPins,
    #[cfg(feature = "indicators")]
    pub indic_pins: matrix::IndicatorPins,
    #[cfg(feature = "audio")]
    pub audio_pins: audio::AudioPins,
    #[cfg(feature = "storage")]
    pub flash: Flash,
    #[cfg(any(feature = "rgb", not(feature = "halfmoon")))]
    pub i2c: &'static i2c::SharedI2c,
}

pub struct Moonlander {
    left: Left,
    #[cfg(not(feature = "halfmoon"))]
    right: matrix::Right,
    #[cfg(feature = "rgb")]
    rgb_controller: rgb::RgbController,
    #[cfg(feature = "audio")]
    audio_driver: audio::AudioDriver,
    #[cfg(feature = "storage")]
    storage_flash: Option<StorageFlash>,
    usb_driver: Option<UsbDriver>,
    watchdog: Option<Watchdog>,
}

impl Moonlander {
    pub fn build(desc: BuildDescriptor) -> Self {
        Self {
            usb_driver: Some(desc.usb_driver),
            watchdog: Some(desc.watchdog),
            left: Left::new(
                desc.col_pins,
                desc.row_pins,
                #[cfg(feature = "indicators")]
                desc.indic_pins,
            ),
            #[cfg(not(feature = "halfmoon"))]
            right: matrix::Right::new(desc.i2c),
            #[cfg(feature = "rgb")]
            rgb_controller: rgb::RgbController::new(desc.i2c),
            #[cfg(feature = "audio")]
            audio_driver: audio::AudioDriver::new(desc.audio_pins),
            #[cfg(feature = "storage")]
            storage_flash: Some(StorageFlash::new(
                embassy_stm32::flash::Flash::new_blocking(desc.flash),
            )),
        }
    }

    #[inline]
    pub async fn run(&mut self) {
        #[allow(clippy::needless_update)]
        let rmk_config = RmkConfig {
            device_config: DeviceConfig {
                product_name: "Moonlander MK I",
                ..Default::default()
            },
            #[cfg(feature = "storage")]
            storage_config: rmk::config::StorageConfig {
                num_sectors: 5,
                ..Default::default()
            },
            #[cfg(feature = "vial")]
            vial_config: rmk::config::VialConfig::new(
                keymaps::vial::VIAL_KEYBOARD_ID,
                keymaps::vial::VIAL_KEYBOARD_DEF,
                &[],
            ),
            ..Default::default()
        };

        let mut keymap_data = KeymapData::new(keymaps::KEYMAPS);
        let positional_config = PositionalConfig::new(keymaps::CHORDAL_HOLD_LAYOUT);
        let mut behavior_config = keymaps::behavior_config();

        #[cfg(feature = "storage")]
        let (keymap, mut storage) = rmk::initialize_keymap_and_storage(
            &mut keymap_data,
            self.storage_flash.take().unwrap(),
            &rmk_config.storage_config,
            &mut behavior_config,
            &positional_config,
        )
        .await;

        #[cfg(not(feature = "storage"))]
        let keymap =
            rmk::initialize_keymap(&mut keymap_data, &mut behavior_config, &positional_config)
                .await;

        let mut keyboard = Keyboard::new(&keymap);
        #[cfg(feature = "vial")]
        let vial_service = rmk::host::HostService::new(&keymap, &rmk_config);
        #[cfg_attr(feature = "vial", allow(unused_mut))]
        let mut transport =
            UsbTransport::new(self.usb_driver.take().unwrap(), rmk_config.device_config);
        #[cfg(feature = "vial")]
        let mut transport = transport.with_host_service(&vial_service);
        let mut watchdog = self.watchdog.take().unwrap();
        let watchdog_runner = async move {
            watchdog.unleash();
            loop {
                watchdog.pet();
                Timer::after_secs(1).await;
            }
        };

        let fut = join::join4(
            transport.run(),
            keyboard.run(),
            self.left.run(),
            watchdog_runner,
        );

        #[cfg(feature = "storage")]
        let fut = join::join(fut, storage.run());
        #[cfg(not(feature = "halfmoon"))]
        let fut = join::join(fut, self.right.run());
        #[cfg(feature = "rgb")]
        let fut = join::join(fut, self.rgb_controller.run());
        #[cfg(feature = "audio")]
        let fut = join::join(fut, self.audio_driver.run());

        fut.await;
    }
}

#[cfg(any(feature = "rgb", not(feature = "halfmoon")))]
pub mod i2c {
    use embassy_stm32::{
        Peri,
        gpio::{Flex, Speed},
        i2c::{Config, I2c as I2cIn, Master},
        mode::Async,
        peripherals,
        time::Hertz,
    };
    use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
    use embassy_time::{Duration, Timer};
    use static_cell::StaticCell;

    pub type I2c = I2cIn<'static, Async, Master>;
    pub type SharedI2c = Mutex<NoopRawMutex, Bus>;

    static I2C: StaticCell<SharedI2c> = StaticCell::new();

    #[repr(transparent)]
    pub struct Bus {
        device: Option<I2c>,
    }

    impl Bus {
        pub async fn write(&mut self, addr: u8, data: &[u8]) -> bool {
            let Some(device) = self.device.as_mut() else {
                return false;
            };
            device.write(addr, data).await.is_ok()
        }

        pub async fn write_read(&mut self, addr: u8, data: &[u8], buf: &mut [u8]) -> bool {
            let Some(device) = self.device.as_mut() else {
                return false;
            };
            device.write_read(addr, data, buf).await.is_ok()
        }

        pub async fn recover(&mut self) {
            drop(self.device.take());

            {
                // SAFETY: the driver is gone, nobody else owns these pins.
                let (mut scl, mut sda) = unsafe {
                    (
                        Flex::new(peripherals::PB6::steal()),
                        Flex::new(peripherals::PB7::steal()),
                    )
                };

                // Level first, then mode, so the lines never glitch low.
                scl.set_high();
                sda.set_high();
                scl.set_as_input_output(Speed::Low);
                sda.set_as_input_output(Speed::Low);
                Timer::after_micros(5).await;

                // Up to 9 clocks, until the slave lets go of SDA.
                for _ in 0..9 {
                    if sda.is_high() {
                        break;
                    }
                    scl.set_low();
                    Timer::after_micros(5).await;
                    scl.set_high();
                    Timer::after_micros(5).await;
                }

                // STOP: SDA low -> high while SCL is high.
                sda.set_low();
                Timer::after_micros(5).await;
                sda.set_high();
                Timer::after_micros(5).await;
            }

            // SAFETY: same as above, `device` is None so these are free.
            self.device = Some(unsafe {
                I2c::new(
                    peripherals::I2C1::steal(),
                    peripherals::PB6::steal(),
                    peripherals::PB7::steal(),
                    peripherals::DMA1_CH6::steal(),
                    peripherals::DMA1_CH7::steal(),
                    crate::Irqs,
                    config(),
                )
            });
        }
    }

    #[inline(always)]
    fn config() -> Config {
        let mut config = Config::default();
        config.frequency = Hertz::khz(400);
        config.timeout = Duration::from_millis(20);
        config
    }

    #[inline]
    #[must_use]
    pub fn init(
        i2c1: Peri<'static, peripherals::I2C1>,
        scl: Peri<'static, peripherals::PB6>,
        sda: Peri<'static, peripherals::PB7>,
        dma_tx: Peri<'static, peripherals::DMA1_CH6>,
        dma_rx: Peri<'static, peripherals::DMA1_CH7>,
    ) -> &'static SharedI2c {
        I2C.init(SharedI2c::new(Bus {
            device: Some(I2c::new(
                i2c1,
                scl,
                sda,
                dma_tx,
                dma_rx,
                crate::Irqs,
                config(),
            )),
        }))
    }
}
