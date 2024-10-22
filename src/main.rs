#![no_std]
#![no_main]
#![deny(unused_must_use)]

use core::fmt::Write;

use emb_test::lighting::Message;
use embassy_rp::pio::Instance;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use log::info;

use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_time::Timer;
use bt_hci::controller::ExternalController;
use static_cell::ConstStaticCell;
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

use emb_test::lighting;
use emb_test::led::LedDriver;
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

#[embassy_executor::task]
async fn lighting_task(
    led_driver: LedDriver<'static, PIO0, 0>, 
    recv: Receiver<'static, NoopRawMutex, Message, 1>
) -> ! {
    lighting::run(led_driver, recv).await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize peripherals and USB driver.
    let p = embassy_rp::init(Default::default());

    // Spawn USB logger
    let usb_driver = Driver::new(p.USB, Irqs);
    spawner.must_spawn(logger_task(usb_driver));

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
    let leds = {
        LedDriver::new(&mut pio.common, pio.sm0, p.PIN_28)
    };

    let lighting_channel = {
        static LIGHTING_CHANNEL: ConstStaticCell<Channel<NoopRawMutex, Message, 1>> = ConstStaticCell::new(Channel::new());
        LIGHTING_CHANNEL.take()
    };

    spawner.must_spawn(lighting_task(leds, lighting_channel.receiver()));

    // initialize the bluetooth chip
    // first, lets get the firmware in here. we need this firmware to use
    // the onboard bluetooth chip
    let fw = include_bytes!("../firmware/43439A0.bin");
    let clm = include_bytes!("../firmware/43439A0_clm.bin");
    let btfw = include_bytes!("../firmware/43439A0_btfw.bin");

    // setup pins for talking to the SPI bus of the bluetooth chip
    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let spi = PioSpi::new(&mut pio.common, pio.sm1, pio.irq0, cs, p.PIN_24, p.PIN_29, p.DMA_CH0);

    // spin up the driver
    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (_net_device, bt_device, mut control, runner) = cyw43::new_with_bluetooth(state, pwr, spi, fw, btfw).await;
    spawner.must_spawn(cyw43_task(runner));
    control.init(clm).await;

    let controller: ExternalController<_, 10> = ExternalController::new(bt_device);

    blue::run(controller, lighting_channel.sender()).await;
    panic!("end of program.");
}
