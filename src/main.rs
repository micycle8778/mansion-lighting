#![no_std]
#![no_main]
#![deny(unused_must_use)]

use core::fmt::Write;

use embassy_executor::Spawner;
use embassy_rp::gpio::Input;
use embassy_rp::i2c::{self, I2c};
use embassy_rp::pio::Pio;

use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::I2C0;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::InterruptHandler as PIOInterruptHandler;
use embassy_rp::i2c::InterruptHandler as I2CInterruptHandler;

use defmt as _;
use defmt_rtt as _;

use ssd1306::{prelude::*, Ssd1306};
use ssd1306::I2CDisplayInterface;

use emb_test::led::{Color, LedDriver};
// Bind interrupts to their handlers.
bind_interrupts!(struct Irqs {
    I2C0_IRQ => I2CInterruptHandler<I2C0>;
    PIO0_IRQ_0 => PIOInterruptHandler<PIO0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize peripherals and USB driver.
    let p = embassy_rp::init(Default::default());

    // initialize the OLED (SSD1306)
    let i2c = I2c::new_async(
        p.I2C0,
        p.PIN_1,
        p.PIN_0, 
        Irqs,
        i2c::Config::default()
    );
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    ).into_terminal_mode();
    display.init().unwrap();
    display.clear().unwrap();

    // initialize the w2812 LEDs
    let Pio {
        mut common,
        sm0,
        ..
    } = Pio::new(p.PIO0, Irqs); 
    let mut leds = LedDriver::new(&mut common, sm0, p.PIN_28);

    let mut button = Input::new(p.PIN_2, embassy_rp::gpio::Pull::Up);
    
    let mut rng = fastrand::Rng::with_seed(1337);

    const NUM_LEDS: usize = 5;
    loop {
        {
            let dim_amount = rng.f32();

            let _ = display.clear();
            let _ = writeln!(display, "sending purple");
            let _ = writeln!(display, "{}", Color::WHITE.dim(dim_amount).red());

            for _ in 0..NUM_LEDS {
                leds.send_color(Color::PURPLE.dim(dim_amount)).await;
            }
        }

        button.wait_for_falling_edge().await;

        {
            let dim_amount = rng.f32();

            let _ = display.clear();
            let _ = writeln!(display, "sending white");
            let _ = writeln!(display, "{}", Color::WHITE.dim(dim_amount).red());

            for _ in 0..NUM_LEDS {
                leds.send_color(Color::WHITE.dim(dim_amount)).await;
            }
        }

        button.wait_for_falling_edge().await;
    }
}
