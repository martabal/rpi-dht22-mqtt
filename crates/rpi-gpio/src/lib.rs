pub mod dht22;
pub mod light;
pub mod tls;

/// Errors that may occur when reading temperature.
#[derive(Debug)]
pub enum ReadingError {
    /// Occurs if a timeout occured reading the pin.
    Timeout,

    /// Occurs if the checksum value from the DHT22 is incorrect.
    Checksum,

    /// Occurs if there is a problem accessing gpio itself on the Raspberry PI.
    Gpio(rppal::gpio::Error),
}
