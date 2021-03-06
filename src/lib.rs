#![no_std]
#![forbid(deprecated)]
#![deny(warnings)]

use embedded_hal::blocking;
use embedded_hal::digital::v2::OutputPin;

const WIDTH: u8 = 84;
const HEIGHT: u8 = 48;
const ROWS: u8 = HEIGHT / 8;

#[repr(u8)]
pub enum TemperatureCoefficient {
    TC0 = 0,
    TC1 = 1,
    TC2 = 2,
    TC3 = 3,
}

#[repr(u8)]
pub enum BiasMode {
    Bias1To100 = 0,
    Bias1To80 = 1,
    Bias1To65 = 2,
    Bias1To48 = 3,
    Bias1To40 = 4,
    Bias1To24 = 5,
    Bias1To18 = 6,
    Bias1To10 = 7,
}

#[repr(u8)]
pub enum DisplayMode {
    DisplayBlank = 0b000,
    NormalMode = 0b100,
    AllSegmentsOn = 0b001,
    InverseVideoMode = 0b101,
}

#[derive(Debug)]
pub enum OutputError<SPIE, DCE, CEE, RSTE, LIGHTE> {
    SPIError(SPIE),
    DCError(DCE),
    CEError(CEE),
    RSTError(RSTE),
    LIGHTError(LIGHTE),
}

pub struct PCD8544<SPI, DC, CE, RST, LIGHT>
where
    SPI: blocking::spi::Write<u8>,
    DC: OutputPin,
    CE: OutputPin,
    RST: OutputPin,
    LIGHT: OutputPin,
{
    spi: SPI,
    dc: DC,
    ce: CE,
    rst: RST,
    light: LIGHT,
    power_down_control: bool,
    entry_mode: bool,
    extended_instruction_set: bool,
    x: u8,
    y: u8,
}

