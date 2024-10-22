//! Lighting state and task

use log::info;

use embassy_rp::pio::Instance;
use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Receiver};

use crate::led::LedDriver;
use crate::led::NUM_LEDS;
use crate::Color;

#[derive(Debug)]
pub enum Animation {
}

#[derive(Debug)]
pub enum Message {
    // Set the lights a color
    SetColor(Color),
    // Set the brightness to a value between 0-255
    SetBrightness(u8),
    // Set how many lights to skip to dim lights further
    SetSkip(u8),
    // Use a color animation
    UseAnimation(Animation),
}

pub async fn run<M: RawMutex, PIO: Instance, const N: usize, const SM: usize>(
    mut led_driver: LedDriver<'_, PIO, SM>,
    recv: Receiver<'_, M, Message, N>
) -> ! {
    let mut brightness = 1.0;
    let mut base_color = Color::BLACK;
    let mut skip = 0;

    loop {
        let message = recv.receive().await;
        info!("[lighting] handling message {message:?}");
        match message {
            Message::SetColor(c) => {
                base_color = c;
            }
            Message::SetBrightness(b) => {
                brightness = (b as f32) / 255.;
            }
            Message::SetSkip(s) => {
                skip = s;
            }
        }

        let color = base_color.dim(brightness);
        let mut n = skip;
        for _ in 0..NUM_LEDS {
            if n == 0 {
                led_driver.send_color(color).await;
                n = skip;
            } else {
                led_driver.send_color(Color::BLACK).await;
                n -= 1;
            }
        }
    }
}
