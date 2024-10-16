#![no_std]
#![no_main]
#![deny(unused_must_use)]


use core::fmt::Write;
use core::panic::PanicInfo;

use dht_sensor::InputOutputPin;
use embassy_futures::yield_now;
use embassy_rp::i2c::{Config, I2c};
use embedded_hal::digital::OutputPin;
use rand_core::RngCore;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Input, OutputOpenDrain};
use embassy_rp::gpio::{Level, Output, Pull};
use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, Peripherals};
use embassy_rp::peripherals::USB;
use embassy_rp::peripherals::I2C0;
use embassy_rp::i2c::InterruptHandler as I2CInterruptHandler;
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_time::{Delay, Instant, Timer};
use embassy_futures::select::{self, Either};

use log::info;

use defmt as _;
use defmt_rtt as _;
// use panic_probe as _;

use ssd1306::{prelude::*, Ssd1306};
use ssd1306::I2CDisplayInterface;

use dht11::{Dht11, Measurement};

// Bind interrupts to their handlers.
bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    I2C0_IRQ => I2CInterruptHandler<I2C0>;
});

#[panic_handler]
fn panic_handler(panic_info: &PanicInfo) -> ! {
    // SAFETY: we just panicked, therefore nobody has these
    let p = unsafe { Peripherals::steal() };

    let i2c = I2c::new_blocking(
        p.I2C0,
        p.PIN_1,
        p.PIN_0, 
        Config::default()
    );
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    ).into_terminal_mode();
    display.init().unwrap();
    display.clear().unwrap();
    
    let _ = writeln!(display, "{}", panic_info.message());

    if let Some(location) = panic_info.location() {
        let _ = writeln!(display, "file: {}", location.file());
        let _ = writeln!(display, "line: {}", location.line());
    }

    loop {}
}

// Async task for USB logging.
#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn cycle(mut pin: Output<'static>) {
    loop {
        pin.set_high();
        Timer::after_secs(1).await;
        pin.set_low();
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn panic_soon() {
    Timer::after_secs(10).await;
    panic!();
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize peripherals and USB driver.
    let rp_peripherals = embassy_rp::init(Default::default());
    let usb_driver = Driver::new(rp_peripherals.USB, Irqs);

    // Spawn the logger task
    // spawner.spawn(logger_task(usb_driver)).unwrap();
    spawner.spawn(cycle(Output::new(rp_peripherals.PIN_16, Level::High))).unwrap();
    // spawner.spawn(panic_soon()).unwrap();

    let i2c = I2c::new_async(
        rp_peripherals.I2C0,
        rp_peripherals.PIN_1,
        rp_peripherals.PIN_0, 
        Irqs,
        Config::default()
    );
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    ).into_terminal_mode();
    display.init().unwrap();
    display.clear().unwrap();

    let mut dht11 = Dht11::new(OutputOpenDrain::new(rp_peripherals.PIN_2, Level::Low));
    loop {
        let result = dht11.perform_measurement(&mut Delay);
        display.set_position(0, 0).unwrap();

        match result {
            Ok(r) => {
                let temp = (f32::from(r.temperature) / 10. * 1.8) + 32.;
                let humidity = f32::from(r.humidity) / 10.;
                let _ = writeln!(display, "temp: {:.1}°F", temp);
                let _ = writeln!(display, "humidity: {:.1}%", humidity);
            }
            Err(e) => {
                let _ = writeln!(display, "{:?}", e);
            }
        }

        let x = 69;
        let ptr = (&x as *const i32) as usize;
        let _ = writeln!(display, "mem: {:?}", ptr.wrapping_sub(0x20000000));

        yield_now().await;
    }

    // for i in 0..100 {
    //     let _ = writeln!(display, "{}", i);
    //     Timer::after_secs(1).await;
    // }

    // let mut rng = fastrand::Rng::with_seed(RoscRng.next_u64());
    // let mut led = Output::new(rp_peripherals.PIN_16, Level::Low);
    // let mut button = Input::new(rp_peripherals.PIN_15, Pull::Down);
    //
    // loop {
    //     info!("GAME START");
    //
    //     Timer::after_millis(200).await;
    //
    //     let either = select::select(
    //         Timer::after_millis(rng.u64(300..3300)), 
    //         button.wait_for_rising_edge()
    //     ).await;
    //
    //     if let Either::Second(_) = either {
    //         info!("FAIL");
    //         Timer::after_millis(500).await;
    //         button.wait_for_rising_edge().await;
    //         continue;
    //     }
    //
    //     info!("PRESS!!!");
    //     led.set_high();
    //     let now = Instant::now();
    //     button.wait_for_rising_edge().await;
    //     led.set_low();
    //     info!("duration: {}", now.elapsed().as_millis());
    //
    //     Timer::after_millis(500).await;
    //     button.wait_for_rising_edge().await;
    // }
}