impl<SPI, DC, CE, RST, LIGHT> PCD8544<SPI, DC, CE, RST, LIGHT>
where
    SPI: blocking::spi::Write<u8>,
    DC: OutputPin,
    CE: OutputPin,
    RST: OutputPin,
    LIGHT: OutputPin,
{
    // TODO: somehow add type alias for this, like:
    //   type Error = OutputError<SPI::Error, DC::Error, ...>;
    // This is not yet possible, see
    // https://github.com/rust-lang/rfcs/blob/master/text/0195-associated-items.md

    pub fn new(
        spi: SPI,
        dc: DC,
        mut ce: CE,
        mut rst: RST,
        light: LIGHT,
    ) -> Result<
        PCD8544<SPI, DC, CE, RST, LIGHT>,
        OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>,
    > {
        rst.set_low().map_err(|e| OutputError::RSTError(e))?;
        ce.set_high().map_err(|e| OutputError::CEError(e))?;
        Ok(PCD8544 {
            spi,
            dc,
            ce,
            rst,
            light,
            power_down_control: false,
            entry_mode: false,
            extended_instruction_set: false,
            x: 0,
            y: 0,
        })
    }

    pub fn reset(
        &mut self,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.rst.set_low().map_err(|e| OutputError::RSTError(e))?;
        self.x = 0;
        self.y = 0;
        self.init()?;
        Ok(())
    }

    pub fn init(
        &mut self,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        // reset the display
        self.rst.set_low().map_err(|e| OutputError::RSTError(e))?;
        self.rst.set_high().map_err(|e| OutputError::RSTError(e))?;

        // reset state variables
        self.power_down_control = false;
        self.entry_mode = false;
        self.extended_instruction_set = false;

        // write init configuration
        self.enable_extended_commands(true)?;
        self.set_contrast(56_u8)?;
        self.set_temperature_coefficient(TemperatureCoefficient::TC3)?;
        self.set_bias_mode(BiasMode::Bias1To40)?;
        self.enable_extended_commands(false)?;
        self.set_display_mode(DisplayMode::NormalMode)?;

        // clear display data
        self.clear()?;
        Ok(())
    }

    pub fn clear(
        &mut self,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        for _ in 0..(WIDTH as u16 * ROWS as u16) {
            self.write_data(0x00)?;
        }
        self.set_x_position(0)?;
        self.set_y_position(0)?;
        Ok(())
    }

    pub fn set_power_down(
        &mut self,
        power_down: bool,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.power_down_control = power_down;
        self.write_current_function_set()?;
        Ok(())
    }

    pub fn set_entry_mode(
        &mut self,
        entry_mode: bool,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.entry_mode = entry_mode;
        self.write_current_function_set()?;
        Ok(())
    }

    pub fn x(&self) -> u8 {
        self.x
    }

    pub fn y(&self) -> u8 {
        self.y
    }

    pub fn set_x_position(
        &mut self,
        x: u8,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        let x = x % WIDTH;
        self.x = x;
        self.write_command(0x80 | x)
    }

    pub fn set_y_position(
        &mut self,
        y: u8,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        let y = y % ROWS;
        self.y = y;
        self.write_command(0x40 | y)
    }

    pub fn set_light(
        &mut self,
        enabled: bool,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        if enabled {
            self.light
                .set_low()
                .map_err(|e| OutputError::LIGHTError(e))?;
        } else {
            self.light
                .set_high()
                .map_err(|e| OutputError::LIGHTError(e))?;
        }
        Ok(())
    }

    pub fn set_display_mode(
        &mut self,
        mode: DisplayMode,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.write_command(0x08 | mode as u8)
    }

    pub fn set_bias_mode(
        &mut self,
        bias: BiasMode,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.write_command(0x10 | bias as u8)
    }

    pub fn set_temperature_coefficient(
        &mut self,
        coefficient: TemperatureCoefficient,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.write_command(0x04 | coefficient as u8)
    }

    /// contrast in range of 0..128
    pub fn set_contrast(
        &mut self,
        contrast: u8,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.write_command(0x80 | contrast)
    }

    pub fn enable_extended_commands(
        &mut self,
        enable: bool,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.extended_instruction_set = enable;
        self.write_current_function_set()?;
        Ok(())
    }

    fn write_current_function_set(
        &mut self,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        let power = self.power_down_control;
        let entry = self.entry_mode;
        let extended = self.extended_instruction_set;
        self.write_function_set(power, entry, extended)?;
        Ok(())
    }

    fn write_function_set(
        &mut self,
        power_down_control: bool,
        entry_mode: bool,
        extended_instruction_set: bool,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        let mut val = 0x20;
        if power_down_control {
            val |= 0x04;
        }
        if entry_mode {
            val |= 0x02;
        }
        if extended_instruction_set {
            val |= 0x01;
        }
        self.write_command(val)?;
        Ok(())
    }

    pub fn write_command(
        &mut self,
        value: u8,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.write_byte(false, value)?;
        Ok(())
    }

    pub fn write_data(
        &mut self,
        value: u8,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        self.write_byte(true, value)?;
        Ok(())
    }

    fn write_byte(
        &mut self,
        data: bool,
        value: u8,
    ) -> Result<(), OutputError<SPI::Error, DC::Error, CE::Error, RST::Error, LIGHT::Error>> {
        if data {
            self.dc.set_high().map_err(|e| OutputError::DCError(e))?;
            self.increase_position();
        } else {
            self.dc.set_low().map_err(|e| OutputError::DCError(e))?;
        }
        self.ce.set_low().map_err(|e| OutputError::CEError(e))?;

        self.spi
            .write(&[value])
            .map_err(|e| OutputError::SPIError(e))?;

        self.ce.set_high().map_err(|e| OutputError::CEError(e))?;
        Ok(())
    }

    fn increase_position(&mut self) {
        self.x = (self.x + 1) % WIDTH;
        if self.x == 0 {
            self.y = (self.y + 1) & ROWS;
        }
    }
}

use core::fmt::{self, Write};

