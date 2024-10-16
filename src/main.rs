#![no_std]
#![no_main]
#![deny(unused_must_use)]

use core::fmt::Write;
use core::panic::PanicInfo;

use embassy_futures::yield_now;
use embassy_rp::gpio::Output;
use embassy_rp::gpio::Level;
use embassy_rp::pio::ShiftDirection;
use embassy_rp::Peripheral;
use embassy_time::Timer;
use embassy_rp::pio::{self, Pio};
use embassy_rp::i2c::{self, I2c};
use embassy_executor::Spawner;

use embassy_rp::PeripheralRef;
use embassy_rp::{bind_interrupts, Peripherals};
use embassy_rp::peripherals::I2C0;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::InterruptHandler as PIOInterruptHandler;
use embassy_rp::i2c::InterruptHandler as I2CInterruptHandler;

use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

use defmt as _;
use defmt_rtt as _;
// use panic_probe as _;

use ssd1306::{prelude::*, Ssd1306};
use ssd1306::I2CDisplayInterface;

// Bind interrupts to their handlers.
bind_interrupts!(struct Irqs {
    I2C0_IRQ => I2CInterruptHandler<I2C0>;
    PIO0_IRQ_0 => PIOInterruptHandler<PIO0>;
});

#[panic_handler]
fn panic_handler(panic_info: &PanicInfo) -> ! {
    // SAFETY: we just panicked, therefore nobody has these
    let p = unsafe { Peripherals::steal() };

    let i2c = I2c::new_blocking(
        p.I2C0,
        p.PIN_1,
        p.PIN_0, 
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
    
    let _ = writeln!(display, "{}", panic_info.message());

    if let Some(location) = panic_info.location() {
        // let _ = writeln!(display, "file: {}", location.file());
        // let _ = writeln!(display, "line: {}", location.line());
    }

    loop {}
}

async fn w2812_driver() {
    let prg = pio_proc::pio_asm!(
        ".side_set 1",
        ".wrap_target",
        "bitloop:",
            "out x, 1 side 0 [3]", // set low 4 cycles (0.500us)
            "jmp !x, do_zero side 1 [1]", // set high 2 cycle (0.250us)
            "jmp bitloop side 1 [4]", // set high 5 cycles (0.625us)
        "do_zero:",
            "nop side 0 [2]" // set low 3 cycles (0.375us)
        // "out x, 1", // wait for data and read a bit into x (pin 1 should be set low)
        // "set pins, 1 [1]", // set pin high (0.4us)
        // "mov pins, x [1]", // set pin to x (0.4us)
        // "set pins, 0", // set pin low (0.2us + 0.2us during `out x, 1`)
        ".wrap"
    );
    let mut cfg = pio::Config::default();

    let mut raw_pin = p.PIN_28.into_ref();
    Output::new(raw_pin.reborrow(), Level::Low); // set the pin low
    let out_pin = common.make_pio_pin(raw_pin);

    
    let program = common.load_program(&prg.program);
    cfg.use_program(&program, &[&out_pin]);

    // cfg.set_out_pins(&[&out_pin]);
    // cfg.set_set_pins(&[&out_pin]);
    cfg.clock_divider = (U56F8!(125_000_000) / 8_000_000).to_fixed();

    cfg.shift_out.auto_fill = true;
    cfg.shift_out.threshold = 08;
    cfg.shift_out.direction = ShiftDirection::Right;

    sm0.set_pin_dirs(pio::Direction::Out, &[&out_pin]);
    sm0.set_config(&cfg);
    sm0.set_enable(true);
}

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
        mut sm0,
        ..
    } = Pio::new(p.PIO0, Irqs); 

    let mut dma = PeripheralRef::new(p.DMA_CH0);
    loop {
        let _ = display.clear();
        let _ = writeln!(display, "sending white");
        sm0.tx().wait_push(0).await;
        sm0.tx().wait_push(0).await;
        sm0.tx().wait_push(0).await;
        Timer::after_secs(1).await;

        let _ = display.clear();
        let _ = writeln!(display, "sending purple");
        // ???, blue, red, green
        // sm0.tx().wait_push(0xff_00_00_00).await;
        // sm0.tx().wait_push(0xff_00_00_00).await;
        sm0.tx().wait_push(u32::MAX).await;
        sm0.tx().wait_push(u32::MAX).await;
        sm0.tx().wait_push(u32::MAX).await;
        // sm0.tx().wait_push(0x00_00_00_ff).await;
        // sm0.tx().wait_push(0x00_ff_ff_00).await;
        Timer::after_secs(1).await;
    }
}
