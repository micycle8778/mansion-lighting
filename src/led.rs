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

use crate::Color;

pub const NUM_LEDS: usize = 90;

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
