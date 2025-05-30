use crate::env;
use crate::env::EnvVars;
use crate::models::kucoin_models::{Comm, TickerV2};
use crate::sockets::kucoin_utils::{get_kucoin_url, send_ping};
use std::net::TcpStream;
use std::sync::mpsc;
use tungstenite::connect;
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use url::Url;

pub fn get_kucoin_ticker_socket(
    market: &str,
    kucoin_futures_wss_url: &String,
) -> (tungstenite::WebSocket<MaybeTlsStream<TcpStream>>, Comm) {
    let vars: EnvVars = env::env_variables();

    let (mut kucoin_ticker_socket, _response) =
        connect(Url::parse(&kucoin_futures_wss_url).unwrap()).expect("Can't connect.");

    tracing::info!(
        "Connected to Kucoin stream at url:{}.",
        &kucoin_futures_wss_url
    );
    // Construct the message
    let sub_message = format!(
        r#"{{
        "type": "subscribe",
        "topic":"{}:{}"
    }}"#,
        &vars.kucoin_ticker_v2_socket_topic, market
    );

    tracing::info!("Subscribing to kucoin ticker socket for market: {:?} and topic: {:?}", &market, &vars.kucoin_ticker_v2_socket_topic);

    // Send the message
    kucoin_ticker_socket
        .send(tungstenite::protocol::Message::Text(
            sub_message.to_string(),
        ))
        .unwrap();

    let read = kucoin_ticker_socket.read().expect("Error reading message");

    let ack_msg = match read {
        tungstenite::Message::Text(s) => s,
        _ => {
            panic!("Error getting text");
        }
    };

    let ack: Comm = serde_json::from_str(&ack_msg).expect("Can't parse");

    return (kucoin_ticker_socket, ack);
}

pub fn stream_kucoin_ticker_socket(market: &str, tx: mpsc::Sender<(String, TickerV2)>) {
    let (mut socket, mut ack) = get_kucoin_ticker_socket(market, &get_kucoin_url());
    let mut last_ping_time = std::time::Instant::now();
    let mut last_best_bid_price: Option<String> = None;
    let mut last_best_ask_price: Option<String> = None;

    loop {
        let read = socket.read();

        match read {
            Ok(message) => {
                match message {
                    Message::Text(msg) => {
                        if msg.contains("pong") {
                            continue;
                        }

                        let parsed_kucoin_ticker: TickerV2 =
                            serde_json::from_str(&msg).expect("Can't parse");

                        let price_changed = match &last_best_bid_price {
                            Some(last_price) => {
                                &parsed_kucoin_ticker.data.best_bid_price != last_price
                            }
                            None => true,
                        } || match &last_best_ask_price {
                            Some(last_price) => {
                                &parsed_kucoin_ticker.data.best_ask_price != last_price
                            }
                            None => true,
                        };

                        if price_changed {
                            tx.send(("kucoin_ticker".to_string(), parsed_kucoin_ticker.clone()))
                                .unwrap();
                            last_best_bid_price =
                                Some(parsed_kucoin_ticker.data.best_bid_price.clone());
                            last_best_ask_price =
                                Some(parsed_kucoin_ticker.data.best_ask_price.clone());
                        }

                        send_ping("Kucoin Ticker V2".to_string(),&mut socket, &mut ack, 18, &mut last_ping_time);
                    }
                    Message::Ping(ping_data) => {
                        // Handle the Ping message, e.g., by sending a Pong response
                        tracing::info!("Ping message recieved from Kucoin Ticker V2");
                        socket.write(Message::Pong(ping_data)).unwrap();
                    }
                    Message::Pong(_pong_data) => {
                        send_ping(format!("Pong Kucoin Ticker V2"), &mut socket, &mut ack, 18, &mut last_ping_time);
                    }
                    other => {
                        tracing::error!("Error: Received unexpected message type in Kucoin Ticker V2 channel: {:?}", other);
                    }
                }
            }

            Err(e) => {
                tracing::error!("Error during Kucoin ticker V2 socket message handling with error: {:?}", e);
                (socket, ack) = get_kucoin_ticker_socket(market, &get_kucoin_url());                    
                tracing::info!("Resubscribed to Kucoin ticker V2 socket.");
                continue;
            }
        }
    }
}
