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

#[tokio::main]
async fn main() {
    // get env variables
    let vars: EnvVars = env::env_variables();
    env::init_logger(vars.log_level);

    // create bluefin client
    let client = BluefinClient::new(
        &vars.bluefin_wallet_key,
        &vars.bluefin_endpoint,
        &vars.bluefin_on_boarding_url,
        &vars.bluefin_websocket_url,
        vars.bluefin_leverage,
    )
    .await;

    // start connector
    let handle_mm = thread::spawn(move || {
        MM {}.connect();
    });

    // start bluefin event listener
    client.listen_to_web_socket().await;

    handle_mm
        .join()
        .expect("market maker thread failed to join main");
}
