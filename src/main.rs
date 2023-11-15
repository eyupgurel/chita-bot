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


    // for every market we have a market maker living in a separate thread (will be moved to configuration)
    let handle_eth_mm = thread::spawn(move || {
        let market_map = HashMap::from([
            ("binance", "ethusdt"),
            ("kucoin", "ETHUSDTM"),
            ("bluefin", "ETH-PERP"),
        ]);
        MM::new(market_map).connect();
    });

/*    let handle_btc_mm = thread::spawn(move || {
        let market_map = HashMap::from([
            ("binance", "btcusdt"),
            ("kucoin", "XBTUSDTM"),
            ("bluefin", "BTC-PERP"),
        ]);
        MM::new(market_map).connect();
    });*/

    handle_eth_mm
        .join()
        .expect("market maker thread failed to join main");

/*    handle_btc_mm
        .join()
        .expect("market maker thread failed to join main");*/

}
