pub mod client {
    use async_trait::async_trait;
    use log::{debug, error, info, warn};
    use std::net::TcpStream;
    use tungstenite::connect;

    use crate::kucoin::models;

    pub struct KuCoinClient {
        api_gateway: String,
        onboarding_url: String,
        websocket_url: String,
        auth_token: String,
        leverage: u128,
    }

    #[async_trait]
    pub trait ClientMethods {
        async fn init(
            api_gateway: &str,
            onboarding_url: &str,
            websocket_url: &str,
            leverage: u128,
        ) -> KuCoinClient;

        async fn get_token(onboarding_url: &str) -> String;
    }

    #[async_trait]
    impl ClientMethods for KuCoinClient {
        async fn init(
            api_gateway: &str,
            onboarding_url: &str,
            websocket_url: &str,
            leverage: u128,
        ) -> KuCoinClient {
            let mut client = KuCoinClient {
                api_gateway: api_gateway.to_string(),
                onboarding_url: onboarding_url.to_string(),
                websocket_url: websocket_url.to_string(),
                auth_token: "".to_string(),
                leverage,
            };

            client.auth_token = KuCoinClient::get_token(client.onboarding_url.as_str()).await;

            info!("KuCoin client initialized");

            return client;
        }

        async fn get_token(onboarding_url: &str) -> String {
            let client = reqwest::Client::new();

            let resp: models::Response = serde_json::from_str(
                client
                    .post(onboarding_url)
                    .send()
                    .await
                    .unwrap()
                    .text()
                    .await
                    .unwrap()
                    .as_str(),
            )
            .unwrap();

            return resp.data.token;
        }
    }

    #[tokio::test]
    async fn should_create_kucoin_client() {
        let _ = KuCoinClient::init(
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        )
        .await;
    }
}
