use std::time::Duration;
use log::{info, warn};
use tokio::time::Instant;
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{Credentials, KuCoinClient};
use crate::models::common::CircuitBreakerConfig;


pub struct KuCoinBreaker {
    pub client: KuCoinClient,
    pub env_vars: EnvVars
}

impl KuCoinBreaker {

    fn client(
        kucoin_api_key: &str,
        kucoin_api_secret: &str,
        kucoin_api_phrase: &str,
        kucoin_endpoint: &str,
        kucoin_on_boarding_url: &str,
        kucoin_websocket_url: &str,
        kucoin_leverage: u128) -> KuCoinClient {

        KuCoinClient::new(
            Credentials::new(
                kucoin_api_key,
                kucoin_api_secret,
                kucoin_api_phrase,
            ),
            kucoin_endpoint,
            kucoin_on_boarding_url,
            kucoin_websocket_url,
            kucoin_leverage,
        )

    }

    pub fn new() -> KuCoinBreaker {
        let vars= env::env_variables();
        KuCoinBreaker {
            client: KuCoinBreaker::client(
                &vars.kucoin_api_key,
                &vars.kucoin_api_secret,
                &vars.kucoin_api_phrase,
                &vars.kucoin_endpoint,
                &vars.kucoin_on_boarding_url,
                &vars.kucoin_websocket_url,
                vars.kucoin_leverage,
            ),
            env_vars: vars
        }

    }


    pub fn cancel_all_orders(&mut self, cb_config: &CircuitBreakerConfig, market: &String, dry_run: bool) -> bool {
        if dry_run {
            return true;
        }

        let mut resp = self.client.cancel_all_orders(Some(market.as_str()));

        let mut retry: u8 = 0;
        let mut last_retry = Instant::now();

        while retry < cb_config.num_retries && resp.error.is_some() {
            if last_retry.elapsed() >= Duration::from_millis(self.env_vars.market_making_time_throttle_period) {
                warn!("KuCoin cancel all orders failed. Retrying {} times, current retry: {}", cb_config.num_retries, retry + 1);
                resp = self.client.cancel_all_orders(Some(market.as_str()));
                retry += 1;
                last_retry = Instant::now();
            }
        }
        return if retry == cb_config.num_retries {
            warn!("Retry number exceeded. Could not cancel orders on KuCoin after Circuit Breaker opened.");
            false
        } else {
            info!("Successfully cancelled all orders after Circuit Breaker opened");
            true
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    //WILL CANCEL ACTUAL ORDERS ON ETH
    fn breaker_cancel_orders() {
        //use env var to setup the client in the breaker
        let vars = env::env_variables();

        let mut breaker = KuCoinBreaker {
            client: KuCoinBreaker::client(
                &vars.kucoin_api_key,
                &vars.kucoin_api_secret,
                &vars.kucoin_api_phrase,
                &vars.kucoin_endpoint,
                &vars.kucoin_on_boarding_url,
                &vars.kucoin_websocket_url,
                vars.kucoin_leverage,
            ),
            env_vars: vars,
        };
        let config = CircuitBreakerConfig {
            num_retries: 3,
            failure_threshold: 3,
        };
        let market = "ETH-PERP".to_string();

        assert!(
            breaker.cancel_all_orders(&config, &market, false),
            "Error cancelling all orders for ETH market"
        );
    }
}