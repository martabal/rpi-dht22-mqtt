use rumqttc::TlsConfiguration;
use std::{error::Error, fs::File, io::Read};
use tracing::trace;

/// # Errors
/// Returns the error if there's no certificate or if it's unvalid
pub fn load_certs(
    ca_cert_path_optional: Option<String>,
    client_key_path_optional: Option<String>,
    client_cert_path_optional: Option<String>,
) -> Result<Option<TlsConfiguration>, Box<dyn Error>> {
    ca_cert_path_optional.map_or_else(
        || Ok(None),
        |ca_cert_path| {
            trace!("Loading cert from {ca_cert_path}");
            let mut ca_cert = vec![];
            File::open(ca_cert_path)?.read_to_end(&mut ca_cert)?;

            let client_auth = if let (Some(client_key_path), Some(client_cert_path)) =
                (client_key_path_optional, client_cert_path_optional)
            {
                trace!("Using mtls");
                let mut client_key = vec![];
                File::open(client_key_path)?.read_to_end(&mut client_key)?;

                let mut client_cert = vec![];
                File::open(client_cert_path)?.read_to_end(&mut client_cert)?;
                Some((client_cert, client_key))
            } else {
                None
            };

            let tls_config = TlsConfiguration::Simple {
                ca: ca_cert,
                alpn: None,
                client_auth,
            };

            Ok(Some(tls_config))
        },
    )
}
