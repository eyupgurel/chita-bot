use crate::models::kucoin_models::{Comm};
use crate::sockets::kucoin_utils::{get_kucoin_url, send_ping};
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use tungstenite::connect;
use tungstenite::stream::MaybeTlsStream;
use url::Url;

pub fn get_kucoin_socket(
    url: &str,
    market: &str,
    topic: &str
) -> (tungstenite::WebSocket<MaybeTlsStream<TcpStream>>, Comm) {
    let (mut kucoin_socket, _response) =
        connect(Url::parse(&url).unwrap()).expect("Can't connect.");

    // Construct the message
    let sub_message = format!(
        r#"{{
        "type": "subscribe",
        "topic":"{}:{}"
    }}"#,
        topic, market
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


pub fn stream_kucoin_socket<T, F>(
    url: &str,
    market: &str,
    topic: &str,
    tx: Sender<(String, T)>,
    parse_and_send: F,
)
    where
        T: Send + 'static,
        F: Fn(&str) -> T,
{
    let (mut kucoin_socket, mut ack) = get_kucoin_socket(url,market, topic);
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

                let data: T = parse_and_send(&msg);

                tx.send(("kucoin".to_string(), data)).unwrap();

                send_ping(&mut kucoin_socket, &mut ack, 50, &mut last_ping_time);
            }

            Err(e) => {
                println!("Error during message handling: {:?}", e);
                let (mut new_kucoin_socket, mut new_ack) =
                    get_kucoin_socket(&get_kucoin_url(), market,  &topic);
                std::mem::swap(&mut kucoin_socket, &mut new_kucoin_socket);
                std::mem::swap(&mut ack, &mut new_ack);
                continue;
            }
        }
    }
}