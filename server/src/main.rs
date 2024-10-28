#![no_std]
#![no_main]
#![deny(unused_must_use)]

use core::fmt::Write;

use cortex_m::asm::wfe;
use embassy_executor::Executor;
use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_futures::yield_now;
use embassy_rp::multicore::Stack;

use embassy_rp::Peripheral;
use embassy_rp::PeripheralRef;
use embassy_rp::Peripherals;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use embassy_time::Instant;
use log::info;
use mansion_lighting::lighting::Message;

use bt_hci::controller::ExternalController;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_time::Timer;
use static_cell::ConstStaticCell;
use static_cell::StaticCell;

use cyw43_pio::PioSpi;

use embassy_executor::Spawner;
use embassy_rp::gpio::Level;
use embassy_rp::gpio::Output;
use embassy_rp::i2c::{self, I2c};
use embassy_rp::pio::Pio;

use embassy_rp::bind_interrupts;
use embassy_rp::i2c::InterruptHandler as I2CInterruptHandler;
use embassy_rp::peripherals::I2C0;
use embassy_rp::peripherals::PIO0;
use embassy_rp::peripherals::PIO1;
use embassy_rp::pio::InterruptHandler as PIOInterruptHandler;
use embassy_rp::usb::InterruptHandler as USBInterruptHandler;

use defmt as _;
use defmt_rtt as _;

use ssd1306::I2CDisplayInterface;
use ssd1306::{prelude::*, Ssd1306};

use mansion_lighting::blue;
use mansion_lighting::led::LedDriver;
use mansion_lighting::lighting;

// Bind interrupts to their handlers.
bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => USBInterruptHandler<USB>;
    I2C0_IRQ => I2CInterruptHandler<I2C0>;
    PIO0_IRQ_0 => PIOInterruptHandler<PIO0>;
    PIO1_IRQ_0 => PIOInterruptHandler<PIO1>;
});

static CORE1_STACK: ConstStaticCell<Stack<4096>> = ConstStaticCell::new(Stack::new());
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn lighting_task(
    led_driver: LedDriver<'static, PIO1, 0>,
    recv: Receiver<'static, CriticalSectionRawMutex, Message, 1>,
) -> ! {
    lighting::run(led_driver, recv).await;
}

async fn dim(led: &mut Output<'_>, n: u64) {
    let start = Instant::now();
    loop {
        let millis = start.elapsed().as_millis() % 10;

        if millis < n {
            led.set_low();
        } else {
            led.set_high();
        }

        yield_now().await;
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut led = Output::new(p.PIN_16, Level::High);

    for i in 0..10 {
        select(Timer::after_secs(1), dim(&mut led, i)).await;
    }
}
