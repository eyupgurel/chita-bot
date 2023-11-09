use crate::models::common::OrderBook;
use log::info;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use serde::de::DeserializeOwned;
use tungstenite::{connect, WebSocket};
use tungstenite::stream::MaybeTlsStream;
use url::Url;
use crate::sockets::common::OrderBookStream;

pub struct BinanceOrderBookStream<T> {
    phantom: std::marker::PhantomData<T>, // Use PhantomData to indicate the generic type usage
}

impl<T> BinanceOrderBookStream<T> {
    pub fn new() -> Self {
        BinanceOrderBookStream {
            phantom: std::marker::PhantomData,
        }
    }
}

// Implement OrderBookStream for any type T that meets the trait bounds
impl<T> OrderBookStream for BinanceOrderBookStream<T>
    where
        T: DeserializeOwned + Into<OrderBook>,
{
    fn get_ob_socket(&self, url:&str, _market:&str) -> WebSocket<MaybeTlsStream<TcpStream>> {
        let (socket, _response) =
            connect(Url::parse(&url).unwrap()).expect("Can't connect.");

        info!("Connected to binance stream.");
        return socket;
    }

    fn stream_ob_socket(&self, url:&str, market:&str, tx: Sender<(String, OrderBook)>, tx_diff: Sender<(String, OrderBook)>) {
        let mut socket = self.get_ob_socket(url, market);
        let mut last_first_ask_price: Option<f64> = None;
        let mut last_first_bid_price: Option<f64> = None;

        loop {
            let socket_message;
            let read = socket.read();

            match read {
                Ok(message) => {
                    socket_message = message;

                    let msg = match socket_message {
                        tungstenite::Message::Text(s) => s,
                        _ => {
                            println!("Error getting text");
                            continue;
                        }
                    };

                    let parsed: T = serde_json::from_str(&msg).expect("Can't parse");
                    let ob: OrderBook = parsed.into();

                    let current_first_ask_price = ob.asks.first().map(|ask| ask.0.clone());
                    let current_first_bid_price = ob.bids.first().map(|bid| bid.0.clone());

                    let is_first_ask_price_changed = current_first_ask_price != last_first_ask_price;
                    let is_first_bid_price_changed = current_first_bid_price != last_first_bid_price;

                    if is_first_ask_price_changed || is_first_bid_price_changed {
                        // Update the last known prices
                        last_first_ask_price = current_first_ask_price;
                        last_first_bid_price = current_first_bid_price;

                        // Send the order book through the channel
                        tx_diff.send(("binance".to_string(), ob.clone())).unwrap();
                    }
                    tx.send(("binance".to_string(), ob)).unwrap();

                }
                Err(e) => {
                    println!("Error during message handling: {:?}", e);
                    socket =  self.get_ob_socket(url, market);
                }
            }
        }
    }
}