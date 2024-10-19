use core::fmt::Write;
use core::panic::PanicInfo;

use embassy_rp::Peripherals;
use embassy_rp::i2c;
use embassy_rp::i2c::I2c;

use ssd1306::prelude::*;
use ssd1306::Ssd1306;
use ssd1306::I2CDisplayInterface;

#[panic_handler]
pub fn panic_handler(panic_info: &PanicInfo) -> ! {
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

