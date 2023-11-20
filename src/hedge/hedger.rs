use crate::sockets::kucoin_ob_socket::{stream_kucoin_socket};
use std::collections::HashMap;
use std::ops::Div;
use std::sync::mpsc;
use std::thread;
use log::{debug, info};
use serde_json::Value;
use crate::bluefin::{BluefinClient, parse_user_position, UserPosition};
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{Credentials, KuCoinClient, PositionChangeMessage};
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;

pub struct HGR {
    pub market_map: HashMap<String, String>,
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
    bluefin_position:UserPosition,
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

        let bluefin_market = market_map.get("bluefin").expect("Bluefin key not found").to_owned();
        let bluefin_position = bluefin_client.get_user_position(&bluefin_market);

        HGR {
            market_map,
            bluefin_client,
            kucoin_client,
            bluefin_position,
        }
    }
}

pub trait Hedger{
    fn connect(&mut self);
    fn round_to_decimal_places(value: f64, places: u32) -> f64;
}

impl Hedger for HGR {
    fn connect(&mut self) {
        let vars: EnvVars = env::env_variables();
        let (tx_kucoin_pos_change, rx_kucoin_pos_change) = mpsc::channel();
        let (tx_bluefin_pos_change, rx_bluefin_pos_change) = mpsc::channel();

        let kucoin_market = self.market_map.get("kucoin").expect("Kucoin key not found").to_owned();
        let kucoin_market_for_position_change = kucoin_market.clone();
        let kucoin_private_socket_url = self.kucoin_client.get_kucoin_private_socket_url().clone();
        let topic = format!("/contract/position");

        let _handle_kucoin_of = thread::spawn(move || {
            stream_kucoin_socket(
                &kucoin_private_socket_url,
                &kucoin_market_for_position_change,
                &topic,
                tx_kucoin_pos_change, // Sender channel of the appropriate type
                |msg: &str| -> PositionChangeMessage {
                    info!("PositionChangeMessage essence:{}",msg);
                    let message: PositionChangeMessage =
                        serde_json::from_str(&msg).expect("Can't parse");
                    message
                },
                "currentQty",
                true
            );
        });

        let bluefin_market = self.market_map.get("bluefin").expect("Bluefin key not found").to_owned();
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
                    info!("UserPosition essence:{}",msg);
                    let v: Value = serde_json::from_str(&msg).unwrap();
                    let message =
                        parse_user_position(v["data"]["position"].clone());
                    message
                },
            );
        });

        loop{
            match rx_kucoin_pos_change.try_recv() {
                Ok((_key, value)) => {
                    debug!("order response: {:?}", value);

                    let bluefin_market = self.market_map.get("bluefin").expect("Bluefin key not found").to_owned();
                    let current_qty : f64 = (value.data.current_qty as f64).div(100.0);
                    let target_quantity = current_qty * -1.0;
                    let mut bluefin_quantity = self.bluefin_position.quantity as f64;

                    if !self.bluefin_position.side {
                        bluefin_quantity = bluefin_quantity * -1.0;
                    }
                    let diff = target_quantity - bluefin_quantity;

                    let order_quantity =  HGR::round_to_decimal_places( diff.abs(),2);

                    let is_buy = diff.is_sign_positive();

                    let order =
                        self.bluefin_client.create_limit_ioc_order(&bluefin_market, is_buy, false, value.data.avg_entry_price, order_quantity, None);

                    let signature = self.bluefin_client.sign_order(order.clone());
                    let _status = self.bluefin_client.post_signed_order(order.clone(), signature);

                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Bluefin worker has disconnected!");
                }
            }

            match rx_bluefin_pos_change.try_recv() {
                Ok(value) => {
                    info!("position change: {:?}", value);
                    self.bluefin_position = value;
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

    fn round_to_decimal_places(value: f64, places: u32) -> f64 {
        let scale = 10_f64.powi(places as i32);
        (value * scale).round() / scale
    }
}