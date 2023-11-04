use dotenv::dotenv;

// env variable struct
pub struct EnvVars {
    pub bluefin_on_boarding_url: String,
    pub bluefin_endpoint: String,
    pub bluefin_wallet_key: String,
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
    return EnvVars {
        bluefin_on_boarding_url,
        bluefin_endpoint,
        bluefin_wallet_key,
    };
}
