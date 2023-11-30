use std::thread::JoinHandle;
use std::{fs, panic, process, thread};

mod bluefin;
mod env;
mod hedge;
mod kucoin;
mod market_maker;
mod models;
mod sockets;
mod statistics;
mod tests;
mod utils;

mod circuit_breakers;

use crate::market_maker::mm::{MarketMaker, MM};

use crate::hedge::hedger::{Hedger, HGR};
use crate::models::common::Config;
use crate::statistics::stats::{Statistics, Stats};
use env::EnvVars;
use crate::statistics::account_stats::{AccountStatistics, AccountStats};

fn main() {
    // Set a custom global panic hook
    panic::set_hook(Box::new(|info| {
        // Log the panic information
        tracing::error!("Panic occurred: {:?}", info);

        // Exit with a non-zero status code to indicate error
        process::exit(1);
    }));

    // get env variables
    let vars: EnvVars = env::env_variables();

    // initialize trace logger and hold on to the guard. Without the guard
    // the logs won't be written to log file
    let _guard = env::init_logger(vars.log_level);

    let config_str =
        fs::read_to_string("src/config/config.json").expect("Unable to read config.json");
    let config: Config = serde_json::from_str(&config_str).expect("JSON was not well-formatted");

    // Create and collect thread handles using an iterator
    let (mm_handles, statistic_handles): (Vec<JoinHandle<()>>, Vec<JoinHandle<()>>) = config
        .markets
        .clone()
        .into_iter()
        .map(|market| {
            let (mut mm, tx_stats) = MM::new(market.clone(), config.circuit_breaker_config.clone()); // get MM instance and tx_stats

            // Spawn one thread for MM
            let mm_handle = thread::spawn(move || {
                mm.connect();
            });

            // Spawn another thread for Stats, passing tx_stats
            let stats_handle = thread::spawn(move || {
                Stats::new(market, tx_stats).emit();
            });

            (mm_handle, stats_handle)
        })
        .unzip();

    let hgr_handles: Vec<JoinHandle<()>> = config
        .markets
        .clone()
        .into_iter()
        .map(|market| {
            thread::spawn(move || {
                HGR::new(market, config.circuit_breaker_config.clone()).connect();
            })
        })
        .collect();

    let account_stats_handle = thread::spawn(move || {
        AccountStats::new().log();
    });

    let mut combined_handles = mm_handles;
    combined_handles.extend(hgr_handles);
    combined_handles.extend(statistic_handles);


    // Wait for all threads to complete in one pass
    combined_handles.into_iter().for_each(|handle| {
        handle.join().expect("Thread failed to join main");
    });

    account_stats_handle.join().expect("Thread failed to join main");
}
