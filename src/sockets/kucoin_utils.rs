use crate::env;
use crate::env::EnvVars;
use crate::models::kucoin_models::Comm;
use crate::models::kucoin_models::Response;
use reqwest;
use std::net::TcpStream;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::WebSocket;

pub fn get_kucoin_url() -> String {
    let client = reqwest::blocking::Client::new();
    let vars: EnvVars = env::env_variables();
    let j: Result<Response, Box<dyn std::error::Error>> = client
        .post(&vars.kucoin_on_boarding_url)
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
            tracing::error!("Error: {}", e);
            e
        })
        .unwrap();

    let _kucoin_futures_wss_url = format!("{}?token={}", vars.kucoin_websocket_url, _token);

    return _kucoin_futures_wss_url;
}

pub fn get_kucoin_spot_url() -> String {
    let client = reqwest::blocking::Client::new();
    let vars: EnvVars = env::env_variables();
    let j: Result<Response, Box<dyn std::error::Error>> = client
        .post(&vars.kucoin_on_boarding_spot_url)
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
            tracing::error!("Error: {}", e);
            e
        })
        .unwrap();

    let _kucoin_spot_wss_url = format!("{}?token={}", vars.kucoin_websocket_spot_url, _token);

    return _kucoin_spot_wss_url;
}

pub fn send_ping(
    name: String,
    kucoin_socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    ack: &mut Comm,
    skip_duration: u64,
    last_ping_time: &mut std::time::Instant,
) {
    if last_ping_time.elapsed() >= std::time::Duration::from_secs(skip_duration) {
        tracing::debug!("Sending ping to Kucoin from socket {}...", &name);

        let ping = Comm {
            id: ack.id.clone(),
            type_: "ping".to_string(),
            data: None,
        };
        kucoin_socket
            .send(tungstenite::Message::Text(
                serde_json::to_string(&ping).unwrap(),
            ))
            .expect("Failed to send ping");
        *last_ping_time = std::time::Instant::now();
    }
}
