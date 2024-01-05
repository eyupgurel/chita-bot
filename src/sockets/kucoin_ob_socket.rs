use crate::models::kucoin_models::Comm;
use crate::sockets::kucoin_utils::{get_kucoin_url, send_ping};
use rand::Rng;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use tungstenite::connect;
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use url::Url;
fn generate_random_number_of_digits(digits: u32) -> i64 {
    if digits < 1 || digits > 18 {
        panic!("Number of digits must be between 1 and 18");
    }

    let min = 10_i64.pow(digits - 1);
    let max = 10_i64.pow(digits) - 1;
    let mut rng = rand::thread_rng();

    rng.gen_range(min..=max)
}
pub fn get_kucoin_socket(
    url: &str,
    market: &str,
    topic: &str,
    is_private: bool,
) -> (tungstenite::WebSocket<MaybeTlsStream<TcpStream>>, Comm) {
    let (mut kucoin_socket, _response) =
        connect(Url::parse(&url).unwrap()).expect("Can't connect.");
    tracing::info!("Connected to Kucoin stream at url:{}.", &url);

    let _read = kucoin_socket.read().expect("Error reading message");

    // Construct the message
    let id = generate_random_number_of_digits(13);

    let sub_message = format!(
        r#"{{
        "id":{},
        "type": "subscribe",
        "topic":"{}:{}",
        "privateChannel": {},
        "response": true
    }}"#,
        id, topic, market, is_private
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
    indicator: &str,
    is_private: bool,
) where
    T: Send + 'static,
    F: Fn(&str) -> T,
{
    let (mut socket, mut ack) = get_kucoin_socket(url, market, topic, is_private);
    let mut last_ping_time = std::time::Instant::now();
    loop {
        tracing::info!("Attempting to read kucoin socket message...");
        let read = socket.read();
        match read {
            Ok(message) => {
                tracing::info!("Kucoin socket message read ok: {}", message);
                match message {
                    Message::Text(msg) => {
                        // Skip the indicator check if indicator is empty
                        if !indicator.is_empty() && !msg.contains(indicator) {
                            continue;
                        }

                        if msg.contains("pong") {
                            continue;
                        }

                        let data: T = parse_and_send(&msg);

                        tx.send(("kucoin".to_string(), data)).unwrap();

                        send_ping(&mut socket, &mut ack, 18, &mut last_ping_time);
                    }
                    Message::Ping(ping_data) => {
                        // Handle the Ping message, e.g., by sending a Pong response
                        socket.write(Message::Pong(ping_data)).unwrap();
                    }
                    other => {
                        tracing::error!("Error: Received unexpected message type: {:?}", other);
                    }
                }
            }

            Err(e) => {
                tracing::error!("Error during message handling: {:?}", e);
                let (mut new_kucoin_socket, mut new_ack) =
                    get_kucoin_socket(&get_kucoin_url(), market, &topic, is_private);
                std::mem::swap(&mut socket, &mut new_kucoin_socket);
                std::mem::swap(&mut ack, &mut new_ack);
                continue;
            }
        }
    }
}
