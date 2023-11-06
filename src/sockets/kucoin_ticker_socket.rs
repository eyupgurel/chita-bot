use crate::models::kucoin_models::{Comm, TickerV2};
use std::net::TcpStream;
use std::sync::mpsc;
use tungstenite::{connect};
use tungstenite::stream::MaybeTlsStream;
use url::Url;
use crate::constants::{KUCOIN_TICKERV2_SOCKET_TOPIC};
use crate::sockets::kucoin_utils::{get_kucoin_url, send_ping};

// Function to handle sending a ping

pub fn get_kucoin_ticker_socket(
    _market: &str,
    _kucoin_futures_wss_url: &String,
) -> (tungstenite::WebSocket<MaybeTlsStream<TcpStream>>, Comm) {
    let (mut kucoin_ticker_socket, _response) =
        connect(Url::parse(&_kucoin_futures_wss_url).unwrap()).expect("Can't connect.");

    // Construct the message
    let sub_message = format!(
        r#"{{
        "type": "subscribe",
        "topic":"{}:{}"
    }}"#,
        KUCOIN_TICKERV2_SOCKET_TOPIC, _market
    );

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

pub fn stream_kucoin_ticker_socket(_market: &str, _tx: mpsc::Sender<(String, TickerV2)>) {
    let (mut kucoin_ticker_socket, mut ack) = get_kucoin_ticker_socket(_market,&get_kucoin_url());
    let mut last_ping_time = std::time::Instant::now();
    loop {
        let read = kucoin_ticker_socket.read();

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

                let parsed_kucoin_ticker: TickerV2 =
                    serde_json::from_str(&msg).expect("Can't parse");

                _tx.send(("kucoin_ticker".to_string(), parsed_kucoin_ticker))
                    .unwrap();

                send_ping(&mut kucoin_ticker_socket, &mut ack, &mut last_ping_time);

            }

            Err(e) => {
                println!("Error during message handling: {:?}", e);
                let (mut new_kucoin_socket, mut new_ack) = get_kucoin_ticker_socket(_market,&get_kucoin_url());
                std::mem::swap(&mut kucoin_ticker_socket, &mut new_kucoin_socket);
                std::mem::swap(&mut ack, &mut new_ack);
                continue;
            }
        }
    }
}

#[test]
fn test_trade_order_message_deserialization() {
    use crate::models::kucoin_models::TradeOrderMessage;
    use std::fs;

    // Read the JSON string from the file
    let json_str = fs::read_to_string("./src/tests/seed/kucoin/trade-orders-per-market.json")
        .expect("Unable to read the file");

    // Deserialize the JSON string into TradeOrderMessage struct
    let parsed_message: TradeOrderMessage =
        serde_json::from_str(&json_str).expect("Failed to parse the JSON");

    // Print and assert or perform tests as necessary
    println!("{:?}", parsed_message);

    // Example assertion
    assert_eq!(parsed_message.message_type, "message");
}
