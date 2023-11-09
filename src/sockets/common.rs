use crate::models::common::OrderBook;
use std::net::TcpStream;
use std::sync::mpsc;
use tungstenite::stream::MaybeTlsStream;

pub trait OrderBookStream{
    fn get_ob_socket(&self, url:&str, market:&str) -> tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;
    fn stream_ob_socket(&self, url:&str, market:&str, tx: mpsc::Sender<OrderBook>, tx_diff: mpsc::Sender<OrderBook>);
}