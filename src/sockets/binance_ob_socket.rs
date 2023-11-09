use crate::models::common::OrderBook;
use log::info;
use std::net::TcpStream;
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

impl<T> OrderBookStream<T> for BinanceOrderBookStream<T>
    where
        T: DeserializeOwned + Into<OrderBook>,
{
    fn get_ob_socket(&self, url:&str, _market:&str) -> WebSocket<MaybeTlsStream<TcpStream>> {
        let (socket, _response) =
            connect(Url::parse(&url).unwrap()).expect("Can't connect.");

        info!("Connected to binance stream.");
        return socket;
    }
}