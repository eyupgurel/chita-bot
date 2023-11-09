use std::thread;

mod bluefin;
mod env;
mod models;
mod sockets;
mod tests;
mod constants;
mod market_maker;
use bluefin::{BluefinClient, ClientMethods};
use env::EnvVars;
use crate::market_maker::mm::{MarketMaker, MM};

#[tokio::main]
async fn main() {
    // get env variables
    let vars: EnvVars = env::env_variables();
    env::init_logger(vars.log_level);

    // create bluefin client
    let client = BluefinClient::init(
        &vars.bluefin_wallet_key,
        &vars.bluefin_endpoint,
        &vars.bluefin_on_boarding_url,
        &vars.bluefin_websocket_url,
        vars.bluefin_leverage,
    )
    .await;

    // start connector
    let handle_mm = thread::spawn(move || {
        MM{}.connect();
    });

    // start bluefin event listener
    client.listen_to_web_socket().await;

    handle_mm.join().expect("market maker thread failed to join main");

}
