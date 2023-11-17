use crate::models::common::OrderBook;
use crate::sockets::common::OrderBookStream;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::protocol::Message;
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

                            if !msg.contains("OrderbookDepthUpdate") {
                                continue;
                            }

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
                        _ => {
                            panic!("Error: Received unexpected message type");
                        }
                    }

                }
                Err(e) => {
                    println!("Error during message handling: {:?}", e);
                    socket = self.get_ob_socket(url, market);
                }
            }
        }
    }
}
