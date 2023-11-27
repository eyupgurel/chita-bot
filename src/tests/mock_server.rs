use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use warp::ws::Ws;
use warp::Filter;

#[allow(dead_code)]
pub async fn run_server() {
    // Define a warp filter that upgrades the connection to a WebSocket.
    let ws_filter = warp::path("echo").and(warp::ws()).map(|ws: Ws| {
        ws.on_upgrade(|websocket| async move {
            let (mut tx, mut rx) = websocket.split();
            while let Some(result) = rx.next().await {
                match result {
                    Ok(msg) => {
                        // Echo the message back
                        tx.send(msg).await.expect("Failed to send message");
                    }
                    Err(e) => {
                        tracing::error!("Error receiving message: {:?}", e);
                    }
                }
            }
        })
    });

    // Start the warp server on a specific port.
    warp::serve(ws_filter).run(([127, 0, 0, 1], 3030)).await;
}

#[tokio::test]
async fn test_websocket_echo() {
    use tokio::time::{sleep, Duration};
    use tokio_tungstenite::connect_async;

    // Start the mock WebSocket server.
    tokio::spawn(async {
        // You might need to adjust the module path to access `run_server`.
        run_server().await;
    });

    // Give the server a moment to start.
    sleep(Duration::from_secs(1)).await;

    // Connect to the mock WebSocket server.
    let url = "ws://127.0.0.1:3030/echo";
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    // Send a message.
    let msg = "Hello, WebSocket!";
    ws_stream
        .send(msg.into())
        .await
        .expect("Failed to send message");

    // Receive the echoed message.
    let response = ws_stream
        .next()
        .await
        .expect("No message received")
        .expect("Failed to receive message");
    let recv_msg = response.to_text().unwrap();

    // Check if the received message is the same as the sent message.
    assert_eq!(recv_msg, msg);
}
