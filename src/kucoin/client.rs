pub mod client {
    use crate::env::EnvVars;
    use crate::{
        env,
        kucoin::models::{CallResponse, UserPosition},
        utils,
    };
    #[allow(deprecated)]
    use base64::encode;
    use hmac::{Hmac, Mac};
    use log::{debug, info, warn};
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use serde_json::{json, Value};
    use sha2::Sha256;
    use snailquote::unescape;
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[allow(unused)]
    type HmacSha256 = Hmac<Sha256>;

    use crate::kucoin::models::{Error, FillsResponse, Method, RecentFillsResponse, Response};

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
        client: reqwest::blocking::Client,
        leverage: u128,
        markets: HashMap<String, String>,
    }

    #[allow(unused)]
    impl KuCoinClient {
        pub fn new(
            credentials: Credentials,
            api_gateway: &str,
            onboarding_url: &str,
            websocket_url: &str,
            leverage: u128,
        ) -> KuCoinClient {
            // TODO read from config and pass as variable when creating kucoin client
            let mut markets: HashMap<String, String> = HashMap::new();
            markets.insert("ETH-PERP".to_string(), "ETHUSDTM".to_string());
            markets.insert("BTC-PERP".to_string(), "XBTUSDTM".to_string());
            markets.insert("SUI-PERP".to_string(), "SUIUSDTM".to_string());

            let client = reqwest::blocking::Client::builder()
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

        pub fn get_private_token(&self) -> String {
            let endpoint = String::from("/api/v1/bullet-private");

            let url: String = format!("{}{}", &self.api_gateway, endpoint);

            let headers: HeaderMap = self.sign_headers(endpoint.clone(), None, None, Method::POST);

            let res = self.client.post(url).headers(headers).send().unwrap();

            let body = res.text().unwrap();

            let resp: Response = serde_json::from_str(&body).expect("JSON Decoding failed");

            resp.data.token
        }

        pub fn get_kucoin_private_socket_url(&self) -> String {
            let vars: EnvVars = env::env_variables();
            let token = self.get_private_token();
            let kucoin_futures_wss_url = format!("{}?token={}", &vars.kucoin_websocket_url, token);
            kucoin_futures_wss_url
        }

        pub fn get_token(onboarding_url: &str) -> String {
            let client = reqwest::blocking::Client::new();

            let resp: Response = serde_json::from_str(
                client
                    .post(onboarding_url)
                    .send()
                    .unwrap()
                    .text()
                    .unwrap()
                    .as_str(),
            )
            .unwrap();

            return resp.data.token;
        }

        pub fn get_recent_fills(&self, market: &str) -> RecentFillsResponse {
            let endpoint = String::from("/api/v1/recentFills");

            let market_symbol = self.markets.get(market).unwrap();

            let mut params: HashMap<String, String> = HashMap::new();
            params.insert(String::from("symbol"), market_symbol.clone());

            let query = utils::format_query(&params);

            let url: String = format!("{}{}{}", &self.api_gateway, endpoint, query);

            let headers: HeaderMap =
                self.sign_headers(endpoint.clone(), None, Some(query), Method::GET);

            let res: String = self
                .client
                .get(url)
                .headers(headers)
                .send()
                .unwrap()
                .text()
                .unwrap();

            let resp: RecentFillsResponse = serde_json::from_str(&res).expect("JSON Decoding failed");

            return resp;
        }

        pub fn get_fills(&self, market: &str, side: Option<&str>, order_type: Option<&str>, start_at: Option<u128>, end_at: Option<u128>, current_page: Option<u64>, page_size: Option<u64>) -> FillsResponse {
            let endpoint = String::from("/api/v1/fills");

            let market_symbol = self.markets.get(market).unwrap();

            let mut params: HashMap<String, String> = HashMap::new();

            params.insert(String::from("symbol"), market_symbol.clone());

            if let Some(s) = side {
                params.insert(String::from("side"), String::from(s));
            };

            if let Some(s) = order_type {
                params.insert(String::from("type"), String::from(s));
            };

            if let Some(u) = start_at {
                params.insert(String::from("startAt"), u.to_string());
            };

            if let Some(u) = end_at {
                params.insert(String::from("endAt"), u.to_string());
            };

            if let Some(u) = current_page {
                params.insert(String::from("currenPage"), u.to_string());
            };

            if let Some(u) = page_size {
                params.insert(String::from("pageSize"), u.to_string());
            };


            let query = utils::format_query(&params);

            let url: String = format!("{}{}{}", &self.api_gateway, endpoint, query);

            let headers: HeaderMap =
                self.sign_headers(endpoint.clone(), None, Some(query), Method::GET);

            let res: String = self
                .client
                .get(url)
                .headers(headers)
                .send()
                .unwrap()
                .text()
                .unwrap();

            let resp: FillsResponse = serde_json::from_str(&res).expect("JSON Decoding failed");

            return resp;
        }

        pub fn get_fill_size_for_time_window(&self, market: &str, side: &str, since: u128) -> i32 {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("could not get current time since unix epoch").as_millis();
            let fills = self.get_fills(market,Some(side), None,Some(since), Some(now), None, Some(1000));
            let total_size = fills.data.items.iter().fold(0,|acc,trade| acc + trade.size);
            total_size
        }
        pub fn get_position(&self, market: &str) -> Option<UserPosition> {
            let endpoint = String::from("/api/v1/position");
            let market_symbol = self.markets.get(market).unwrap();

            let mut params: HashMap<String, String> = HashMap::new();
            params.insert(String::from("symbol"), market_symbol.clone());
            let query = utils::format_query(&params);

            let url: String = format!("{}{}{}", &self.api_gateway, endpoint, query);

            let headers: HeaderMap =
                self.sign_headers(endpoint.clone(), None, Some(query), Method::GET);

            let res: String = self
                .client
                .get(url)
                .headers(headers)
                .send()
                .unwrap()
                .text()
                .unwrap();

            let value: Value = serde_json::from_str(&res).expect("JSON Decoding failed");

            if value["code"].to_string().eq("\"200000\"") {
                let user_position: UserPosition =
                    serde_json::from_value(value["data"].clone()).unwrap();

                debug!("Got position: {:#?}", user_position);
                return Some(user_position);
            } else {
                let error: Error = serde_json::from_str(&res).expect("JSON Decoding failed");
                warn!("Error getting kucoin position: {:#?}", error);
                return None;
            }
        }

        pub fn place_limit_order(
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

            let mut params: HashMap<String, String> = HashMap::new();
            params.insert(String::from("clientOid"), utils::get_random_string());
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
                .unwrap();

            let response_body: String = res.text().unwrap();
            let value: Value = serde_json::from_str(&response_body).expect("JSON Decoding failed");

            if value["code"].to_string().eq("\"200000\"") {
                eprintln!("Futures order placed successfully: {}", response_body);
                let order_id = unescape(value["data"]["orderId"].as_str().unwrap()).unwrap();
                info!("order_id {}", order_id);
                return CallResponse {
                    error: None,
                    order_id: Some(order_id),
                };
            } else {
                let error: Error =
                    serde_json::from_str(&response_body).expect("JSON Decoding failed");

                warn!("Error placing futures order: {:#?}", error);
                return CallResponse {
                    error: Some(error),
                    order_id: None,
                };
            }
        }

        pub fn cancel_order_by_id(&self, order_id: &str) -> CallResponse {
            let endpoint = format!("/api/v1/orders/{}", order_id);
            let url = format!("{}{}", &self.api_gateway, endpoint);

            let headers: HeaderMap = self.sign_headers(endpoint, None, None, Method::DELETE);

            let resp = self.client.delete(url).headers(headers).send().unwrap();

            let response_body: String = resp.text().unwrap();
            let value: Value = serde_json::from_str(&response_body).expect("JSON Decoding failed");

            if value["code"].to_string().eq("\"200000\"") {
                info!("Order successfully cancelled: {}", response_body);
                return CallResponse {
                    error: None,
                    order_id: None,
                };
            } else {
                let error: Error =
                    serde_json::from_str(response_body.as_str()).expect("JSON Decoding failed");
                warn!("Error cancelling order: {:#?}", error);
                return CallResponse {
                    error: Some(error),
                    order_id: None,
                };
            }
        }

        pub fn cancel_all_orders(&self, market: Option<&str>) -> CallResponse {
            let endpoint: String = String::from("/api/v1/orders");
            let url: String;
            let headers: HeaderMap;
            let mut params: HashMap<String, String> = HashMap::new();

            if let Some(s) = market {
                let market_symbol: &String = self.markets.get(s).unwrap();
                params.insert(String::from("symbol"), market_symbol.to_owned());
            };

            if !params.is_empty() {
                let query = utils::format_query(&params);
                url = format!("{}{}{}", &self.api_gateway, endpoint, query);
                headers = self.sign_headers(endpoint, None, Some(query), Method::DELETE);
            } else {
                url = format!("{}{}", &self.api_gateway, endpoint);
                headers = self.sign_headers(endpoint, None, None, Method::DELETE);
            }

            let resp = self.client.delete(url).headers(headers).send().unwrap();

            if resp.status().is_success() {
                let response_body = resp.text().unwrap();
                debug!("Order successfully cancelled");
                return CallResponse {
                    error: None,
                    order_id: None,
                };
            } else {
                let error: Error =
                    serde_json::from_str(&resp.text().unwrap()).expect("JSON Decoding failed");
                warn!("Error cancelling orders: {:#?}", error);
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

    #[test]
    fn should_create_kucoin_client() {
        let credentials = Credentials::new("key", "secret", "phrase");

        let _ = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );
    }

    #[test]
    fn should_get_user_position_on_kucoin() {
        let credentials: Credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );

        client.get_position("ETH-PERP");

        assert!(true, "Error while placing order");
    }

    #[test]
    fn should_post_order_on_kucoin() {
        let credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );

        let resp: CallResponse = client.place_limit_order("SUI-PERP", true, 0.70, 1);

        println!("Placed order with id: {}", resp.order_id.unwrap());

        assert!(true, "Error while placing order");
    }

    #[test]
    fn should_cancel_the_open_order_by_id() {
        let credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );

        client.cancel_order_by_id("12132131231");

        assert!(true, "Error cancelling the order");
    }

    #[test]
    fn should_cancel_all_orders_for_eth_market() {
        let credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );

        let resp = client.cancel_all_orders(Some("ETH-PERP"));

        assert!(
            resp.error.is_none(),
            "Error cancelling all orders for ETH market"
        );
    }

    #[test]
    fn should_cancel_all_orders_for_all_markets() {
        let credentials = Credentials::new("1", "2", "3");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );

        let resp = client.cancel_all_orders(None);

        assert!(
            resp.error.is_none(),
            "Error cancelling all orders for all markets"
        );
    }
    #[test]
    fn should_get_recent_fills() {
        let credentials = Credentials::new("654bad2744b9f1000170a857", "cc0f02dd-9070-4f65-8d60-8bc0d6bfcd8a", "6aabPMdj!!4Xt3Y&");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );

        let resp = client.get_recent_fills("ETH-PERP");

        assert_eq!(resp.code, "200000", "Error getting recent fills");
    }

    #[test]
    fn should_get_fills() {
        let credentials = Credentials::new("654bad2744b9f1000170a857", "cc0f02dd-9070-4f65-8d60-8bc0d6bfcd8a", "6aabPMdj!!4Xt3Y&");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );

        let resp = client.get_fills("ETH-PERP", None, None, Some(1700665497000), Some(1700838297000), None, None);

        assert_eq!(resp.code, "200000", "Error getting recent fills");
    }

    #[test]
    fn should_get_total_fill_size() {
        let credentials = Credentials::new("654bad2744b9f1000170a857", "cc0f02dd-9070-4f65-8d60-8bc0d6bfcd8a", "6aabPMdj!!4Xt3Y&");

        let client = KuCoinClient::new(
            credentials,
            "https://api-futures.kucoin.com",
            "https://api-futures.kucoin.com/api/v1/bullet-public",
            "wss://ws-api-futures.kucoin.com/endpoint",
            3,
        );

        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("could not get current time since unix epoch").as_millis();
        let since = now - 1000 * 60 * 60 * 24;
        let total_buy_size = client.get_fill_size_for_time_window("BTC-PERP", "buy", since);
        let total_sell_size = client.get_fill_size_for_time_window("BTC-PERP", "sell", since);
        let buy_percent = (total_buy_size as f64 / ((total_buy_size + total_sell_size) as f64)) * 100.0;

        assert_eq!(total_buy_size, 1, "Error getting recent fills");
        assert_eq!(total_sell_size, 5, "Error getting recent fills");
    }
}
