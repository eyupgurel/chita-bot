use std::collections::HashMap;
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


    // Consolidate market data into a single HashMap
    let markets = HashMap::from([
        ("ETH", HashMap::from([
            ("binance", "ethusdt"),
            ("kucoin", "ETHUSDTM"),
            ("bluefin", "ETH-PERP"),
        ])),
        ("BTC", HashMap::from([
            ("binance", "btcusdt"),
            ("kucoin", "XBTUSDTM"),
            ("bluefin", "BTC-PERP"),
        ])),
        // Add more markets as needed
    ]);

    // Vector to store thread handles
    let mut handles = vec![];

    // Create and store threads for each market
    for (_currency, market_map) in markets {
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
