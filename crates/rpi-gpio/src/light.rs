use rppal::gpio::Gpio;

use crate::ReadingError;

/// # Errors
/// Returns a `ReadingError` if there's an error when reading.
pub fn read(pin_num: u8) -> Result<bool, ReadingError> {
    let gpio = Gpio::new().map_err(ReadingError::Gpio)?;
    let pin = gpio.get(pin_num).map_err(ReadingError::Gpio)?.into_input();
    Ok(pin.is_high())
}
