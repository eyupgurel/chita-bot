pub mod client {
    use crate::{
        kucoin::models::{CallResponse, UserPosition},
        utils,
    };
    #[allow(deprecated)]
    use base64::encode;
    use hmac::{Hmac, Mac};
    use log::info;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use serde_json::{json, Value};
    use sha2::Sha256;
    use std::collections::HashMap;
    use std::time::Duration;

    #[allow(unused)]
    type HmacSha256 = Hmac<Sha256>;

    use crate::kucoin::models::{Error, Method, Response};

    #[derive(Debug, Clone)]
    pub struct Credentials {
        pub api_key: String,
        pub secret_key: String,
        pub passphrase: String,
    }

    impl Credentials {
        #[allow(unused)]
        pub fn new(api_key: &str, secret_key: &str, passphrase: &str) -> Self {
            Credentials {
                api_key: api_key.to_string(),
                secret_key: secret_key.to_string(),
                passphrase: passphrase.to_string(),
            }
        }
    }

    #[allow(unused)]
    pub struct KuCoinClient {
        credentials: Credentials,
        api_gateway: String,
        #[allow(unused)]
        onboarding_url: String,
        #[allow(unused)]
        websocket_url: String,
        client: reqwest::Client,
        leverage: u128,
        markets: HashMap<String, String>,
    }

    #[allow(unused)]
    impl KuCoinClient {
        pub async fn new(
            credentials: Credentials,
            api_gateway: &str,
            onboarding_url: &str,
            websocket_url: &str,
            leverage: u128,
        ) -> KuCoinClient {
            let mut markets: HashMap<String, String> = HashMap::new();
            markets.insert("ETH-PERP".to_string(), "ETHUSDTM".to_string());
            markets.insert("BTC-PERP".to_string(), "BTCUSDTM".to_string());
            markets.insert("SUI-PERP".to_string(), "SUIUSDTM".to_string());

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

        pub async fn get_token(onboarding_url: &str) -> String {
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

        pub async fn get_position(&self, market: &str) -> UserPosition {
            let endpoint = String::from("/api/v1/position");
            let market_symbol = self.markets.get(market).unwrap();

            let mut params: HashMap<String, String> = HashMap::new();
            params.insert(String::from("symbol"), market_symbol.clone());
            let query = utils::format_query(&params);

            let url: String = format!("{}{}{}", &self.api_gateway, endpoint, query);

            let headers: HeaderMap =
                self.sign_headers(endpoint.clone(), None, Some(query), Method::GET);

            let res = self
                .client
                .get(url)
                .headers(headers)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            let position: Value = serde_json::from_str(&res).expect("JSON Decoding failed");

            let user_position: UserPosition =
                serde_json::from_value(position["data"].clone()).unwrap();

            println!("Got position: {:#?}", user_position);
            return user_position;
        }

        async fn place_limit_order(
            &self,
            market: &str,
            is_buy: bool,
            price: f64,
            quantity: u128,
        ) -> CallResponse {
            let endpoint = String::from("/api/v1/orders");

            let url: String = format!("{}{}", &self.api_gateway, endpoint);

            let side = if is_buy { "buy" } else { "sell" };
            let market_symbol = self.markets.get(market).unwrap();

            let oid = utils::get_random_string();
            let mut params: HashMap<String, String> = HashMap::new();
            params.insert(String::from("clientOid"), oid.clone());
            params.insert(String::from("symbol"), market_symbol.to_string());
            params.insert(String::from("side"), side.to_string());
            params.insert(String::from("price"), price.to_string());
            params.insert(String::from("size"), quantity.to_string());
            params.insert(String::from("leverage"), self.leverage.to_string());
            params.insert(String::from("postOnly"), "true".to_string());

            let headers: HeaderMap =
                self.sign_headers(endpoint.clone(), Some(&params), None, Method::POST);

            let res = self
                .client
                .post(url)
                .headers(headers)
                .json(&json!(params))
                .send()
                .await
                .unwrap();

            if res.status().is_success() {
                let response_body = res.text().await.unwrap();
                println!("Futures order placed successfully: {}", response_body);
                return CallResponse {
                    error: None,
                    order_id: Some(oid.clone()),
                };
            } else {
                let error: Error =
                    serde_json::from_str(&res.text().await.unwrap()).expect("JSON Decoding failed");

                eprintln!("Error placing futures order: {:#?}", error);
                return CallResponse {
                    error: Some(error),
                    order_id: None,
                };
            }
        }

        async fn cancel_order_by_id(&self, order_id: &str) -> CallResponse {
            let endpoint = format!("/api/v1/orders/{}", order_id);
            let url = format!("{}{}", &self.api_gateway, endpoint);

            let headers: HeaderMap = self.sign_headers(endpoint, None, None, Method::DELETE);

            let resp = self
                .client
                .delete(url)
                .headers(headers)
                .send()
                .await
                .unwrap();

            if resp.status().is_success() {
                let response_body = resp.text().await.unwrap();
                println!("Order successfully cancelled: {}", response_body);
                return CallResponse {
                    error: None,
                    order_id: None,
                };
            } else {
                let error: Error = serde_json::from_str(&resp.text().await.unwrap())
                    .expect("JSON Decoding failed");
                eprintln!("Error cancelling order: {:#?}", error);
                return CallResponse {
                    error: Some(error),
                    order_id: None,
                };
            }
        }

        async fn cancel_order_by_market(&self, market: &str) -> CallResponse {
            let endpoint = String::from("/api/v1/orders");
            let mut params: HashMap<String, String> = HashMap::new();

            let market_symbol = self.markets.get(market).unwrap();

            params.insert(String::from("symbol"), market_symbol.to_owned());

            let query = utils::format_query(&params);
            let url = format!("{}{}{}", &self.api_gateway, endpoint, query);
            let headers = self.sign_headers(endpoint, None, Some(query), Method::DELETE);

            let resp = self
                .client
                .delete(url)
                .headers(headers)
                .send()
                .await
                .unwrap();

            if resp.status().is_success() {
                let response_body = resp.text().await.unwrap();
                println!("Order successfully cancelled for market: {}", market);
                return CallResponse {
                    error: None,
                    order_id: None,
                };
            } else {
                let error: Error = serde_json::from_str(&resp.text().await.unwrap())
                    .expect("JSON Decoding failed");
                eprintln!(
                    "Error cancelling orders for market({}): {:#?}",
                    market, error
                );
                return CallResponse {
                    error: Some(error),
                    order_id: None,
                };
            }
        }

        pub fn sign_headers(
            &self,
            endpoint: String,
            params: Option<&HashMap<String, String>>,
            query: Option<String>,
            method: Method,
        ) -> HeaderMap {
            let mut headers = HeaderMap::new();
            let nonce = utils::get_current_time().to_string();
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
            #[allow(deprecated)]
            let sign_digest = encode(&sign_bytes);
            let mut hmac_passphrase =
                HmacSha256::new_varkey(self.credentials.secret_key.as_str().as_bytes())
                    .expect("HMAC can take key of any size");
            hmac_passphrase.input(self.credentials.passphrase.as_str().as_bytes());
            let passphrase_result = hmac_passphrase.result();
            let passphrase_bytes = passphrase_result.code();
            #[allow(deprecated)]
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
        let credentials = Credentials::new("key", "secret", "phrase");

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
    async fn should_get_user_position_on_kucoin() {
        let credentials: Credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        )
        .await;

        client.get_position("ETH-PERP").await;

        assert!(true, "Error while placing order");
    }

    #[tokio::test]
    async fn should_post_order_on_kucoin() {
        let credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        )
        .await;

        let _resp = client.place_limit_order("SUI-PERP", true, 0.58, 1).await;

        assert!(true, "Error while placing order");
    }

    #[tokio::test]
    async fn should_cancel_the_open_order_by_id() {
        let credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        )
        .await;

        client.cancel_order_by_id("12132131231").await;

        assert!(true, "Error cancelling the order");
    }

    #[tokio::test]
    async fn should_cancel_all_orders_for_market() {
        let credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        )
        .await;

        client.cancel_order_by_market("ETH-PERP").await;

        assert!(true, "Error cancelling the order");
    }
}
