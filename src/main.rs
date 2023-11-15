use std::collections::HashMap;
use std::{fs, thread};

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
use crate::models::common::Markets;


fn main() {
    // get env variables
    let vars: EnvVars = env::env_variables();
    env::init_logger(vars.log_level);


    let markets_config = fs::read_to_string("src/config/markets.json")
        .expect("Unable to read markets.json");
    let markets: Markets = serde_json::from_str(&markets_config)
        .expect("JSON was not well-formatted");

    // Vector to store thread handles
    let mut handles = vec![];

    // Create and store threads for each market
    for market_map in vec![markets.eth, markets.btc] {
        let handle = thread::spawn(move || {
            MM::new(market_map).connect();
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("market maker thread failed to join main");
    }

}
