use crate::connector::converge::converge;

mod models;
mod r#type;

mod bluefin;
mod connector;
mod env;
mod sockets;
mod tests;

use bluefin::{BluefinClient, ClientMethods};
use env::EnvVars;

#[tokio::main]
async fn main() {
    env_logger::init();

    // get env variables
    let vars: EnvVars = env::env_variables();

    // start connector
    converge().await;

    // create bluefin client
    let _client = BluefinClient::init(
        &vars.bluefin_wallet_key,
        &vars.bluefin_endpoint,
        &vars.bluefin_on_boarding_url,
    )
    .await;
}
