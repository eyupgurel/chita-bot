use crate::bluefin::{parse_user_position, BluefinClient, UserPosition};
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{Credentials, KuCoinClient};
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;
use log::{debug, info};
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Div;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
static BIGNUMBER_BASE: f64 = 1000000000000000000.0;
static HEDGE_PERIOD_DURATION: u64 = 1;

pub struct HGR {
    pub market_map: HashMap<String, String>,
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
    bluefin_position: UserPosition,
}

impl HGR {
    pub fn new(market_map: HashMap<String, String>) -> HGR {
        let vars: EnvVars = env::env_variables();

        let bluefin_client = BluefinClient::new(
            &vars.bluefin_wallet_key,
            &vars.bluefin_endpoint,
            &vars.bluefin_on_boarding_url,
            &vars.bluefin_websocket_url,
            vars.bluefin_leverage,
        );

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

        let bluefin_market = market_map
            .get("bluefin")
            .expect("Bluefin key not found")
            .to_owned();
        let bluefin_position = bluefin_client.get_user_position(&bluefin_market);

        HGR {
            market_map,
            bluefin_client,
            kucoin_client,
            bluefin_position,
        }
    }
}

pub trait Hedger {
    fn connect(&mut self);
    fn hedge(&mut self);
    fn hedge_pos(&mut self);
}

impl Hedger for HGR {
    fn connect(&mut self) {
        let vars: EnvVars = env::env_variables();
        let (tx_bluefin_pos_change, rx_bluefin_pos_change) = mpsc::channel();

        let bluefin_market = self
            .market_map
            .get("bluefin")
            .expect("Bluefin key not found")
            .to_owned();
        let bluefin_market_for_order_fill = bluefin_market.clone();
        let bluefin_auth_token = self.bluefin_client.auth_token.clone();
        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_of = thread::spawn(move || {
            stream_bluefin_private_socket(
                &bluefin_websocket_url,
                &bluefin_market_for_order_fill,
                &bluefin_auth_token,
                "PositionUpdate",
                tx_bluefin_pos_change, // Sender channel of the appropriate type
                |msg: &str| -> UserPosition {
                    debug!("UserPosition essence:{}", msg);
                    let v: Value = serde_json::from_str(&msg).unwrap();
                    let message = parse_user_position(v["data"]["position"].clone());
                    message
                },
            );
        });
        thread::sleep(Duration::from_secs(5));
        self.hedge();
        loop {
            match rx_bluefin_pos_change.try_recv() {
                Ok(value) => {
                    info!("position change: {:?}", value);
                    //self.bluefin_position = value;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Bluefin worker has disconnected!");
                }
            }
        }
    }

    fn hedge(&mut self) {
        loop {
            self.hedge_pos();
            // Sleep for one second before next iteration
            thread::sleep(Duration::from_secs(HEDGE_PERIOD_DURATION));
        }
    }
    fn hedge_pos(&mut self) {
        let bluefin_market = self
            .market_map
            .get("bluefin")
            .expect("Bluefin key not found")
            .to_owned();

        // the call now returns Option(UserPosition)
        let kucoin_position = self.kucoin_client.get_position(&bluefin_market);

        // if we are unable to get KuCoin position just return
        // this is possible due to rate limiting
        if kucoin_position.is_none() {
            return;
        }

        // unwrap kucoin position and get quantity
        let kucoin_quantity = kucoin_position.unwrap().quantity;

        let current_kucoin_qty: f64 = (kucoin_quantity).div(100.0); // this is only valid for ETH parameterize it through config
        info!("kucoin quantity:{}", current_kucoin_qty);

        let target_quantity = current_kucoin_qty * -1.0;
        let mut bluefin_quantity = (self.bluefin_position.quantity as f64).div(BIGNUMBER_BASE);

        if !self.bluefin_position.side {
            bluefin_quantity = bluefin_quantity * -1.0;
        }
        info!("bluefin quantity:{}", bluefin_quantity);
        let diff = target_quantity - bluefin_quantity;

        let order_quantity = diff.abs();

        let big_target = (order_quantity * BIGNUMBER_BASE) as u128;

        let modulus = big_target % 100;

        let trunk = big_target - modulus;

        let rounded_order_quantity = (trunk as f64).div(BIGNUMBER_BASE);

        let is_buy = diff.is_sign_positive();

        let rv = (rounded_order_quantity * 100.0).round() / 100.0;
        info!("Order quantity:{} is buy:{}", rv, is_buy);

        if rv >= 0.01
        /* Min quantity for ETH (will need to take this value from config for BTC this will not work! */
        {
            let order =
                self.bluefin_client
                    .create_market_order(&bluefin_market, is_buy, false, rv, None);
            info!("Order: {:#?}", order);
            let signature = self.bluefin_client.sign_order(order.clone());
            let status = self
                .bluefin_client
                .post_signed_order(order.clone(), signature);
            info!("{:?}", status);

            // Optimistic approach to prevent oscillations. For now update local position as if the position if filled immediately.
            let bf_pos_sign: i128 = if self.bluefin_position.side { 1 } else { -1 };
            let bf_signed_pos: i128 = (self.bluefin_position.quantity as i128) * bf_pos_sign;
            let order_pos_sign: i128 = if order.isBuy { 1 } else { -1 };
            let order_signed_pos: i128 = (order.quantity as i128) * order_pos_sign;
            let new_pos = bf_signed_pos + order_signed_pos;

            self.bluefin_position.quantity = new_pos.abs() as u128;
            self.bluefin_position.side = if new_pos > 0 { true } else { false };
        }
    }
}
