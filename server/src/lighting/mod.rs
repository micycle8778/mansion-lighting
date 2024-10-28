//! Lighting state and task
mod twinkle;

use embassy_time::Timer;
use twinkle::Twinkle;
use embassy_time::Instant;
use enum_dispatch::enum_dispatch;
use embassy_rp::peripherals::PIO1;
use log::info;
use log::error;

use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Receiver};

use crate::led::LedDriver;
use crate::led::NUM_LEDS;
use crate::Color;

type Driver = LedDriver<'static, PIO1, 0>;

#[enum_dispatch(AnimationEnum)]
trait Animation {
    async fn animate(&mut self, delta: f32, state: &mut State);
}

struct State {
    driver: Driver,
    base_color: Color,
    brightness: f32,
    skip: u8,
}

impl State {
    fn new(driver: Driver) -> Self {
        Self {
            driver,
            base_color: Color::WHITE,
            brightness: 1.0,
            skip: 0,
        }
    }
}

#[derive(Debug)]
#[enum_dispatch]
pub enum AnimationEnum {
    Twinkle,
}

impl AnimationEnum {
    pub fn from_bytes(bytes: [u8; 16]) -> Option<Self> {
        info!("AnimationEnum::from_bytes({bytes:?})");
        match bytes[0] {
            1 => {
                Some(Twinkle::new(bytes[1]).into())
            },
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum Message {
    /// This message does nothing
    Noop,
    /// Set the lights a color
    SetColor(Color),
    /// Set the brightness to a value between 0-255
    SetBrightness(u8),
    /// Set how many lights to skip to dim lights further
    SetSkip(u8),
    /// Use a color animation
    UseAnimation([u8; 16]),
    /// Set speed of animation
    SetAnimationSpeed(f32),
}

pub async fn run<M: RawMutex, const N: usize>(
    led_driver: Driver,
    recv: Receiver<'_, M, Message, N>,
) -> ! {
    let mut state = State::new(led_driver);

    let mut animation_speed = 1.0;
    let mut current_animation = None;

    let mut previous = Instant::now();
    loop {
        if let Ok(message) = recv.try_receive() {
            info!("[lighting] handling message {message:?}");
            match message {
                Message::Noop => {}
                Message::SetColor(c) => {
                    state.base_color = c;
                }
                Message::SetBrightness(b) => {
                    state.brightness = (b as f32) / 255.;
                }
                Message::SetSkip(s) => {
                    state.skip = s;
                }
                Message::UseAnimation(bytes) => {
                    drop(current_animation);
                    current_animation = AnimationEnum::from_bytes(bytes);
                }
                Message::SetAnimationSpeed(speed) => {
                    animation_speed = speed;
                },
            }
        }

        match &mut current_animation {
            Some(a) => {
                let delta = previous.elapsed().as_micros() as f32 / 1_000_000.0;
                a.animate(delta * 40. * animation_speed, &mut state).await;
            }
            None => {
                let color = state.base_color.dim(state.brightness);
                let mut n = state.skip;
                for _ in 0..NUM_LEDS {
                    if n == 0 {
                        state.driver.send_color(color).await;
                        n = state.skip;
                    } else {
                        state.driver.send_color(Color::BLACK).await;
                        n -= 1;
                    }
                }
            }
        }

        Timer::after_micros(500).await;
        previous = Instant::now();
    }
}
