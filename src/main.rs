#![no_std]
#![no_main]
#![deny(unused_must_use)]

use core::fmt::Write;

use embassy_executor::Executor;
use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_rp::multicore::Stack;

use emb_test::lighting::Message;
use embassy_rp::Peripheral;
use embassy_rp::PeripheralRef;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use log::info;

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

use emb_test::blue;
use emb_test::led::LedDriver;
use emb_test::lighting;

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
    let i2c = I2c::new_async(p.I2C0, p.PIN_1, p.PIN_0, Irqs, i2c::Config::default());
    let interface = I2CDisplayInterface::new(i2c);
    let mut display =
        Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0).into_terminal_mode();
    display.init().unwrap();
    display.clear().unwrap();
    let _ = write!(display, "Hello, world!");

    let mut pio = Pio::new(p.PIO1, Irqs);

    // initialize the w2812 LEDs
    let leds = { LedDriver::new(&mut pio.common, pio.sm0, p.PIN_28) };

    let lighting_channel = {
        static LIGHTING_CHANNEL: ConstStaticCell<Channel<CriticalSectionRawMutex, Message, 1>> =
            ConstStaticCell::new(Channel::new());
        LIGHTING_CHANNEL.take()
    };

    let recv = lighting_channel.receiver();
    embassy_rp::multicore::spawn_core1(p.CORE1, CORE1_STACK.take(), move || {
        let executor1 = EXECUTOR1.init(Executor::new());
        executor1.run(|spawner| spawner.spawn(lighting_task(leds, recv)).unwrap());
    });

    // initialize the bluetooth chip
    // first, lets get the firmware in here. we need this firmware to use
    // the onboard bluetooth chip
    let fw = include_bytes!("../firmware/43439A0.bin");
    let clm = include_bytes!("../firmware/43439A0_clm.bin");
    let btfw = include_bytes!("../firmware/43439A0_btfw.bin");

    // We're gonna loop here because the bluetooth driver cannot handle reconnection.
    // The bluetooth driver crashes after the client disconnects, and I can't be bothered
    // trying to fix it.

    let mut pwr_pin = PeripheralRef::new(p.PIN_23);
    let mut cs_pin = PeripheralRef::new(p.PIN_25);
    let mut dma = PeripheralRef::new(p.DMA_CH0);
    let mut pio = PeripheralRef::new(p.PIO0);

    let cyw43_state = {
        static STATE: StaticCell<cyw43::State> = StaticCell::new();
        STATE.init(cyw43::State::new())
    };

    loop {
        // SAFETY: i pinky promise i won't use these pins more than once per iteration
        let dio_pin = unsafe { p.PIN_24.clone_unchecked() };
        let clk_pin = unsafe { p.PIN_29.clone_unchecked() };
        let mut pio = Pio::new(pio.reborrow(), Irqs);

        // setup pins for talking to the SPI bus of the bluetooth chip
        let pwr = Output::new(pwr_pin.reborrow(), Level::Low);
        let cs = Output::new(cs_pin.reborrow(), Level::High);
        let spi = PioSpi::new(
            &mut pio.common,
            pio.sm1,
            pio.irq0,
            cs,
            dio_pin,
            clk_pin,
            dma.reborrow(),
        );

        // spin up the driver
        *cyw43_state = cyw43::State::new();
        let (_net_device, bt_device, mut control, runner) =
            cyw43::new_with_bluetooth(cyw43_state, pwr, spi, fw, btfw).await;
        let controller: ExternalController<_, 10> = ExternalController::new(bt_device);

        select(
            join(control.init(clm), runner.run()), // run the cyw43 driver
            blue::run(controller, lighting_channel.sender()), // run the ble driver
        )
        .await;
    }
}
