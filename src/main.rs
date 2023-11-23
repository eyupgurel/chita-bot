use std::{fs, panic, process, thread};
use std::thread::JoinHandle;

mod bluefin;
mod env;
mod kucoin;
mod market_maker;
mod models;
mod sockets;
mod tests;
mod utils;
mod hedge;
mod config;

use crate::market_maker::mm::{MarketMaker, MM};

use env::EnvVars;
use crate::hedge::hedger::{Hedger, HGR};
use crate::models::common::Markets;


fn main() {
    // Set a custom global panic hook
    panic::set_hook(Box::new(|info| {
        // Log the panic information
        eprintln!("Panic occurred: {:?}", info);

        // Exit with a non-zero status code to indicate error
        process::exit(1);
    }));


    // get env variables
    let vars: EnvVars = env::env_variables();
    env::init_logger(vars.log_level);


    let markets_config = fs::read_to_string("src/config/markets.json")
        .expect("Unable to read markets.json");
    let markets: Markets = serde_json::from_str(&markets_config)
        .expect("JSON was not well-formatted");

    // Create and collect thread handles using an iterator
    let mm_handles: Vec<JoinHandle<()>> = markets.markets.clone().into_iter()
        .map(|(_currency, market_map)| {
            thread::spawn(move || {
                MM::new(market_map).connect();
            })
        })
        .collect();

    let hgr_handles: Vec<JoinHandle<()>> = markets.markets.clone().into_iter()
        .map(|(_currency, market_map)| {
            thread::spawn(move || {
                HGR::new(market_map).connect();
            })
        })
        .collect();


    let mut combined_handles = mm_handles;
    combined_handles.extend(hgr_handles);

    // Wait for all threads to complete in one pass
    combined_handles.into_iter().for_each(|handle| {
        handle.join().expect("Thread failed to join main");
    });
}
