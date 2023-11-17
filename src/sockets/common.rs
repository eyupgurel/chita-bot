use crate::models::common::OrderBook;
use serde::de::DeserializeOwned;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use log::error;
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;

pub trait OrderBookStream<T>
where
    T: DeserializeOwned + Into<OrderBook>,
{
    fn get_ob_socket(
        &self,
        url: &str,
        market: &str,
    ) -> tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;
    fn stream_ob_socket(
        &self,
        url: &str,
        market: &str,
        tx: Sender<OrderBook>,
        tx_diff: Sender<OrderBook>,
    ) {
        let mut socket = self.get_ob_socket(url, market);
        let mut last_first_ask_price: Option<f64> = None;
        let mut last_first_bid_price: Option<f64> = None;

        loop {
            let read = socket.read();

            match read {
                Ok(message) => {
                    match message {
                        Message::Text(msg) => {

                            let parsed: T = serde_json::from_str(&msg).expect("Can't parse");
                            let ob: OrderBook = parsed.into();

                            let current_first_ask_price = ob.asks.first().map(|ask| ask.0.clone());
                            let current_first_bid_price = ob.bids.first().map(|bid| bid.0.clone());

                            let is_first_ask_price_changed =
                                current_first_ask_price != last_first_ask_price;
                            let is_first_bid_price_changed =
                                current_first_bid_price != last_first_bid_price;

                            if is_first_ask_price_changed || is_first_bid_price_changed {
                                // Update the last known prices
                                last_first_ask_price = current_first_ask_price;
                                last_first_bid_price = current_first_bid_price;

                                // Send the order book through the channel
                                tx_diff.send(ob.clone()).unwrap();
                            }
                            tx.send(ob).unwrap();
                        },
                        Message::Ping(ping_data) => {
                            // Handle the Ping message, e.g., by sending a Pong response
                            socket.write(Message::Pong(ping_data)).unwrap();
                        },
                        other => {
                            error!("Error: Received unexpected message type: {:?}", other);
                        }
                    }
                }
                Err(e) => {
                    error!("Error during message handling: {:?}", e);
                    socket = self.get_ob_socket(url, market);
                }
            }
        }
    }
}