fn char_to_bytes(char: char) -> &'static [u8] {
    match char {
        ' ' => &[0x00, 0x00, 0x00, 0x00, 0x00],
        '!' => &[0x00, 0x00, 0x5f, 0x00, 0x00],
        '"' => &[0x00, 0x07, 0x00, 0x07, 0x00],
        '#' => &[0x14, 0x7f, 0x14, 0x7f, 0x14],
        '$' => &[0x24, 0x2a, 0x7f, 0x2a, 0x12],
        '%' => &[0x23, 0x13, 0x08, 0x64, 0x62],
        '&' => &[0x36, 0x49, 0x55, 0x22, 0x50],
        '\'' => &[0x00, 0x05, 0x03, 0x00, 0x00],
        '(' => &[0x00, 0x1c, 0x22, 0x41, 0x00],
        ')' => &[0x00, 0x41, 0x22, 0x1c, 0x00],
        '*' => &[0x14, 0x08, 0x3e, 0x08, 0x14],
        '+' => &[0x08, 0x08, 0x3e, 0x08, 0x08],
        ',' => &[0x00, 0x50, 0x30, 0x00, 0x00],
        '-' => &[0x08, 0x08, 0x08, 0x08, 0x08],
        '.' => &[0x00, 0x60, 0x60, 0x00, 0x00],
        '/' => &[0x20, 0x10, 0x08, 0x04, 0x02],
        '0' => &[0x3e, 0x51, 0x49, 0x45, 0x3e],
        '1' => &[0x00, 0x42, 0x7f, 0x40, 0x00],
        '2' => &[0x42, 0x61, 0x51, 0x49, 0x46],
        '3' => &[0x21, 0x41, 0x45, 0x4b, 0x31],
        '4' => &[0x18, 0x14, 0x12, 0x7f, 0x10],
        '5' => &[0x27, 0x45, 0x45, 0x45, 0x39],
        '6' => &[0x3c, 0x4a, 0x49, 0x49, 0x30],
        '7' => &[0x01, 0x71, 0x09, 0x05, 0x03],
        '8' => &[0x36, 0x49, 0x49, 0x49, 0x36],
        '9' => &[0x06, 0x49, 0x49, 0x29, 0x1e],
        ':' => &[0x00, 0x36, 0x36, 0x00, 0x00],
        ';' => &[0x00, 0x56, 0x36, 0x00, 0x00],
        '<' => &[0x08, 0x14, 0x22, 0x41, 0x00],
        '=' => &[0x14, 0x14, 0x14, 0x14, 0x14],
        '>' => &[0x00, 0x41, 0x22, 0x14, 0x08],
        '?' => &[0x02, 0x01, 0x51, 0x09, 0x06],
        '@' => &[0x32, 0x49, 0x79, 0x41, 0x3e],
        'A' => &[0x7e, 0x11, 0x11, 0x11, 0x7e],
        'B' => &[0x7f, 0x49, 0x49, 0x49, 0x36],
        'C' => &[0x3e, 0x41, 0x41, 0x41, 0x22],
        'D' => &[0x7f, 0x41, 0x41, 0x22, 0x1c],
        'E' => &[0x7f, 0x49, 0x49, 0x49, 0x41],
        'F' => &[0x7f, 0x09, 0x09, 0x09, 0x01],
        'G' => &[0x3e, 0x41, 0x49, 0x49, 0x7a],
        'H' => &[0x7f, 0x08, 0x08, 0x08, 0x7f],
        'I' => &[0x00, 0x41, 0x7f, 0x41, 0x00],
        'J' => &[0x20, 0x40, 0x41, 0x3f, 0x01],
        'K' => &[0x7f, 0x08, 0x14, 0x22, 0x41],
        'L' => &[0x7f, 0x40, 0x40, 0x40, 0x40],
        'M' => &[0x7f, 0x02, 0x0c, 0x02, 0x7f],
        'N' => &[0x7f, 0x04, 0x08, 0x10, 0x7f],
        'O' => &[0x3e, 0x41, 0x41, 0x41, 0x3e],
        'P' => &[0x7f, 0x09, 0x09, 0x09, 0x06],
        'Q' => &[0x3e, 0x41, 0x51, 0x21, 0x5e],
        'R' => &[0x7f, 0x09, 0x19, 0x29, 0x46],
        'S' => &[0x46, 0x49, 0x49, 0x49, 0x31],
        'T' => &[0x01, 0x01, 0x7f, 0x01, 0x01],
        'U' => &[0x3f, 0x40, 0x40, 0x40, 0x3f],
        'V' => &[0x1f, 0x20, 0x40, 0x20, 0x1f],
        'W' => &[0x3f, 0x40, 0x38, 0x40, 0x3f],
        'X' => &[0x63, 0x14, 0x08, 0x14, 0x63],
        'Y' => &[0x07, 0x08, 0x70, 0x08, 0x07],
        'Z' => &[0x61, 0x51, 0x49, 0x45, 0x43],
        '[' => &[0x00, 0x7f, 0x41, 0x41, 0x00],
        '¥' => &[0x02, 0x04, 0x08, 0x10, 0x20],
        ']' => &[0x00, 0x41, 0x41, 0x7f, 0x00],
        '^' => &[0x04, 0x02, 0x01, 0x02, 0x04],
        '_' => &[0x40, 0x40, 0x40, 0x40, 0x40],
        '`' => &[0x00, 0x01, 0x02, 0x04, 0x00],
        'a' => &[0x20, 0x54, 0x54, 0x54, 0x78],
        'b' => &[0x7f, 0x48, 0x44, 0x44, 0x38],
        'c' => &[0x38, 0x44, 0x44, 0x44, 0x20],
        'd' => &[0x38, 0x44, 0x44, 0x48, 0x7f],
        'e' => &[0x38, 0x54, 0x54, 0x54, 0x18],
        'f' => &[0x08, 0x7e, 0x09, 0x01, 0x02],
        'g' => &[0x0c, 0x52, 0x52, 0x52, 0x3e],
        'h' => &[0x7f, 0x08, 0x04, 0x04, 0x78],
        'i' => &[0x00, 0x44, 0x7d, 0x40, 0x00],
        'j' => &[0x20, 0x40, 0x44, 0x3d, 0x00],
        'k' => &[0x7f, 0x10, 0x28, 0x44, 0x00],
        'l' => &[0x00, 0x41, 0x7f, 0x40, 0x00],
        'm' => &[0x7c, 0x04, 0x18, 0x04, 0x78],
        'n' => &[0x7c, 0x08, 0x04, 0x04, 0x78],
        'o' => &[0x38, 0x44, 0x44, 0x44, 0x38],
        'p' => &[0x7c, 0x14, 0x14, 0x14, 0x08],
        'q' => &[0x08, 0x14, 0x14, 0x18, 0x7c],
        'r' => &[0x7c, 0x08, 0x04, 0x04, 0x08],
        's' => &[0x48, 0x54, 0x54, 0x54, 0x20],
        't' => &[0x04, 0x3f, 0x44, 0x40, 0x20],
        'u' => &[0x3c, 0x40, 0x40, 0x20, 0x7c],
        'v' => &[0x1c, 0x20, 0x40, 0x20, 0x1c],
        'w' => &[0x3c, 0x40, 0x30, 0x40, 0x3c],
        'x' => &[0x44, 0x28, 0x10, 0x28, 0x44],
        'y' => &[0x0c, 0x50, 0x50, 0x50, 0x3c],
        'z' => &[0x44, 0x64, 0x54, 0x4c, 0x44],
        '{' => &[0x00, 0x08, 0x36, 0x41, 0x00],
        '|' => &[0x00, 0x00, 0x7f, 0x00, 0x00],
        '}' => &[0x00, 0x41, 0x36, 0x08, 0x00],
        '←' => &[0x10, 0x08, 0x08, 0x10, 0x08],
        '→' => &[0x78, 0x46, 0x41, 0x46, 0x78],
        '°' => &[0x00, 0x02, 0x05, 0x02, 0x00],
        _ => &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    }
}

