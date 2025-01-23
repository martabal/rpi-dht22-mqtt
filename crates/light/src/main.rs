use rpi_gpio::{light::read, tls::load_certs};
use rumqttc::{
    v5::{mqttbytes::QoS, AsyncClient, Event, MqttOptions},
    Transport,
};
use serde_json::json;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, level_filters::LevelFilter, trace};
use tracing_subscriber::EnvFilter;

use std::{env, error::Error, path::Path, time::Duration};

fn not_set(env: &str) -> String {
    format!("{env} not set")
}

const PIN: &str = "LIGHT_PIN";
const MQTT_CLIENT_ID: &str = "LIGHT_MQTT_CLIENT_ID";
const MQTT_IP: &str = "MQTT_IP";
const MQTT_PORT: &str = "MQTT_PORT";
const MQTT_TOPIC: &str = "LIGHT_MQTT_TOPIC";
const MQTT_USERNAME: &str = "MQTT_USERNAME";
const MQTT_PASSWORD: &str = "MQTT_PASSWORD";
const CERTIFICATE_AUTHORITY_PATH: &str = "CERTIFICATE_AUTHORITY_PATH";
const MTLS_CERT_PATH: &str = "MTLS_CERT_PATH";
const MTLS_PKEY_PATH: &str = "MTLS_PKEY_PATH";

#[allow(clippy::too_many_lines)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let path = Path::new(".env");
    if path.exists() {
        dotenvy::from_path(path).unwrap();
    }

    let client_id = format!(
        "{}-rust",
        env::var(MQTT_CLIENT_ID).unwrap_or_else(|_| panic!("{}", not_set(MQTT_CLIENT_ID)))
    );
    let mqtt_ip = env::var(MQTT_IP).unwrap_or_else(|_| panic!("{}", not_set(MQTT_IP)));
    let mqtt_port = env::var(MQTT_PORT)
        .unwrap_or_else(|_| panic!("{}", not_set(MQTT_PORT)))
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{MQTT_PORT} is not a valid u16"));
    let mqtt_topic = env::var(MQTT_TOPIC).unwrap_or_else(|_| panic!("{}", not_set(MQTT_TOPIC)));
    let mqtt_username =
        env::var(MQTT_USERNAME).unwrap_or_else(|_| panic!("{}", not_set(MQTT_USERNAME)));
    let mqtt_password =
        env::var(MQTT_PASSWORD).unwrap_or_else(|_| panic!("{}", not_set(MQTT_PASSWORD)));
    let pin = env::var(PIN)
        .unwrap_or_else(|_| panic!("{}", not_set(PIN)))
        .parse::<u8>()
        .unwrap_or_else(|_| panic!("{PIN} is not a valid u16"));
    let ca_cert_path: Option<String> = env::var(CERTIFICATE_AUTHORITY_PATH).ok();
    let mtls_cert_path: Option<String> = env::var(MTLS_CERT_PATH).ok();
    let mtls_pkey_path: Option<String> = env::var(MTLS_PKEY_PATH).ok();

    let log_level_str = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    println!("Using log level: {log_level_str}");

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .unwrap()
        .add_directive(format!("rpi_gpio={log_level_str}").parse().unwrap())
        .add_directive(format!("light={log_level_str}").parse().unwrap());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();

    let client_config = load_certs(ca_cert_path, mtls_pkey_path, mtls_cert_path).unwrap();

    let mut interval = interval(Duration::from_secs(1));
    let mut previous: Option<bool> = None;
    loop {
        info!("Connecting to MQTT broker...");

        let mut mqttoptions = MqttOptions::new(&client_id, &mqtt_ip, mqtt_port);
        mqttoptions
            .set_keep_alive(Duration::from_secs(60))
            .set_clean_start(true)
            .set_credentials(&mqtt_username, &mqtt_password);

        if let Some(config) = &client_config {
            info!("Using TLS");
            mqttoptions.set_transport(Transport::tls_with_config(config.clone()));
        }

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 50);

        let event_loop_handle = tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Outgoing(_) | Event::Incoming(_)) => {}
                    Err(e) => {
                        error!("Error in event loop: {:?}", e);
                        break;
                    }
                }
            }
        });

        loop {
            debug!("Is there some light...");
            match read(pin) {
                Ok(light) => {
                    if previous.is_some() && previous == Some(light) {
                        trace!("No change detected");
                    } else {
                        previous = Some(light);
                        let data = json!({
                            "light": light,
                        });
                        debug!(
                            "{}",
                            if light {
                                "there's light!"
                            } else {
                                "there's no light"
                            }
                        );
                        match client
                            .publish(&mqtt_topic, QoS::AtLeastOnce, false, data.to_string())
                            .await
                        {
                            Ok(()) => {
                                debug!("Data published!");
                            }
                            Err(e) => {
                                error!("Failed to publish data: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Is there some light? {:?}", e);
                }
            };
            interval.tick().await;
        }

        if event_loop_handle.await.is_err() {
            error!("Reconnecting after event loop failure...");
        }

        sleep(Duration::from_secs(10)).await;
    }
}
