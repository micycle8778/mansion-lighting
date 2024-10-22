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

