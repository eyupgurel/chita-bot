use std::thread::JoinHandle;
use std::{fs, panic, process, thread};
use std::sync::mpsc::Sender;


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
use kucoin::AvailableBalance;
use crate::bluefin::AccountData;
use crate::statistics::account_stats::{AccountStatistics, AccountStats};
use crate::bluefin::TradeOrderUpdate;

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

    let mut v_tx_account_data: Vec<Sender<AccountData>> = Vec::new();
    let mut v_tx_account_data_kc: Vec<Sender<AvailableBalance>> = Vec::new();
    let mut v_tx_account_data_bluefin_user_trade: Vec<Sender<TradeOrderUpdate>> = Vec::new();
    let mut mm_handles: Vec<JoinHandle<()>> = Vec::new();
    let mut statistic_handles: Vec<JoinHandle<()>> = Vec::new();
    let mut hgr_handles: Vec<JoinHandle<()>> = Vec::new();

    for market in config.markets.clone() {
        let (
            mut mm, 
            tx_stats, 
            tx_account_data, 
            tx_account_data_kc, 
            tx_hedger,
            rx_bluefin_hedger_ob,
            tx_account_data_bluefin_user_trade,
        ) = MM::new(market.clone(), config.circuit_breaker_config.clone());

        let mm_handle = thread::spawn(move || {
            mm.connect();
        });

        let market_clone_for_stats = market.clone(); // Clone market for the stats thread

        let stats_handle = thread::spawn(move || {
            Stats::new(market_clone_for_stats.clone(), tx_stats).emit();
        });

        v_tx_account_data.push(tx_account_data);
        v_tx_account_data_kc.push(tx_account_data_kc);
        v_tx_account_data_bluefin_user_trade.push(tx_account_data_bluefin_user_trade);

        let market_clone_for_hgr = market.clone(); // Clone market again for the hedger thread

        let hgr_handle = thread::spawn(move || {
            HGR::new(
                market_clone_for_hgr.clone(), 
                config.circuit_breaker_config.clone(), 
                tx_hedger, 
                rx_bluefin_hedger_ob
            ).connect();
        });

        mm_handles.push(mm_handle);
        statistic_handles.push(stats_handle);
        hgr_handles.push(hgr_handle);
    }



    let account_stats_handle = thread::spawn(move || {
        AccountStats::new(config, v_tx_account_data, v_tx_account_data_kc, v_tx_account_data_bluefin_user_trade).log();
    });

    let mut combined_handles = mm_handles;
    combined_handles.extend(statistic_handles);
    combined_handles.extend(hgr_handles);


    // Wait for all threads to complete in one pass
    combined_handles.into_iter().for_each(|handle| {
        handle.join().expect("Thread failed to join main");
    });

    account_stats_handle.join().expect("Thread failed to join main");
}
