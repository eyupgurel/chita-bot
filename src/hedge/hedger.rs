use crate::sockets::kucoin_ob_socket::{stream_kucoin_socket};
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use log::debug;
use serde_json::Value;
use crate::bluefin::{BluefinClient, OrderUpdate};
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{Credentials, KuCoinClient, TradeOrderMessage};
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;

pub struct HGR {
    pub market_map: HashMap<String, String>,
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
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

        HGR {
            market_map,
            bluefin_client,
            kucoin_client,
        }
    }
}

pub trait Hedger{
    fn connect(&mut self);
}

impl Hedger for HGR {
    fn connect(&mut self) {
        let vars: EnvVars = env::env_variables();
        let (tx_kucoin_of, rx_kucoin_of) = mpsc::channel();
        let (tx_bluefin_of, _rx_bluefin_of) = mpsc::channel();

        let kucoin_market = self.market_map.get("kucoin").expect("Kucoin key not found").to_owned();
        let kucoin_market_for_order_fill = kucoin_market.clone();
        let kucoin_private_socket_url = self.kucoin_client.get_kucoin_private_socket_url().clone();

        let _handle_kucoin_of = thread::spawn(move || {
            stream_kucoin_socket(
                &kucoin_private_socket_url,
                &kucoin_market_for_order_fill,
                &"/contractMarket/tradeOrders",
                tx_kucoin_of, // Sender channel of the appropriate type
                |msg: &str| -> TradeOrderMessage {
                    let message: TradeOrderMessage =
                        serde_json::from_str(&msg).expect("Can't parse");
                    message
                },
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
                "OrderUpdate",
                tx_bluefin_of, // Sender channel of the appropriate type
                |msg: &str| -> OrderUpdate {

                    let v: Value = serde_json::from_str(&msg).unwrap();
                    let message: OrderUpdate =
                        serde_json::from_str(&v["data"]["order"].to_string())
                            .expect("Can't parse");
                    message
                },
            );
        });


        loop{
            match rx_kucoin_of.try_recv() {
                Ok((_key, value)) => {
                    debug!("order response: {:?}", value);

                    let bluefin_market = self.market_map.get("bluefin").expect("Bluefin key not found").to_owned();

                    let is_buy = value.data.side == "sell";
                    let order =
                        self.bluefin_client.create_limit_ioc_order(&bluefin_market,  is_buy , false, value.data.price, value.data.filled_size, Some(vars.bluefin_leverage));

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
        }

    }
}