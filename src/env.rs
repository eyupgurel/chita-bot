use dotenv::dotenv;
use env_logger::{Builder, WriteStyle};
use log::LevelFilter;

// env variable struct
pub struct EnvVars {
    pub bluefin_on_boarding_url: String,
    pub bluefin_websocket_url: String,
    pub bluefin_endpoint: String,
    pub bluefin_wallet_key: String,
    pub bluefin_leverage: u128,
    pub log_level: String,
}

/**
 * Method to parse environment variables
 */
#[allow(dead_code)]
pub fn env_variables() -> EnvVars {
    dotenv().ok();

    let bluefin_on_boarding_url =
        std::env::var("BLUEFIN_ON_BOARDING_URL").expect("BLUEFIN_ON_BOARDING_URL must be set.");
    let bluefin_endpoint =
        std::env::var("BLUEFIN_ENDPOINT").expect("BLUEFIN_ENDPOINT must be set.");
    let bluefin_wallet_key =
        std::env::var("BLUEFIN_WALLET_KEY").expect("BLUEFIN_WALLET_KEY must be set.");
    let bluefin_websocket_url =
        std::env::var("BLUEFIN_WEB_SOCKET_URL").expect("BLUEFIN_WEB_SOCKET_URL must be set.");
    let bluefin_leverage = std::env::var("BLUEFIN_LEVERAGE")
        .expect("BLUEFIN_LEVERAGE must be set.")
        .parse::<u128>()
        .unwrap();
    let log_level = std::env::var("LOG_LEVEL").expect("LOG_LEVEL must be set.");

    return EnvVars {
        bluefin_on_boarding_url,
        bluefin_endpoint,
        bluefin_wallet_key,
        bluefin_websocket_url,
        bluefin_leverage,
        log_level,
    };
}

/**
 * Initializes logger with provided log level
 */
#[allow(dead_code)]
pub fn init_logger(log_level: String) {
    let mut builder: Builder = Builder::new();

    let filter: LevelFilter = match log_level.as_str() {
        "Error" => LevelFilter::Error,
        "Warn" => LevelFilter::Warn,
        "Debug" => LevelFilter::Debug,
        "Trace" => LevelFilter::Trace,
        "Off" => LevelFilter::Off,
        "Info" | _ => LevelFilter::Info,
    };

    builder
        .filter(None, filter)
        .write_style(WriteStyle::Always)
        .init();
}