impl<SPI, DC, CE, RST, LIGHT> Write for PCD8544<SPI, DC, CE, RST, LIGHT>
where
    SPI: blocking::spi::Write<u8>,
    DC: OutputPin,
    CE: OutputPin,
    RST: OutputPin,
    LIGHT: OutputPin,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for char in s.chars() {
            match char {
                '\r' => {
                    self.set_x_position(0).map_err(|_| fmt::Error {})?;
                }
                '\n' => {
                    for _ in 0..(WIDTH - self.x) {
                        self.write_data(0x00).map_err(|_| fmt::Error {})?;
                    }
                }
                _ => {
                    for b in char_to_bytes(char) {
                        self.write_data(*b).map_err(|_| fmt::Error {})?;
                    }
                    self.write_data(0x00).map_err(|_| fmt::Error {})?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BadPin {}

    impl BadPin {
        fn new() -> BadPin {
            BadPin {}
        }
    }

    struct BadPinError {}

    impl OutputPin for BadPin {
        type Error = BadPinError;
        fn set_low(&mut self) -> Result<(), Self::Error> {
            Err(BadPinError {})
        }
        fn set_high(&mut self) -> Result<(), Self::Error> {
            Err(BadPinError {})
        }
    }

    #[test]
    fn errors_basic() {
        let pcd8544_result = PCD8544::new(
            BadPin::new(),
            BadPin::new(),
            BadPin::new(),
            BadPin::new(),
            BadPin::new(),
            BadPin::new(),
        );
        assert!(pcd8544_result.is_err());
    }
}
