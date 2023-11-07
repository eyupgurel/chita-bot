use crate::models::bluefin_models::BluefinOrderBook;
use futures_util::StreamExt;
use std::sync::mpsc;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

#[allow(dead_code)]
pub async fn get_bluefin_socket(
    url: &str,
) -> WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let (bluefin_socket, _response) = connect_async(url).await.expect("Failed to connect");
    println!("Connected to bluefin stream.");
    println!("HTTP status code: {}", _response.status());
    println!("Response headers:");
    for (ref header, ref header_value) in _response.headers() {
        println!("- {}: {:?}", header, header_value);
    }
    return bluefin_socket;
}
#[allow(dead_code)]
pub async fn stream_bluefin_socket(
    mut bluefin_socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    _tx: mpsc::Sender<(String, BluefinOrderBook)>,
) {
    loop {
        let response = bluefin_socket
            .next()
            .await
            .expect("No message received")
            .expect("Failed to receive message");
        let recv_msg = response.to_text().unwrap();
        let parsed: BluefinOrderBook = serde_json::from_str(&recv_msg).expect("Can't parse");
        _tx.send(("bluefin_ob".to_string(), parsed)).unwrap();
        println!("recv_msg: {:?}", recv_msg);
    }
}

#[tokio::test]
async fn mock_bluefin_websocket() {
    use crate::tests::mock_server::run_server;
    use futures_util::SinkExt;
    use std::fs;
    use tokio::time::{sleep, Duration};

    // Start the mock WebSocket server.
    tokio::spawn(async {
        run_server().await;
    });

    sleep(Duration::from_secs(1)).await;
    tokio::spawn(async {
        let url = "ws://127.0.0.1:3030/echo";
        let mut bluefin_socket = get_bluefin_socket(url).await;
        let json_str = fs::read_to_string("./src/tests/seed/bluefin/bluefin-partial-depth.json")
            .expect("Unable to read the file");
        bluefin_socket
            .send(json_str.into())
            .await
            .expect("Failed to send message");
        let (tx, _) = mpsc::channel();
        stream_bluefin_socket(bluefin_socket, tx).await;
    });

    // Give the server a moment to start.
    sleep(Duration::from_secs(20)).await;
}
