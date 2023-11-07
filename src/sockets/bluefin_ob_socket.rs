use std::fs;
use std::sync::mpsc;
use serde_json::json;
use std::net::TcpStream;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::connect;
use url::Url;
use crate::constants::BLUEFIN_WSS_URL;
use crate::models::bluefin_models::{OrderbookDepthUpdate};
use crate::models::common::OrderBook;

pub fn get_bluefin_ob_socket(
    market: &str,
    futures_wss_url: &str,
) -> tungstenite::WebSocket<MaybeTlsStream<TcpStream>> {
    let (mut socket, _response) =
        connect(Url::parse(&futures_wss_url).unwrap()).expect("Can't connect.");

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

pub fn stream_bluefin_ob_socket(_market: &str, _tx: mpsc::Sender<(String, OrderBook)>) {
    let mut socket = get_bluefin_ob_socket(_market, &BLUEFIN_WSS_URL);

    loop {
        let read = socket.read();

        match read {
            Ok(message) => {
                let socket_message = message;

                let msg = match socket_message {
                    tungstenite::Message::Text(s) => s,
                    _ => {
                        panic!("Error getting text");
                    }
                };

                if !msg.contains("OrderbookDepthUpdate") {
                    continue;
                }

/*                let ob_depth_update: OrderbookDepthUpdate =
                    serde_json::from_str(&msg).expect("Can't parse");*/

                let json_str = fs::read_to_string("./src/tests/seed/bluefin/bluefin-partial-depth.json")
                    .expect("Unable to read the file");

                let ob_depth_update: OrderbookDepthUpdate =
                    serde_json::from_str(&json_str).expect("Can't parse");

                let ob:OrderBook = ob_depth_update.into();

                _tx.send(("bluefin_ob".to_string(), ob))
                    .unwrap();
            }

            Err(e) => {
                println!("Error during message handling: {:?}", e);
                let mut new_socket = get_bluefin_ob_socket(_market, &BLUEFIN_WSS_URL);
                std::mem::swap(&mut socket, &mut new_socket);
                continue;
            }
        }
    }
}