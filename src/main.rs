use crate::connector::converge::converge;

mod bluefin;
mod connector;
mod env;
mod models;
mod sockets;
mod tests;
mod constants;

use bluefin::{BluefinClient, ClientMethods};
use env::EnvVars;

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

    // start bluefin event listener
    // this will block the main thread and converge method will never run!
    client.listen_to_web_socket().await;

    // start connector
    converge().await;
}
