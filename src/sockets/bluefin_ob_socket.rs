use serde_json::json;
use std::net::TcpStream;
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
impl<T> OrderBookStream<T> for BluefinOrderBookStream<T>
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
}