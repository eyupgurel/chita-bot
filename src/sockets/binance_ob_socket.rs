use crate::models::binance_models::DepthUpdate;
use crate::models::common::OrderBook;
use log::info;
use std::net::TcpStream;
use std::sync::mpsc;
use tungstenite::connect;
use tungstenite::stream::MaybeTlsStream;
use url::Url;
use crate::constants::BINANCE_WS_API;

pub fn get_binance_ob_socket(_market:&str) -> tungstenite::WebSocket<MaybeTlsStream<TcpStream>> {
    let binance_url = format!("{}/ws/{}@depth5@100ms", BINANCE_WS_API, _market);

    let (binance_socket, _response) =
        connect(Url::parse(&binance_url).unwrap()).expect("Can't connect.");

    info!("Connected to binance stream.");
    return binance_socket;
}

pub fn stream_binance_ob_socket(_market:&str, _tx: mpsc::Sender<(String, OrderBook)>) {
    let mut binance_socket = get_binance_ob_socket(_market);

    loop {
        let binance_socket_message;
        let read = binance_socket.read();

        match read {
            Ok(message) => {
                binance_socket_message = message;

                let msg = match binance_socket_message {
                    tungstenite::Message::Text(s) => s,
                    _ => {
                        println!("Error getting text");
                        continue;
                    }
                };

                let parsed: DepthUpdate = serde_json::from_str(&msg).expect("Can't parse");
                let ob: OrderBook = parsed.into();
                _tx.send(("binance_ob".to_string(), ob)).unwrap();
            }
            Err(e) => {
                println!("Error during message handling: {:?}", e);
                binance_socket = get_binance_ob_socket(_market);
            }
        }
    }
}
