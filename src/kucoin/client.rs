pub mod client {
    use crate::utils;
    use async_trait::async_trait;
    use base64::encode;
    use hmac::{Hmac, Mac};
    use log::{debug, error, info, warn};
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use serde_json::json;
    use sha2::Sha256;
    use std::collections::HashMap;
    use std::net::TcpStream;
    use std::time::Duration;
    use tungstenite::connect;

    type HmacSha256 = Hmac<Sha256>;

    use crate::kucoin::models::{Method, Response};

    #[derive(Debug, Clone)]
    pub struct CredentialsV2 {
        pub api_key: String,
        pub secret_key: String,
        pub passphrase: String,
    }

    impl CredentialsV2 {
        pub fn new(api_key: &str, secret_key: &str, passphrase: &str) -> Self {
            CredentialsV2 {
                api_key: api_key.to_string(),
                secret_key: secret_key.to_string(),
                passphrase: passphrase.to_string(),
            }
        }
    }

    pub struct KuCoinClient {
        credentials: CredentialsV2,
        api_gateway: String,
        onboarding_url: String,
        websocket_url: String,
        client: reqwest::Client,
        leverage: u128,
        markets: HashMap<String, String>,
    }

    impl KuCoinClient {
        async fn new(
            credentials: CredentialsV2,
            api_gateway: &str,
            onboarding_url: &str,
            websocket_url: &str,
            leverage: u128,
        ) -> KuCoinClient {
            let mut markets: HashMap<String, String> = HashMap::new();
            markets.insert("ETH-PERP".to_string(), "ETHUSDTM".to_string());
            markets.insert("BTC-PERP".to_string(), "BTCUSDTM".to_string());

            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap();

            let kucoin_client = KuCoinClient {
                credentials,
                api_gateway: api_gateway.to_string(),
                onboarding_url: onboarding_url.to_string(),
                websocket_url: websocket_url.to_string(),
                leverage,
                client,
                markets,
            };

            info!("KuCoin client initialized");

            return kucoin_client;
        }

        async fn get_token(onboarding_url: &str) -> String {
            let client = reqwest::Client::new();

            let resp: Response = serde_json::from_str(
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

        async fn place_limit_order(&self, market: &str, is_buy: bool, price: f64, quantity: f64) {
            let endpoint = String::from("/api/v1/orders");

            let url: String = format!("{}{}", &self.api_gateway, endpoint);
            println!("url: {}", url);

            let side = if is_buy { "buy" } else { "sell" };
            let market_symbol = self.markets.get(market).unwrap();
            println!("market_symbol: {}", market_symbol);

            let oid = utils::get_random_string();
            let mut params: HashMap<String, String> = HashMap::new();
            params.insert(String::from("clientOid"), oid.clone());
            params.insert(String::from("symbol"), market_symbol.to_string());
            params.insert(String::from("side"), side.to_string());
            params.insert(String::from("price"), price.to_string());
            params.insert(String::from("size"), quantity.to_string());
            params.insert(String::from("leverage"), self.leverage.to_string());

            let headers: HeaderMap =
                self.sign_headers(endpoint.clone(), Some(&params), None, Method::POST);

            println!("Headers: {:#?}", headers);

            let res = self
                .client
                .post(url)
                .headers(headers)
                .json(&json!(params))
                .send()
                .await;

            println!("Response: {:#?}", res);
        }

        pub fn sign_headers(
            &self,
            endpoint: String,
            params: Option<&HashMap<String, String>>,
            query: Option<String>,
            method: Method,
        ) -> HeaderMap {
            let mut headers = HeaderMap::new();
            let nonce = utils::get_time().to_string();
            let mut str_to_sign: String = String::new();

            match method {
                Method::GET => {
                    let meth = "GET";
                    if let Some(q) = query {
                        // let query = format_query(&p);
                        str_to_sign = format!("{}{}{}{}", nonce, meth, endpoint, q);
                    } else {
                        str_to_sign = format!("{}{}{}", nonce, meth, endpoint)
                    }
                }
                Method::POST => {
                    let meth = "POST";
                    if let Some(p) = params {
                        let q = json!(&p);
                        str_to_sign = format!("{}{}{}{}", nonce, meth, endpoint, q);
                    } else {
                        str_to_sign = format!("{}{}{}", nonce, meth, endpoint)
                    }
                }
                Method::PUT => {}
                Method::DELETE => {
                    let meth = "DELETE";
                    if let Some(q) = query {
                        // let query = format_query(&p);
                        str_to_sign = format!("{}{}{}{}", nonce, meth, endpoint, q);
                    } else {
                        str_to_sign = format!("{}{}{}", nonce, meth, endpoint)
                    }
                }
            }

            let mut hmac_sign =
                HmacSha256::new_varkey(self.credentials.secret_key.as_str().as_bytes())
                    .expect("HMAC can take key of any size");

            hmac_sign.input(str_to_sign.as_bytes());
            let sign_result = hmac_sign.result();
            let sign_bytes = sign_result.code();
            let sign_digest = encode(&sign_bytes);
            let mut hmac_passphrase =
                HmacSha256::new_varkey(self.credentials.secret_key.as_str().as_bytes())
                    .expect("HMAC can take key of any size");
            hmac_passphrase.input(self.credentials.passphrase.as_str().as_bytes());
            let passphrase_result = hmac_passphrase.result();
            let passphrase_bytes = passphrase_result.code();
            let passphrase_digest = encode(&passphrase_bytes);
            headers.insert(
                HeaderName::from_static("kc-api-key"),
                HeaderValue::from_str(&self.credentials.api_key).unwrap(),
            );
            headers.insert(
                HeaderName::from_static("kc-api-sign"),
                HeaderValue::from_str(&sign_digest).unwrap(),
            );
            headers.insert(
                HeaderName::from_static("kc-api-timestamp"),
                HeaderValue::from_str(&nonce).unwrap(),
            );
            headers.insert(
                HeaderName::from_static("kc-api-passphrase"),
                HeaderValue::from_str(&passphrase_digest).unwrap(),
            );
            headers.insert(
                HeaderName::from_static("kc-api-key-version"),
                HeaderValue::from_str("2").unwrap(),
            );
            return headers;
        }
    }

    #[tokio::test]
    async fn should_create_kucoin_client() {
        let credentials = CredentialsV2::new("key", "secret", "phrase");

        let _ = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        )
        .await;
    }

    #[tokio::test]
    async fn should_post_order_on_kucoin() {}
}
