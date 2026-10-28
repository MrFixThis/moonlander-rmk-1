#![allow(dead_code)] // TMP

use rmk::core_traits::Runnable;

use crate::keyboard::i2c::SharedI2c;

pub struct RgbController {
    i2c: &'static SharedI2c,
}

impl RgbController {
    pub fn new(i2c: &'static SharedI2c) -> Self {
        Self { i2c }
    }
}

impl Runnable for RgbController {
    async fn run(&mut self) -> ! {
        info!("[Moonlander]: RGB controller initialized");
        loop {
            // TODO: To be implemented
            embassy_futures::yield_now().await;
        }
    }
}
