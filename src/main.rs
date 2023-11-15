use std::thread;

mod bluefin;
mod constants;
mod env;
mod kucoin;
mod market_maker;
mod models;
mod sockets;
mod tests;
mod utils;
use crate::market_maker::mm::{MarketMaker, MM};
use bluefin::BluefinClient;
use env::EnvVars;
use kucoin::{Credentials, KuCoinClient};

fn main() {
    // get env variables
    let vars: EnvVars = env::env_variables();
    env::init_logger(vars.log_level);

    let _ = BluefinClient::new(
        &vars.bluefin_wallet_key,
        &vars.bluefin_endpoint,
        &vars.bluefin_on_boarding_url,
        &vars.bluefin_websocket_url,
        vars.bluefin_leverage,
    );

    let _ = KuCoinClient::new(
        Credentials::new(
            &vars.kucoin_api_key,
            &vars.kucoin_api_secret,
            &vars.kucoin_api_phrase,
        ),
        &vars.kucoin_endpoint,
        &vars.kukoin_on_boarding_url,
        &vars.kucoin_websocket_url,
        vars.kucoin_leverage,
    );

    // start connector
    let handle_mm = thread::spawn(move || {
        MM {}.connect();
    });

    handle_mm
        .join()
        .expect("market maker thread failed to join main");
}
