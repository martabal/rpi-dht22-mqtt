//! This is a Rust API to obtain temperature and humidity measurements from a DHT22 connected to
//! a Raspberry Pi.
//!
//! This library is essentially a port of the
//! [Adafruit_Python_DHT](https://github.com/adafruit/Adafruit_Python_DHT) library from C to Rust.  
//!
//! This library has been tesed on a DHT22 from Adafruit using a Raspberry Pi Module B+.
//!
use rppal::gpio::{Gpio, Level, Mode};

use std::{
    ptr::{read_volatile, write_volatile},
    thread::sleep,
    time::Duration,
};

use crate::ReadingError;

/// A temperature and humidity reading from the DHT22.
#[derive(Debug, Clone, Copy)]
pub struct Reading {
    pub temperature: f32,
    pub humidity: f32,
}

impl From<rppal::gpio::Error> for ReadingError {
    fn from(err: rppal::gpio::Error) -> Self {
        Self::Gpio(err)
    }
}

const MAX_COUNT: usize = 32000;
const DHT_PULSES: usize = 41;

fn tiny_sleep() {
    let mut i = 0;
    unsafe {
        while read_volatile(&i) < 50 {
            write_volatile(&mut i, read_volatile(&i) + 1);
        }
    }
}

/// # Errors
/// Returns a `ReadingError` if there's an error when reading.
fn decode(arr: &[usize; DHT_PULSES * 2]) -> Result<Reading, ReadingError> {
    let mut threshold: usize = 0;

    let mut i = 2;
    while i < DHT_PULSES * 2 {
        threshold += arr[i];

        i += 2;
    }

    threshold /= DHT_PULSES - 1;

    let mut data = [0_u8; 5];
    let mut i = 3;
    while i < DHT_PULSES * 2 {
        let index = (i - 3) / 16;
        data[index] <<= 1;
        if arr[i] >= threshold {
            data[index] |= 1;
        } else {
            // else zero bit for short pulse
        }

        i += 2;
    }

    if data[4]
        != data[0]
            .wrapping_add(data[1])
            .wrapping_add(data[2])
            .wrapping_add(data[3])
    {
        return Result::Err(ReadingError::Checksum);
    }

    let h_dec = u16::from(data[0]) * 256 + u16::from(data[1]);
    let h = f32::from(h_dec) / 10.0f32;

    let t_dec = u16::from(data[2] & 0x7f) * 256 + u16::from(data[3]);
    let mut t = f32::from(t_dec) / 10.0f32;
    if (data[2] & 0x80) != 0 {
        t *= -1.0f32;
    }

    Result::Ok(Reading {
        temperature: t,
        humidity: h,
    })
}

/// Read temperature and humidity from a DHT22 connected to a Gpio pin on a Raspberry Pi.
///
/// On a Raspberry Pi this is implemented using bit-banging which is very error-prone.  It will
/// fail 30% of the time.  You should write code to handle this.  In addition you should not
/// attempt a reading more frequently than once every 2 seconds because the DHT22 hardware does
/// not support that.
///
/// # Errors
/// Returns a `ReadingError` if there's an error when reading.
pub fn read(pin: u8) -> Result<Reading, ReadingError> {
    let mut gpio = match Gpio::new() {
        Err(e) => return Err(ReadingError::Gpio(e)),
        Ok(g) => match g.get(pin) {
            Err(e) => return Err(ReadingError::Gpio(e)),
            Ok(pin) => pin.into_io(Mode::Output),
        },
    };

    let mut pulse_counts: [usize; DHT_PULSES * 2] = [0; DHT_PULSES * 2];

    gpio.write(Level::High);
    sleep(Duration::from_millis(500));

    gpio.write(Level::Low);
    sleep(Duration::from_millis(20));

    gpio.set_mode(Mode::Input);

    // Sometimes the pin is briefly low.
    tiny_sleep();

    let mut count: usize = 0;

    while gpio.read() == Level::High {
        count += 1;

        if count > MAX_COUNT {
            return Result::Err(ReadingError::Timeout);
        }
    }

    for c in 0..DHT_PULSES {
        let i = c * 2;

        while gpio.read() == Level::Low {
            pulse_counts[i] += 1;

            if pulse_counts[i] > MAX_COUNT {
                return Result::Err(ReadingError::Timeout);
            }
        }

        while gpio.read() == Level::High {
            pulse_counts[i + 1] += 1;

            if pulse_counts[i + 1] > MAX_COUNT {
                return Result::Err(ReadingError::Timeout);
            }
        }
    }

    decode(&pulse_counts)
}

