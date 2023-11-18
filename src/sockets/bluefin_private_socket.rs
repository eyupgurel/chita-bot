use serde_json::json;
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use log::{error, info};
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, WebSocket};
use url::Url;


fn get_private_bluefin_socket(url: &str, _market: &str, auth_token:&str) -> WebSocket<MaybeTlsStream<TcpStream>> {
    let (mut socket, _response) = connect(Url::parse(url).unwrap()).expect("Can't connect.");
    info!("Connected to Bluefin stream at url:{}.", &url);

    // Construct the message
    let sub_message =  json!([
                    "SUBSCRIBE",
                    [
                        {
                            "e": "userUpdates",
                            "rt": "",
                            "t": auth_token
                        }
                    ]
                ]);

    // Send the message
    socket
        .send(tungstenite::protocol::Message::Text(
            sub_message.to_string(),
        ))
        .unwrap();

    return socket;
}

pub fn stream_bluefin_private_socket<T, F>(
    url: &str,
    _market: &str,
    auth_token:&str,
    indicator:&str,
    tx: Sender<T>,
    parse_and_send: F,
)
    where
        T: Send + 'static,
        F: Fn(&str) -> T,
{
    let mut socket = get_private_bluefin_socket(url,_market, auth_token);

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
                    },
                    Message::Ping(ping_data) => {
                        // Handle the Ping message, e.g., by sending a Pong response
                        socket.write(Message::Pong(ping_data)).unwrap();
                    },
                    other => {
                        error!("Error: Received unexpected message type: {:?}", other);
                    }
                }
            }

            Err(e) => {
                error!("Error during message handling: {:?}", e);
                let mut new_socket =
                    get_private_bluefin_socket(url, _market,  auth_token);
                std::mem::swap(&mut socket, &mut new_socket);
                continue;
            }
        }
    }
}