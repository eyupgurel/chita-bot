use crate::connector::converge::converge;

mod bluefin;
mod connector;
mod env;
mod models;
mod sockets;
mod tests;

use bluefin::{BluefinClient, ClientMethods};
use env::EnvVars;

#[tokio::main]
async fn main() {
    // get env variables
    let vars: EnvVars = env::env_variables();
    env::init_logger(vars.log_level);

    // create bluefin client
    let _client = BluefinClient::init(
        &vars.bluefin_wallet_key,
        &vars.bluefin_endpoint,
        &vars.bluefin_on_boarding_url,
    )
    .await;

    // start connector
    converge().await;
}
