use std::net::TcpStream;
use std::sync::mpsc;
use tungstenite::connect;
use url::Url;
use reqwest;
use crate::kucoin_models::{Response};
use tungstenite::stream::MaybeTlsStream;
use crate::kucoin_models::{Comm, Level2Depth};

static KUCOIN_FUTURES_TOKEN_REQUEST_URL: &str = "https://api-futures.kucoin.com/api/v1/bullet-public";
static KUCOIN_FUTURES_BASE_WSS_URL: &str = "wss://ws-api-futures.kucoin.com/endpoint";
static KUCOIN_SOCKET_TOPIC: &str = "/contractMarket/level2Depth50:XBTUSDTM";

pub fn get_kucoin_url() -> String {
    let client = reqwest::blocking::Client::new();

    let j: Result<Response, Box<dyn std::error::Error>> =
        client.post(KUCOIN_FUTURES_TOKEN_REQUEST_URL)
            .send()
            .map_err(|e| format!("Error making the request: {}", e).into())
            .and_then(|res| res.text().map_err(|e| format!("Error reading the response body: {}", e).into()))
            .and_then(|body| serde_json::from_str(&body).map_err(Into::into));

    let _token = j.map(|response| response.data.token)
        .map_err(|e| {
            println!("Error: {}", e);
            e
        }).unwrap();


    let _kucoin_futures_wss_url = format!(
        "{}?token={}",
        KUCOIN_FUTURES_BASE_WSS_URL, _token
    );

    return _kucoin_futures_wss_url;
}

pub fn get_kucoin_socket(_kucoin_futures_wss_url: &String) -> (tungstenite::WebSocket<MaybeTlsStream<TcpStream>>, Comm) {
    let (mut kucoin_socket, _response) =
        connect(Url::parse(&_kucoin_futures_wss_url).unwrap()).expect("Can't connect.");

    // Construct the message
    let sub_message = format!(r#"{{
        "type": "subscribe",
        "topic":"{}"
    }}"#, KUCOIN_SOCKET_TOPIC);

    // Send the message
    kucoin_socket.send(tungstenite::protocol::Message::Text(sub_message.to_string())).unwrap();


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

pub fn stream_kucoin_socket(_tx: mpsc::Sender<(String, Level2Depth)>) {
    let (mut kucoin_socket, mut ack) = get_kucoin_socket(&get_kucoin_url());
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

                let parsed_kucoin_ob: Level2Depth = serde_json::from_str(&msg).expect("Can't parse");
                _tx.send(("kucoin_ob".to_string(), parsed_kucoin_ob)).unwrap();

                if last_ping_time.elapsed() >= std::time::Duration::from_secs(50) {
                    let ping = Comm {
                        id: ack.id.clone(),
                        type_: "ping".to_string(),
                    };
                    kucoin_socket.send(tungstenite::protocol::Message::Text(serde_json::to_string(&ping).unwrap())).unwrap();
                    last_ping_time = std::time::Instant::now();
                }
            }

            Err(e) => {
                println!("Error during message handling: {:?}", e);
                let (mut new_kucoin_socket, mut new_ack) = get_kucoin_socket(&get_kucoin_url());
                std::mem::swap(&mut kucoin_socket, &mut new_kucoin_socket);
                std::mem::swap(&mut ack, &mut new_ack);
                continue;
            }
        }
    }
}
