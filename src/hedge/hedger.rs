use crate::bluefin::{parse_user_position, BluefinClient, UserPosition};
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{Credentials, KuCoinClient};
use crate::models::common::Market;
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

static BIGNUMBER_BASE: u128 = 1000000000000000000;
static HEDGE_PERIOD_DURATION: u64 = 1;

pub struct HGR {
    pub market: Market,
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
    bluefin_position: UserPosition,
}

impl HGR {
    pub fn new(market: Market) -> HGR {
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

        let bluefin_market = market.symbols.bluefin.to_owned();
        let bluefin_position = bluefin_client.get_user_position(&bluefin_market);

        HGR {
            market,
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

        let bluefin_market = self.market.symbols.bluefin.to_owned();
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
                    tracing::debug!("UserPosition essence:{}", msg);
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
                    tracing::debug!("position change: {:?}", value);
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
        let bluefin_market = self.market.symbols.bluefin.to_owned();

        // the call now returns Option(UserPosition)
        let kucoin_position = self.kucoin_client.get_position(&bluefin_market);

        // if we are unable to get KuCoin position just return
        // this is possible due to rate limiting
        if kucoin_position.is_none() {
            return;
        }

        // unwrap kucoin position and get quantity
        let kucoin_quantity = Decimal::from(kucoin_position.unwrap().quantity);

        let current_kucoin_qty = kucoin_quantity / Decimal::from(self.market.lot_size);

        let target_quantity = current_kucoin_qty * Decimal::from(-1);

        let mut bluefin_quantity = Decimal::from_u128(self.bluefin_position.quantity).unwrap()
            / Decimal::from(BIGNUMBER_BASE);

        if !self.bluefin_position.side {
            bluefin_quantity = bluefin_quantity * Decimal::from(-1);
        }

        let diff = target_quantity - bluefin_quantity;

        let order_quantity = diff.abs();

        let is_buy = diff.is_sign_positive();

        if order_quantity > Decimal::from(0) {
            tracing::info!("kucoin quantity:{}", current_kucoin_qty);
            tracing::info!("bluefin quantity:{}", bluefin_quantity);
            tracing::info!("Order quantity: {} is buy:{}", order_quantity, is_buy);
        }
        if order_quantity >= Decimal::from_str(&self.market.min_size).unwrap() {
            {
                tracing::debug!("order quantity as decimal: {}", order_quantity);
                let order_quantity_f64 = order_quantity.to_f64().unwrap();
                tracing::debug!("order quantity as f64: {}", order_quantity_f64);
                let order = self.bluefin_client.create_market_order(
                    &bluefin_market,
                    is_buy,
                    false,
                    order_quantity_f64,
                    None,
                );
                tracing::info!("Order: {:#?}", order);
                let signature = self.bluefin_client.sign_order(order.clone());
                let status = self
                    .bluefin_client
                    .post_signed_order(order.clone(), signature);
                tracing::info!("{:?}", status);

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
}
