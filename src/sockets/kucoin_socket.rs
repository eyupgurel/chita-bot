use crate::models::kucoin_models::Comm;
use crate::sockets::kucoin_utils::{get_kucoin_url, send_ping};
use crate::kucoin::KuCoinClient;
use crate::kucoin::Credentials;
use crate::env;

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

    let resolved_topic = if market.is_empty() { topic.to_string() } else { format!("{}:{}", topic, market) };
    tracing::info!("Resolved Topic: {}", resolved_topic);

    let sub_message = format!(
        r#"{{
        "id":{},
        "type": "subscribe",
        "topic":"{}",
        "privateChannel": {},
        "response": true
    }}"#,
        id, resolved_topic, is_private
    );

    tracing::info!("Subscribing to kucoin socket for market: {:?} and topic: {:?}", &market, &topic);

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

    tracing::info!("Kucoin socket {} ack msg {}", &topic, &ack_msg);

    let ack: Comm = serde_json::from_str(&ack_msg).expect("Can't parse");

    //Panic on error during Ack handshake
    if ack.type_.eq("error") {
        panic!("Error: {:?} while getting ack message back from Kucoin for topic {:?}", &ack.data.unwrap_or("No Error Message".to_string()), &topic);
    }

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

    let vars= env::env_variables();

    let kucoin_client = KuCoinClient::new(
        Credentials::new(
            &vars.kucoin_api_key,
            &vars.kucoin_api_secret,
            &vars.kucoin_api_phrase,
        ),
        &vars.kucoin_endpoint,
        &vars.kucoin_on_boarding_url,
        &vars.kucoin_websocket_url,
        vars.kucoin_leverage,
    );

    loop {
        let read = socket.read();

        match read {
            Ok(message) => {
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

                        send_ping(format!("Indicator: {}, Topic: {}", &indicator, &topic), &mut socket, &mut ack, 18, &mut last_ping_time);
                    }
                    Message::Ping(ping_data) => {
                        // Handle the Ping message, e.g., by sending a Pong response
                        tracing::debug!("Ping message recieved from Kucoin");
                        socket.write(Message::Pong(ping_data)).unwrap();
                    }
                    Message::Pong(_pong_data) => {
                        tracing::debug!("Pong message recieved from Kucoin");
                        send_ping(format!("Pong Indicator: {}, Topic: {}", &indicator, &topic), &mut socket, &mut ack, 18, &mut last_ping_time);
                    }
                    other => {
                        tracing::error!(kucoin_socket_unexpected_message = format!("Error: Received unexpected message type: {:?}", other));
                        if is_private {
                            (socket, ack) = get_kucoin_socket(&kucoin_client.get_kucoin_private_socket_url(), market, &topic, is_private);
                        } else { 
                            (socket, ack) = get_kucoin_socket(&get_kucoin_url(), market, &topic, is_private);
                        }
                        tracing::info!(kucoin_socket_unexpected_message_reconnect = format!("Reconnected to Kucoin websocket topic {}, indicator {}", &topic, &indicator));
                    }
                }
            }

            Err(e) => {
                tracing::error!(kucoin_socket_error = format!("Error during Kucoin socket message handling for topic : {:?} with indicator: {:?}, and error: {:?}", &topic, &indicator, e));
                if is_private {
                    (socket, ack) = get_kucoin_socket(&kucoin_client.get_kucoin_private_socket_url(), market, &topic, is_private);
                } else { 
                    (socket, ack) = get_kucoin_socket(&get_kucoin_url(), market, &topic, is_private);
                }
                tracing::info!(kucoin_socket_reconnect = format!("Resubscribed to Kucoin socket for topic : {:?} with indicator: {:?}", &topic, &indicator));
                continue;
            }
        }
    }
}
