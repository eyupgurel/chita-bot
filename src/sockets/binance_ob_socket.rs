use crate::constants::BINANCE_WS_API;
use crate::models::binance_models::DepthUpdate;
use crate::models::common::OrderBook;
use log::{info, warn};
use std::net::TcpStream;
use std::sync::mpsc;
use tungstenite::connect;
use tungstenite::stream::MaybeTlsStream;
use url::Url;

pub fn get_binance_ob_socket(_market: &str) -> tungstenite::WebSocket<MaybeTlsStream<TcpStream>> {
    let binance_url = format!("{}/ws/{}@depth5@100ms", BINANCE_WS_API, _market);

    let (binance_socket, _response) =
        connect(Url::parse(&binance_url).unwrap()).expect("Can't connect.");

    info!("Connected to binance stream.");
    return binance_socket;
}

pub fn stream_binance_ob_socket(
    _market: &str,
    _tx: mpsc::Sender<(String, OrderBook)>,
    tx_diff: mpsc::Sender<(String, OrderBook)>,
) {
    let mut binance_socket = get_binance_ob_socket(_market);
    let mut last_first_ask_price: Option<String> = None;
    let mut last_first_bid_price: Option<String> = None;

    loop {
        let binance_socket_message;
        let read = binance_socket.read();

        match read {
            Ok(message) => {
                binance_socket_message = message;

                let msg = match binance_socket_message {
                    tungstenite::Message::Text(s) => s,
                    _ => {
                        warn!("Error getting text");
                        continue;
                    }
                };

                let parsed: DepthUpdate = serde_json::from_str(&msg).expect("Can't parse");
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
                    tx_diff
                        .send(("binance_ob".to_string(), ob.clone()))
                        .unwrap();
                }
                _tx.send(("binance_ob".to_string(), ob)).unwrap();
            }
            Err(e) => {
                println!("Error during message handling: {:?}", e);
                binance_socket = get_binance_ob_socket(_market);
            }
        }
    }
}
