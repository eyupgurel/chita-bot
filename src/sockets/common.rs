use crate::models::common::OrderBook;
use std::net::TcpStream;
use std::sync::mpsc;
use tungstenite::stream::MaybeTlsStream;

pub trait OrderBookStream{
    fn get_ob_socket(&self, _market:&str) -> tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;
    fn stream_ob_socket(&self, _market:&str, _tx: mpsc::Sender<(String, OrderBook)>, tx_diff: mpsc::Sender<(String, OrderBook)>);
}