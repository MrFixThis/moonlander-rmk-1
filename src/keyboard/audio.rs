#![allow(dead_code)] // TMP

use embassy_stm32::gpio::Output;
use rmk::core_traits::Runnable;

pub type AudioPins = [Output<'static>; 2];

pub struct AudioDriver {
    pins: AudioPins,
}

impl AudioDriver {
    pub fn new(pins: AudioPins) -> Self {
        Self { pins }
    }
}

impl Runnable for AudioDriver {
    async fn run(&mut self) -> ! {
        loop {
            // TODO: To be implemented
            embassy_futures::yield_now().await;
        }
    }
}
