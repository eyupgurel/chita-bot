use std::fs;
use serde_json::json;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use serde::de::DeserializeOwned;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, WebSocket};
use url::Url;
use crate::models::common::OrderBook;
use crate::sockets::common::OrderBookStream;


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
impl<T> OrderBookStream for BluefinOrderBookStream<T>
    where
        T: DeserializeOwned + Into<OrderBook>, // T can be deserialized and converted into OrderBook
{
    fn get_ob_socket(&self, url: &str, market: &str) -> WebSocket<MaybeTlsStream<TcpStream>> {
        let (mut socket, _response) =
            connect(Url::parse(url).unwrap()).expect("Can't connect.");

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
        socket
            .send(tungstenite::protocol::Message::Text(
                sub_message.to_string(),
            ))
            .unwrap();

        let read = socket.read().expect("Error reading message");

        let _ack_msg = match read {
            tungstenite::Message::Text(s) => s,
            _ => {
                panic!("Error getting text");
            }
        };

        return socket;
    }

    fn stream_ob_socket(&self, url: &str, market: &str, tx: Sender<OrderBook>, tx_diff: Sender<OrderBook>) {
        let mut socket = self.get_ob_socket(url, market);
        let mut last_first_ask_price: Option<f64> = None;
        let mut last_first_bid_price: Option<f64> = None;

        loop {
            let socket_message;
            let read = socket.read();

            match read {
                Ok(message) => {
                    socket_message = message;

                    let _msg = match socket_message {
                        tungstenite::Message::Text(s) => s,
                        _ => {
                            println!("Error getting text");
                            continue;
                        }
                    };

                    //let parsed: OrderbookDepthUpdate = serde_json::from_str(&msg).expect("Can't parse");

                    let json_str = fs::read_to_string("./src/tests/seed/bluefin/bluefin-partial-depth.json")
                        .expect("Unable to read the file");

                    let parsed: T =
                        serde_json::from_str(&json_str).expect("Can't parse");


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
                        tx_diff.send( ob.clone()).unwrap();
                    }
                    tx.send(ob).unwrap();
                }
                Err(e) => {
                    println!("Error during message handling: {:?}", e);
                    socket = self.get_ob_socket(url, market);
                }
            }
        }
    }
}