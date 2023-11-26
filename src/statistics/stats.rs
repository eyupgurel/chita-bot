use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use log::info;
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{Credentials, KuCoinClient};
use crate::models::common::Market;

pub struct Stats{
    market: Market,
    kucoin_client: KuCoinClient,
    tx_stats: Sender<f64>,
    genesis:SystemTime
}

impl Stats {
    pub fn new(market: Market, tx_stats:Sender<f64>) -> Stats{
        let vars: EnvVars = env::env_variables();
        let kucoin_client = KuCoinClient::new(
            Credentials::new(
                &vars.kucoin_api_key,
                &vars.kucoin_api_secret,
                &vars.kucoin_api_phrase,
            ),
            &vars.kucoin_endpoint,
            &vars.kucoin_on_boarding_url,
            &vars.kucoin_websocket_url,
            vars.kucoin_leverage,
        );

        Stats {
            market,
            kucoin_client,
            tx_stats,
            genesis:SystemTime::now()
        }
    }

}

pub trait Statistics {
    fn emit(&self);
}

impl Statistics for Stats {
    fn emit(&self) {
        loop{
            let service_start = self.genesis.duration_since(UNIX_EPOCH).expect("could not get current time since unix epoch").as_millis();
            let bluefin_market = self.market.symbols.bluefin.to_owned();
            let total_buy_size = self.kucoin_client.get_fill_size_for_time_window(&bluefin_market, "buy", service_start);
            let total_sell_size = self.kucoin_client.get_fill_size_for_time_window(&bluefin_market, "sell", service_start);
            let buy_percent = if total_buy_size + total_sell_size == 0 {50.0} else  {(total_buy_size as f64 / ((total_buy_size + total_sell_size) as f64)) * 100.0};
            info!("buy percent:{}", buy_percent);
            self.tx_stats.send(buy_percent).expect("Error in sending stats");

            thread::sleep(Duration::from_secs(60));
        }
    }
}