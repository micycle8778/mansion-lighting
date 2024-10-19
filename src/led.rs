use embassy_rp::gpio::Output;
use embassy_rp::gpio::Level;

use embassy_rp::pio::StateMachine;
use embassy_rp::pio::Instance;
use embassy_rp::pio::Common;
use embassy_rp::pio::PioPin;
use embassy_rp::pio;
use embassy_rp::pio::ShiftDirection;

// use embassy_rp::pio::{self, Pio, PioPin, Instance, StateMachineTx};

use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

#[derive(Clone, Copy, Debug)]
pub struct Color {
    // ???, blue, red, green
    inner: u32
}

impl Color {
    pub const BLACK: Self = Self::new(0, 0, 0);

    pub const RED: Self = Self::new(255, 0, 0);
    pub const GREEN: Self = Self::new(0, 255, 0);
    pub const BLUE: Self = Self::new(0, 0, 255);
    
    pub const YELLOW: Self = Self::new(255, 255, 0);
    pub const CYAN: Self = Self::new(0, 255, 255);
    pub const PURPLE: Self = Self::new(255, 0, 255);

    pub const WHITE: Self = Self::new(255, 255, 255);

    pub const fn new(red: u8, green: u8, blue: u8) -> Color {
        Self {
            inner: ((blue as u32) << 16) | ((red as u32) << 8) | (green as u32)
        }
    }

    pub const fn with_red(self, red: u8) -> Color {
        Self {
            inner: (self.inner & 0b11111111_00000000_11111111) | ((red as u32) << 8)
        }
    }

    pub const fn with_green(self, green: u8) -> Color {
        Self {
            inner: (self.inner & 0b11111111_11111111_00000000) | (green as u32)
        }
    }

    pub const fn with_blue(self, blue: u8) -> Color {
        Self {
            inner: (self.inner & 0b00000000_11111111_11111111) | ((blue as u32) << 16)
        }
    }

    pub const fn red(self) -> u8 {
        ((self.inner & 0b00000000_11111111_00000000) >> 8) as u8
    }

    pub const fn green(self) -> u8 {
        self.inner as u8
    }

    pub const fn blue(self) -> u8 {
        ((self.inner & 0b11111111_00000000_00000000) >> 16) as u8
    }

    pub fn dim(self, multiplier: f32) -> Color {
        let red = (self.red() as f32 * multiplier) as u8;
        let green = (self.green() as f32 * multiplier) as u8;
        let blue = (self.blue() as f32 * multiplier) as u8;

        Self::new(red, green, blue)
    }

    pub const fn as_u32(self) -> u32 {
        self.inner
    }
}

pub struct LedDriver<'peripherals, PIO: Instance, const SM: usize> {
    sm: StateMachine<'peripherals, PIO, SM>
}

impl<'peripheral, PIO: Instance, const SM: usize> LedDriver<'peripheral, PIO, SM> {
    pub fn new(
        common: &mut Common<'peripheral, PIO>,
        mut sm: StateMachine<'peripheral, PIO, SM>,
        pin: impl PioPin
    ) -> Self {
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

        let mut raw_pin = pin.into_ref();
        Output::new(raw_pin.reborrow(), Level::Low); // set the pin low
        let out_pin = common.make_pio_pin(raw_pin);


        let program = common.load_program(&prg.program);
        cfg.use_program(&program, &[&out_pin]);

        // cfg.set_out_pins(&[&out_pin]);
        // mfg.set_set_pins(&[&out_pin]);
        cfg.clock_divider = (U56F8!(125_000_000) / 8_000_000).to_fixed();

        cfg.shift_out.auto_fill = true;
        cfg.shift_out.threshold = 24;
        cfg.shift_out.direction = ShiftDirection::Right;

        sm.set_pin_dirs(pio::Direction::Out, &[&out_pin]);
        sm.set_config(&cfg);
        sm.set_enable(true);

        Self { sm }
    }

    pub async fn send_color(&mut self, color: Color) {
        self.sm.tx().wait_push(color.as_u32()).await;
    }
}
