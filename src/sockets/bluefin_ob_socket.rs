use crate::env;
use crate::env::EnvVars;
use crate::models::common::OrderBook;
use crate::sockets::common::OrderBookStream;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, WebSocket};
use url::Url;

pub struct BluefinOrderBookStream<T> {
    phantom: std::marker::PhantomData<T>, // Use PhantomData to indicate the generic type usage
}

impl<T> BluefinOrderBookStream<T> {
    // Ensure this `new` function is in the same module as `BinanceOrderBookStream`
    pub fn new() -> Self {
        BluefinOrderBookStream {
            phantom: std::marker::PhantomData,
        }
    }
}

// Implement OrderBookStream for any type T that meets the trait bounds
impl<T> OrderBookStream<T> for BluefinOrderBookStream<T>
where
    T: DeserializeOwned + Into<OrderBook>, // T can be deserialized and converted into OrderBook
{
    fn get_ob_socket(&self, url: &str, market: &str) -> WebSocket<MaybeTlsStream<TcpStream>> {
        let (mut socket, _response) = connect(Url::parse(url).unwrap()).expect("Can't connect.");
        // Construct the message
        let sub_message = json!([
            "SUBSCRIBE",
            [
                {
                    "e": "orderbookDepthStream",
                    "p": market
                }
            ]
        ]);

        // Send the message
        let connect_status = socket.send(Message::Text(sub_message.to_string()));
        if connect_status.is_err() {
            tracing::error!("Error subscribing to Bluefin OrderBook websocket room");
        }

        let read = socket.read().expect("Error reading message");

        let _ack_msg = match read {
            Message::Text(s) => {
                tracing::info!("Connected to Bluefin stream at url:{} with Ack message {}.", &url, &s);
                s
            },
            _ => {
                panic!("Error getting text");
            }
        };

        return socket;
    }

    fn stream_ob_socket(
        &self,
        url: &str,
        market: &str,
        tx: Sender<OrderBook>,
        tx_diff: Sender<OrderBook>,
    ) {
        let vars: EnvVars = env::env_variables();
        let mut socket = self.get_ob_socket(url, market);
        let mut last_first_ask_price: Option<f64> = None;
        let mut last_first_bid_price: Option<f64> = None;

        loop { 
            let read = socket.read();

            match read {
                Ok(message) => {
                    match message {
                        Message::Text(msg) => {
                            if !msg.contains("OrderbookDepthUpdate") {
                                continue;
                            }

                            let parsed: T = serde_json::from_str(&msg).expect("Can't parse");
                            let ob: OrderBook = parsed.into();

                            let current_first_ask_price = ob.asks.first().map(|ask| ask.0.clone());
                            let current_first_bid_price = ob.bids.first().map(|bid| bid.0.clone());

                            let is_first_ask_price_changed =
                                match (current_first_ask_price, last_first_ask_price) {
                                    (Some(current), Some(last)) => {
                                        (current - last).abs() / last * 10000.0
                                            >= vars.market_making_trigger_bps
                                    }
                                    _ => false, // Consider unchanged if either current or last price is None
                                };

                            let is_first_bid_price_changed =
                                match (current_first_bid_price, last_first_bid_price) {
                                    (Some(current), Some(last)) => {
                                        (current - last).abs() / last * 10000.0
                                            >= vars.market_making_trigger_bps
                                    }
                                    _ => false, // Consider unchanged if either current or last price is None
                                };

                            // Update the last known prices
                            last_first_ask_price = current_first_ask_price;
                            last_first_bid_price = current_first_bid_price;

                            if is_first_ask_price_changed || is_first_bid_price_changed {
                                // Send the order book through the channel
                                tx_diff.send(ob.clone()).unwrap();
                            }

                            // tx_diff.send(ob.clone()).unwrap();
                            tx.send(ob).unwrap();
                        }
                        Message::Ping(ping_data) => {
                            tracing::debug!("Recieved Ping message from Bluefin OB channel, sending back Pong...");
                            // Handle the Ping message, e.g., by sending a Pong response
                            socket.write(Message::Pong(ping_data)).unwrap();
                        }
                        Message::Pong(pong_data) => {
                            tracing::debug!("Recieved Pong message from Bluefin OB channel, sending back Ping...");
                            // Handle the Ping message, e.g., by sending a Pong response
                            socket.write(Message::Ping(pong_data)).unwrap();
                        }
                        other => {
                            tracing::error!(bluefin_ob_socket_unexpected_disconnect = format!("Error: Received unexpected message type: {:?}. Reconnecting to websocket...", other));
                            socket = self.get_ob_socket(url, market);
                            tracing::info!(bluefin_ob_socket_unexpected_reconnect = "Reconnecting to Bluefin OB websocket ...");
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(bluefin_ob_socket_disconnect = format!("Error during Bluefin OrderBook socket message handling: {:?}", e));
                    socket = self.get_ob_socket(url, market);
                    tracing::info!(bluefin_ob_socket_resubscribe = "Resubscribed to Bluefin OrderBook socket.");
                    continue;
                }
            }
        }
    }
}
