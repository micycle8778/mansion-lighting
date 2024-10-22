#[derive(Clone, Copy, Debug)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8
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
            red, green, blue
        }
    }

    pub const fn with_red(self, red: u8) -> Color {
        Self {
            red,
            green: self.green,
            blue: self.blue
        }
    }

    pub const fn with_green(self, green: u8) -> Color {
        Self {
            red: self.red,
            green,
            blue: self.blue
        }
    }

    pub const fn with_blue(self, blue: u8) -> Color {
        Self {
            red: self.red,
            green: self.green,
            blue
        }
    }

    pub const fn red(self) -> u8 {
        self.red
    }

    pub const fn green(self) -> u8 {
        self.green
    }

    pub const fn blue(self) -> u8 {
        self.blue
    }

    pub fn dim(self, multiplier: f32) -> Color {
        let red = (self.red() as f32 * multiplier) as u8;
        let green = (self.green() as f32 * multiplier) as u8;
        let blue = (self.blue() as f32 * multiplier) as u8;

        Self::new(red, green, blue)
    }

    // green, red, blue, ???
    pub const fn as_u32(self) -> u32 {
        ((self.green as u32) << 24)
        | ((self.red as u32) << 16)
        | ((self.blue as u32) << 8)
    }
}

