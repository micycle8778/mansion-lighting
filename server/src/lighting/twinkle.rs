use half::f16;
use rand_core::RngCore;
use crate::Color;
use crate::lighting::State;
use crate::lighting::Animation;
use embassy_rp::clocks::RoscRng;
use embassy_sync::mutex::MutexGuard;
use crate::lighting::NUM_LEDS;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use log::info;

#[derive(Copy, Clone)]
enum Star {
    Dead,
    Starting { target: f16, at: f16 },
    Decaying(f16)
}

impl Star {
    fn dead() -> Self {
        Star::Dead
    }

    fn starting(target: f32) -> Self {
        info!("Star::starting(target = {target})");
        Star::Starting { 
            target: f16::from_f32(target),
            at: f16::from_bits(0) 
        }
    }

    fn decaying(f: f32) -> Self {
        info!("Star::decaying(f = {f})");
        Star::Decaying(f16::from_f32(f))
    }

    fn tick(&mut self, delta: f32) -> bool {
        let delta = f16::from_f32(delta);
        match self {
            Self::Dead => false,
            Self::Starting { target, at } => {
                *at += delta;
                if at >= target {
                    *self = Self::Decaying(*target)
                }
                false
            }
            Self::Decaying(f) => {
                *f -= delta;
                if *f <= f16::from_bits(0) {
                    *self = Self::Dead;
                    true
                } else {
                    false
                }
            }
        }
    }

    fn brightness(&self) -> f32 {
        match self {
            Self::Dead => 0.0,
            Self::Starting { at, .. } => f32::from(*at),
            Self::Decaying(f) => f32::from(*f),
        }
    }

    fn is_dead(&self) -> bool {
        matches!(self, Star::Dead)
    }
}

static STARS: Mutex<ThreadModeRawMutex, [Star; NUM_LEDS]> = Mutex::new([Star::Dead; NUM_LEDS]);
pub struct Twinkle {
    stars: MutexGuard<'static, ThreadModeRawMutex, [Star; NUM_LEDS]>
}

impl Twinkle {
    pub fn new(star_count: u8) -> Self {
        let mut stars = STARS.try_lock().unwrap();
        *stars = [Star::Dead; NUM_LEDS];

        let len = stars.len();

        let mut rng = fastrand::Rng::with_seed(RoscRng.next_u64());
        for _ in 0..star_count {
            stars[rng.usize(0..len)] = Star::decaying(rng.f32());
        }
        
        Self { 
            stars
        }
    }
}

impl Animation for Twinkle {
    async fn animate(&mut self, delta: f32, state: &mut State) {
        let mut rng = fastrand::Rng::with_seed(RoscRng.next_u64());
        let mut colors = [Color::BLACK; NUM_LEDS];

        for idx in 0..self.stars.len() {
            if self.stars[idx].tick(delta) {
                loop {
                    let idx = rng.usize(0..self.stars.len());
                    if self.stars[idx].is_dead() {
                        self.stars[idx] = Star::starting(rng.f32().max(0.1));
                        break;
                    }
                }
            }

            colors[idx] = state.base_color.dim(self.stars[idx].brightness() * state.brightness);
        }

        for color in colors {
            state.driver.send_color(color).await;
        }
    }
}

