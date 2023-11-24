use std::thread::JoinHandle;
use std::{fs, panic, process, thread};

mod bluefin;
mod env;
mod hedge;
mod kucoin;
mod market_maker;
mod models;
mod sockets;
mod tests;
mod utils;

use crate::market_maker::mm::{MarketMaker, MM};

use crate::hedge::hedger::{Hedger, HGR};
use crate::models::common::Config;
use env::EnvVars;

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

    let config_str =
        fs::read_to_string("src/config/config.json").expect("Unable to read config.json");
    let config: Config = serde_json::from_str(&config_str).expect("JSON was not well-formatted");

    // Create and collect thread handles using an iterator
    let mm_handles: Vec<JoinHandle<()>> = config
        .markets
        .clone()
        .into_iter()
        .map(|market| {
            thread::spawn(move || {
                MM::new(market).connect();
            })
        })
        .collect();

    let hgr_handles: Vec<JoinHandle<()>> = config
        .markets
        .clone()
        .into_iter()
        .map(|market| {
            thread::spawn(move || {
                HGR::new(market).connect();
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
