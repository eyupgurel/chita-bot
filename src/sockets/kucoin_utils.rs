use crate::models::kucoin_models::Response;
use reqwest;
use crate::constants::{KUCOIN_FUTURES_BASE_WSS_URL, KUCOIN_FUTURES_TOKEN_REQUEST_URL};
use crate::models::kucoin_models::{Comm};
use std::net::TcpStream;
use tungstenite::{WebSocket};
use tungstenite::stream::MaybeTlsStream;


pub fn get_kucoin_url() -> String {
    let client = reqwest::blocking::Client::new();

    let j: Result<Response, Box<dyn std::error::Error>> = client
        .post(KUCOIN_FUTURES_TOKEN_REQUEST_URL)
        .send()
        .map_err(|e| format!("Error making the request: {}", e).into())
        .and_then(|res| {
            res.text()
                .map_err(|e| format!("Error reading the response body: {}", e).into())
        })
        .and_then(|body| serde_json::from_str(&body).map_err(Into::into));

    let _token = j
        .map(|response| response.data.token)
        .map_err(|e| {
            println!("Error: {}", e);
            e
        })
        .unwrap();

    let _kucoin_futures_wss_url = format!("{}?token={}", KUCOIN_FUTURES_BASE_WSS_URL, _token);

    return _kucoin_futures_wss_url;
}

pub fn send_ping(
    kucoin_socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    ack: &mut Comm,
    skip_duration: u64,
    last_ping_time: &mut std::time::Instant,
) {
    if last_ping_time.elapsed() >= std::time::Duration::from_secs(skip_duration) {
        let ping = Comm {
            id: ack.id.clone(),
            type_: "ping".to_string(),
        };
        kucoin_socket
            .send(tungstenite::Message::Text(serde_json::to_string(&ping).unwrap()))
            .expect("Failed to send ping");
        *last_ping_time = std::time::Instant::now();
    }
}
