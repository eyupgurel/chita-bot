use crate::constants::KUCOIN_DEPTH_SOCKET_TOPIC;
use crate::models::common::OrderBook;
use crate::models::kucoin_models::{Comm, Level2Depth};
use crate::sockets::kucoin_utils::{get_kucoin_url, send_ping};
use std::net::TcpStream;
use std::sync::mpsc;
use tungstenite::connect;
use tungstenite::stream::MaybeTlsStream;
use url::Url;

pub fn get_kucoin_ob_socket(
    market: &str,
    kucoin_futures_wss_url: &String,
) -> (tungstenite::WebSocket<MaybeTlsStream<TcpStream>>, Comm) {
    let (mut kucoin_socket, _response) =
        connect(Url::parse(&kucoin_futures_wss_url).unwrap()).expect("Can't connect.");

    // Construct the message
    let sub_message = format!(
        r#"{{
        "type": "subscribe",
        "topic":"{}:{}"
    }}"#,
        KUCOIN_DEPTH_SOCKET_TOPIC, market
    );

    // Send the message
    kucoin_socket
        .send(tungstenite::protocol::Message::Text(
            sub_message.to_string(),
        ))
        .unwrap();

    let read = kucoin_socket.read().expect("Error reading message");

    let ack_msg = match read {
        tungstenite::Message::Text(s) => s,
        _ => {
            panic!("Error getting text");
        }
    };

    let ack: Comm = serde_json::from_str(&ack_msg).expect("Can't parse");

    return (kucoin_socket, ack);
}

pub fn stream_kucoin_ob_socket(market: &str, tx: mpsc::Sender<(String, OrderBook)>) {
    let (mut kucoin_socket, mut ack) = get_kucoin_ob_socket(market, &get_kucoin_url());
    let mut last_ping_time = std::time::Instant::now();
    loop {
        let read = kucoin_socket.read();

        match read {
            Ok(message) => {
                let kucoin_socket_message = message;

                let msg = match kucoin_socket_message {
                    tungstenite::Message::Text(s) => s,
                    _ => {
                        panic!("Error getting text");
                    }
                };

                if msg.contains("pong") {
                    continue;
                }

                let parsed_kucoin_ob: Level2Depth =
                    serde_json::from_str(&msg).expect("Can't parse");
                let ob: OrderBook = parsed_kucoin_ob.into();

                tx.send(("kucoin".to_string(), ob)).unwrap();

                send_ping(&mut kucoin_socket, &mut ack, &mut last_ping_time);
            }

            Err(e) => {
                println!("Error during message handling: {:?}", e);
                let (mut new_kucoin_socket, mut new_ack) =
                    get_kucoin_ob_socket(market, &get_kucoin_url());
                std::mem::swap(&mut kucoin_socket, &mut new_kucoin_socket);
                std::mem::swap(&mut ack, &mut new_ack);
                continue;
            }
        }
    }
}
