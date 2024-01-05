use serde_json::json;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, WebSocket};
use url::Url;

fn get_private_bluefin_socket(
    url: &str,
    _market: &str,
    auth_token: &str,
) -> WebSocket<MaybeTlsStream<TcpStream>> {
    let (mut socket, _response) = connect(Url::parse(url).unwrap()).expect("Can't connect.");
    // Construct the message
    let sub_message = json!([
        "SUBSCRIBE",
        [
            {
                "e": "userUpdates",
                "t": auth_token
            }
        ]
    ]);

    // Send the message
    let connection_status = socket.send(
        tungstenite::protocol::Message::Text(
            sub_message.to_string(),
        )
    );

    if connection_status.is_err() {
        tracing::error!("Error subscribing to Bluefin userUpdates websocket room");
    }

    let read = socket.read().expect("Error reading message");

    let _ack_msg = match read {
        Message::Text(s) => {
            tracing::info!("Connected to Bluefin stream at url:{} with Ack message {}.", &url, &s);
            s
        },
        _ => {
            panic!("Error getting text");
        }
    };

    return socket;
}

pub fn stream_bluefin_private_socket<T, F>(
    url: &str,
    _market: &str,
    auth_token: &str,
    indicator: &str,
    tx: Sender<T>,
    parse_and_send: F,
) where
    T: Send + 'static,
    F: Fn(&str) -> T,
{
    let mut socket = get_private_bluefin_socket(url, _market, auth_token);
    tracing::info!("Subscribing to bluefin socket with indicator: {:?}", &indicator);
    loop {
        let read = socket.read();

        match read {
            Ok(message) => {
                match message {
                    Message::Text(s) => {
                        if !s.contains(indicator) {
                            continue;
                        }
                        let data: T = parse_and_send(&s);
                        tx.send(data).unwrap();
                    }
                    Message::Ping(ping_data) => {
                        tracing::debug!("Recieved Ping message from Bluefin for indicator: {:?}, sending back Pong...", &indicator);
                        // Handle the Ping message, e.g., by sending a Pong response
                        socket.write(Message::Pong(ping_data)).unwrap();
                    }
                    Message::Pong(pong_data) => {
                        tracing::debug!("Recieved Pong message from Bluefin for indicator: {:?}, sending back Ping...", &indicator);
                        // Handle the Ping message, e.g., by sending a Pong response
                        socket.write(Message::Ping(pong_data)).unwrap();
                    }
                    other => {
                        tracing::error!(bluefin_private_socket_unexpected_message = format!("Error: Received unexpected message type: {:?}. Reconnecting to websocket...", other));
                        socket = get_private_bluefin_socket(url, _market, auth_token);
                        tracing::info!(bluefin_private_socket_unexpected_reconnect = format!("Reconnecting to websocket {:?} ...", &indicator));
                        continue;
                    }
                }
            }

            Err(e) => {
                tracing::error!(bluefin_private_socket_disconnect = format!("Error during Bluefin private socket message handling for topic: {:?}, with error: {:?}", &indicator, e));
                socket = get_private_bluefin_socket(url, _market, auth_token);
                tracing::info!(bluefin_private_socket_reconnect = format!("Resubscribed to Bluefin private socket for topic {}.", &indicator));
                continue;
            }
        }
    }
}
