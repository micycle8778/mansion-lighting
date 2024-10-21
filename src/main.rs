#![no_std]
#![no_main]
#![deny(unused_must_use)]

use core::fmt::Write;

use embassy_rp::gpio::Input;
use log::info;

use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_time::Duration;
use embassy_time::Timer;
use embassy_futures::join::join3;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use bt_hci::controller::ExternalController;
use static_cell::StaticCell;

use cyw43_pio::PioSpi;

use embassy_executor::Spawner;
use embassy_rp::gpio::Output;
use embassy_rp::gpio::Level;
use embassy_rp::i2c::{self, I2c};
use embassy_rp::pio::Pio;

use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::DMA_CH0;
use embassy_rp::peripherals::I2C0;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::InterruptHandler as PIOInterruptHandler;
use embassy_rp::i2c::InterruptHandler as I2CInterruptHandler;
use embassy_rp::usb::InterruptHandler as USBInterruptHandler;

use defmt as _;
use defmt_rtt as _;

use ssd1306::{prelude::*, Ssd1306};
use ssd1306::I2CDisplayInterface;

use emb_test::led::{Color, LedDriver};
use emb_test::blue;

// Bind interrupts to their handlers.
bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => USBInterruptHandler<USB>;
    I2C0_IRQ => I2CInterruptHandler<I2C0>;
    PIO0_IRQ_0 => PIOInterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Trace, driver);
}

#[embassy_executor::task]
async fn cyw43_task(runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 1, DMA_CH0>>) -> ! {
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize peripherals and USB driver.
    let p = embassy_rp::init(Default::default());

    // Spawn USB logger
    let usb_driver = Driver::new(p.USB, Irqs);
    spawner.spawn(logger_task(usb_driver)).unwrap();

    // sleep 1 second to give us time to start the serial connection
    Timer::after_secs(1).await;
    info!("Hello, world!");

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
    let _ = write!(display, "Hello, world!");

    // setup PIO
    let mut pio = Pio::new(p.PIO0, Irqs); 

    // initialize the w2812 LEDs
    let mut leds = {
        LedDriver::new(&mut pio.common, pio.sm0, p.PIN_28)
    };

    let mut big_red = Input::new(p.PIN_2, embassy_rp::gpio::Pull::Up);
    let mut count = 0;

    loop {
        display.clear().unwrap();
        let _ = write!(display, "count: {count}");

        for _ in 0..count {
            leds.send_color(Color::PURPLE).await;
        }

        big_red.wait_for_falling_edge().await;
        Timer::after_millis(100).await;

        count += 1;
    }
}
