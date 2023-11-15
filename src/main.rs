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

use env::EnvVars;


fn main() {
    // get env variables
    let vars: EnvVars = env::env_variables();
    env::init_logger(vars.log_level);

    // start connector
    let handle_mm = thread::spawn(move || {
        MM::new().connect();
    });

    handle_mm
        .join()
        .expect("market maker thread failed to join main");
}