#[cfg(test)]
mod tests {
    use super::decode;
    use super::ReadingError;

    #[test]
    fn from_spec_positive_temp() {
        let arr = [
            80, // initial 80us low period
            80, // initial 80us high period
            // humidity
            50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 70, 50, 26, 50, 70, 50, 26, 50, 26,
            50, 26, 50, 70, 50, 70, 50, 26, 50, 26, // temp
            50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 70, 50, 26, 50, 70, 50, 26,
            50, 70, 50, 70, 50, 70, 50, 70, 50, 70, // checksum
            50, 70, 50, 70, 50, 70, 50, 26, 50, 70, 50, 70, 50, 70, 50, 26,
        ];

        let x = decode(&arr).unwrap();
        assert!(x.humidity == 65.2);
        assert!(x.temperature == 35.1);
    }

    #[test]
    fn from_spec_negative_temp() {
        let arr = [
            80, // initial 80us low period
            80, // initial 80us high period
            // humidity
            50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 70, 50, 26, 50, 70, 50, 26, 50, 26,
            50, 26, 50, 70, 50, 70, 50, 26, 50, 26, // temp
            50, 70, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 70, 50, 70,
            50, 26, 50, 26, 50, 70, 50, 26, 50, 70, // checksum
            50, 26, 50, 70, 50, 70, 50, 70, 50, 26, 50, 26, 50, 70, 50, 70,
        ];

        let x = decode(&arr).unwrap();
        assert!(x.humidity == 65.2);
        assert!(x.temperature == -10.1);
    }

    #[test]
    fn checksum() {
        let arr = [
            80, // initial 80us low period
            80, // initial 80us high period
            // humidity
            50, 26, 50, 26, 50, 26, 50, 26, 50, 70, 50, 26, 50, 70, 50, 26, 50, 70, 50, 26, 50, 26,
            50, 26, 50, 70, 50, 70, 50, 26, 50, 26, // temp
            50, 70, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 26, 50, 70, 50, 70,
            50, 26, 50, 26, 50, 70, 50, 26, 50, 70, // checksum
            50, 26, 50, 70, 50, 70, 50, 70, 50, 26, 50, 26, 50, 70, 50, 70,
        ];

        match decode(&arr) {
            Ok(_) => {
                panic!("should have failed");
            }
            Err(e) => {
                match e {
                    ReadingError::Checksum => {
                        // ok
                    }
                    _ => {
                        panic!("should have Checksum, got {e:?} instead");
                    }
                }
            }
        }
    }

    #[test]
    fn sample1() {
        let arr = [
            458, // initial 80us low period
            328, // initial 80us high period
            // humidity
            320, 101, 249, 153, 314, 153, 320, 154, 317, 153, 316, 153, 321, 431, 320, 147, 397,
            154, 315, 435, 316, 154, 320, 431, 320, 430, 319, 431, 320, 431, 320, 426,
            // temperature
            401, 148, 319, 154, 316, 154, 320, 150, 320, 154, 315, 154, 320, 149, 320, 148, 397,
            154, 319, 430, 321, 430, 321, 431, 320, 429, 318, 432, 320, 150, 320, 147,
            // checksum
            379, 434, 316, 434, 317, 153, 320, 431, 317, 435, 316, 435, 317, 153, 320, 425,
        ];

        let x = decode(&arr).unwrap();
        assert_eq!(x.humidity, 60.7);
        assert_eq!(x.temperature, 12.4);
    }
}
