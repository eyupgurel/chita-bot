#![allow(dead_code)]
pub mod client {
    use log::{debug, error, info, warn};
    use std::net::TcpStream;
    use tungstenite::connect;

    #[allow(deprecated)]
    use base64::encode;

    use blake2b_simd::Params;
    use ed25519_dalek::*;
    use serde_json::{json, Value};
    use sha256::digest;
    use std::collections::HashMap;

    // custom modules
    use crate::bluefin::{
        models::{
            parse_user_position, Auth, Error, OrderUpdate, PostResponse, UserPosition, Wallet,
        },
        orders::{create_order, to_order_request, Order},
    };
    use tungstenite::stream::MaybeTlsStream;
    pub struct BluefinClient {
        wallet: Wallet,
        api_gateway: String,
        onboarding_url: String,
        websocket_url: String,
        pub auth_token: String,
        client: reqwest::blocking::Client,
        leverage: u128,
        markets: HashMap<String, String>,
    }

    /**
     * Given a wallet's private key, creates a wallet of ED25519
     */
    pub fn create_wallet(wallet_key: &str) -> Wallet {
        let mut key = wallet_key;
        // remove 0x from key
        if wallet_key.starts_with("0x") {
            let mut chars = wallet_key.chars();
            chars.next();
            chars.next();
            key = chars.as_str();
        }

        let bytes = hex::decode(key).expect("Decoding failed");
        let mut private_key_bytes: [u8; 32] = [0; 32];
        private_key_bytes.copy_from_slice(&bytes[0..32]);

        let signing_key = SigningKey::from_bytes(&private_key_bytes);

        // Generate the corresponding public key
        let public_key: VerifyingKey = (&signing_key).into();

        // Generate the b64 of the public key
        #[allow(deprecated)]
        let public_key_b64 = encode(&public_key.to_bytes());

        // Append 0x00 to public key due to BIP32
        let public_key_array = public_key.to_bytes();
        let mut public_key_array_bip32 = [0; 33];
        public_key_array_bip32[0] = 0;
        public_key_array_bip32[1..].copy_from_slice(&public_key_array);

        let hash = Params::new()
            .hash_length(32)
            .to_state()
            .update(&public_key_array_bip32)
            .finalize();
        let address = format!("0x{}", &hash.to_hex().to_ascii_lowercase());

        return Wallet {
            signing_key,
            public_key: public_key_b64,
            address,
        };
    }

    impl BluefinClient {
        // creates client object, on-boards the user and fetches market ids
        pub fn new(
            wallet_key: &str,
            api_gateway: &str,
            onboarding_url: &str,
            websocket_url: &str,
            leverage: u128,
        ) -> BluefinClient {
            let mut client = BluefinClient {
                wallet: create_wallet(wallet_key),
                api_gateway: api_gateway.to_string(),
                onboarding_url: onboarding_url.to_string(),
                websocket_url: websocket_url.to_string(),
                auth_token: "".to_string(),
                client: reqwest::blocking::Client::new(),
                markets: HashMap::new(),
                leverage,
            };

            // on-boards user on exchange
            client.onboard();

            // get market ids from dapi
            client.fetch_markets();

            info!(
                "Bluefin client initialized for wallet: {}",
                client.wallet.address
            );

            return client;
        }

        pub fn onboard(&mut self) {
            let mut msg_dict = HashMap::new();
            msg_dict.insert("onboardingUrl", self.onboarding_url.clone());

            let msg_str = serde_json::to_string(&msg_dict).unwrap();
            let mut intent: Vec<u8> = vec![3, 0, 0, msg_str.len() as u8];
            intent.extend_from_slice(msg_str.as_bytes());

            let hash = Params::new()
                .hash_length(32)
                .to_state()
                .update(&intent)
                .finalize();

            let onboarding_sig_temp = self.wallet.signing_key.sign(&hash.as_bytes());
            let onboarding_sig =
                format!("{}1", onboarding_sig_temp.to_string().to_ascii_lowercase());

            let onboarding_sig_full = format!("{}{}", onboarding_sig, &self.wallet.public_key);

            let mut body = HashMap::new();
            body.insert("signature", onboarding_sig_full);
            body.insert("userAddress", self.wallet.address.clone());
            body.insert("isTermAccepted", "True".to_string());

            let res = self
                .client
                .post(format!("{}/authorize", self.api_gateway))
                .json(&body)
                .send()
                .unwrap()
                .text()
                .unwrap();

            let response: Value = serde_json::from_str(&res).unwrap();
            if response["error"].is_object() {
                let error: Error = serde_json::from_str(&response["error"].to_string()).unwrap();
                error!(
                    "Error while onboarding - code: {}, message: {}",
                    error.code, error.message
                );
                panic!()
            }

            let auth: Auth = serde_json::from_str(&res).unwrap();
            self.auth_token = auth.token;
        }

        pub fn fetch_markets(&mut self) {
            let markets = ["ETH-PERP", "BTC-PERP"];
            for market in markets.iter() {
                let res = self
                    .client
                    .get(format!("{}/meta?symbol={}", self.api_gateway, market))
                    .send()
                    .unwrap()
                    .text()
                    .unwrap();

                let v: Value = serde_json::from_str(&res).expect("JSON Decoding failed");
                let v1: Value = serde_json::from_str(&v["perpetualAddress"].to_string())
                    .expect("JSON Decoding failed");
                let market_id_value: Value =
                    serde_json::from_str(&v1["id"].to_string()).expect("JSON Decoding failed");
                let market_id = market_id_value.as_str().unwrap();

                self.markets
                    .insert(market.to_string(), market_id.to_string());
            } // end of for loop
        }

        pub fn get_user_position(&self, market: &str) -> UserPosition {
            let query = vec![("symbol", market)];

            let res = self
                .client
                .get(format!("{}/userPosition", self.api_gateway))
                .header(
                    "Authorization",
                    format!("Bearer {}", &self.auth_token.to_owned()),
                )
                .query(&query)
                .send()
                .unwrap()
                .text()
                .unwrap();

            // let resp = serde_json::from_str(&res).unwrap();

            let position: Value = serde_json::from_str(&res).expect("JSON Decoding failed");

            // if user address key does not exist, implies that the user has no position
            // assuming the get call did not throw error
            // TODO check for error as well
            if position["userAddress"].is_string() {
                return parse_user_position(position);
            } else {
                return UserPosition {
                    symbol: market.to_string(),
                    side: false,
                    quantity: 0,
                    avg_entry_price: 0,
                    margin: 0,
                    leverage: 0,
                };
            };
        }

        pub fn create_limit_ioc_order(
            &self,
            market: &str,
            is_buy: bool,
            reduce_only: bool,
            price: f64,
            quantity: f64,
            leverage: Option<u128>,
        ) -> Order {
            // assuming market will exist in markets map
            // TODO add if/else checks
            let market_id = self.markets.get(market).unwrap().to_string();

            let leverage = leverage.unwrap_or_else(|| self.leverage);

            return create_order(
                self.wallet.address.clone(),
                market.to_string(),
                market_id,
                is_buy,
                reduce_only,
                Some(price),
                quantity,
                leverage,
            );
        }

        pub fn create_market_order(
            &self,
            market: &str,
            is_buy: bool,
            reduce_only: bool,
            quantity: f64,
            leverage: Option<u128>,
        ) -> Order {
            // assuming market will exist in markets map
            // TODO add if/else checks
            let market_id = self.markets.get(market).unwrap().to_string();

            let leverage = leverage.unwrap_or_else(|| self.leverage);

            return create_order(
                self.wallet.address.clone(),
                market.to_string(),
                market_id,
                is_buy,
                reduce_only,
                None,
                quantity,
                leverage,
            );
        }

        pub fn sign_order(&self, order: Order) -> String {
            let msg_hash_decoded = hex::decode(digest(&order.serialized)).expect("Decoding failed");
            let sig: Signature = self.wallet.signing_key.sign(&msg_hash_decoded);
            let order_signature = format!(
                "{}1{}",
                sig.to_string().to_ascii_lowercase(),
                &self.wallet.public_key
            );

            return order_signature;
        }

        pub fn post_signed_order(&self, order: Order, signature: String) -> PostResponse {
            let order_request = to_order_request(order, signature);

            let res = self
                .client
                .post(format!("{}/orders", self.api_gateway))
                .header(
                    "Authorization",
                    format!("Bearer {}", &self.auth_token.to_owned()),
                )
                .json(&order_request)
                .send()
                .unwrap()
                .text()
                .unwrap();

            debug!("Response: {}", res);

            let response: Value = serde_json::from_str(&res).expect("JSON Decoding failed");

            if response["error"].is_object() {
                let error: Error = serde_json::from_str(&response["error"].to_string()).unwrap();
                warn!("Order placement failed: {}", error.message);
                return PostResponse { error: Some(error) };
            } else {
                return PostResponse { error: None };
            }
        }

        pub fn listen_to_web_socket(&self) {
            // helper function to connect with websocket
            fn connect_socket(
                url: String,
                auth_token: String,
            ) -> tungstenite::WebSocket<MaybeTlsStream<TcpStream>> {
                let (mut bluefin_socket, _) =
                    connect(url::Url::parse(url.as_str()).unwrap()).expect("Failed to connect");
                info!("Connected to Bluefin stream at url:{}.", &url);

                let request = json!([
                    "SUBSCRIBE",
                    [
                        {
                            "e": "userUpdates",
                            "rt": "",
                            "t": auth_token
                        }
                    ]
                ]);

                // Send message
                if let Err(err) =
                    bluefin_socket.send(tungstenite::protocol::Message::Text(request.to_string()))
                {
                    error!("Error subscribing to user updates: {err:?}");
                } else {
                    info!("Subscribed to user update room");
                };

                return bluefin_socket;
            }

            let mut bluefin_socket =
                connect_socket(self.websocket_url.clone(), self.auth_token.clone());

            loop {
                let read = bluefin_socket.read();

                match read {
                    Ok(message) => {
                        let msg = match message {
                            tungstenite::Message::Text(s) => s,
                            _ => "Error handling message".to_string(),
                        };

                        // ignore all events except order update and account data update
                        if !(msg.contains("OrderUpdate") || msg.contains("PositionUpdate")) {
                            continue;
                        }

                        // println!("{}", msg);

                        if msg.contains("OrderUpdate") {
                            let v: Value = serde_json::from_str(&msg).unwrap();
                            let order_update: OrderUpdate =
                                serde_json::from_str(&v["data"]["order"].to_string())
                                    .expect("Can't parse");

                            println!("order_update: {:#?}", order_update);
                        } else {
                            let v: Value = serde_json::from_str(&msg).unwrap();
                            let position_update =
                                parse_user_position(v["data"]["position"].clone());
                            println!("position_update: {:#?}", position_update);
                        };
                    }

                    Err(e) => {
                        warn!("Error during message handling: {:?}", e);
                        bluefin_socket =
                            connect_socket(self.websocket_url.clone(), self.auth_token.clone())
                    }
                }
            }
        }
    }

    #[test]
    fn should_create_wallet() {
        let wallet =
            create_wallet("c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270");
        assert_eq!(
            wallet.address,
            "0xc6c71c996d437eb6589d1b8b17afcd1480afd5f30f6b7155ef468a9713d3240e",
        );
    }

    #[test]
    fn should_create_bluefin_client() {
        let bluefin_client = BluefinClient::new(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
            "wss://notifications.api.sui-staging.bluefin.io",
            1,
        );

        assert_eq!(
            bluefin_client.wallet.address,
            "0xc6c71c996d437eb6589d1b8b17afcd1480afd5f30f6b7155ef468a9713d3240e"
        );

        assert_eq!(
            bluefin_client.api_gateway,
            "https://dapi.api.sui-staging.bluefin.io"
        );
        assert_eq!(bluefin_client.onboarding_url, "https://testnet.bluefin.io");

        assert_eq!(
            bluefin_client.markets.get("ETH-PERP").unwrap(),
            "0xf4b34d977e09ef15c63736ccd6126eb10b54f910f33394ae7c2454d4c144d6ea"
        );

        assert_eq!(
            bluefin_client.markets.get("BTC-PERP").unwrap(),
            "0x252abf8630394f08e458c6329fab0d6c103ca2f403e05e7913e024f63d0e992d"
        );
    }

    #[test]

    fn should_get_user_position_on_bluefin() {
        let bluefin_client = BluefinClient::new(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
            "wss://notifications.api.sui-staging.bluefin.io",
            1,
        );

        let position: UserPosition = bluefin_client.get_user_position("ETH-PERP");
        assert_eq!(position.quantity, 0);
    }

    #[test]
    fn should_create_limit_ioc_order() {
        let bluefin_client = BluefinClient::new(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
            "wss://notifications.api.sui-staging.bluefin.io",
            1,
        );

        let order =
            bluefin_client.create_limit_ioc_order("ETH-PERP", true, false, 1600.1, 0.33, Some(1));
        // println!("{:#?}", order);
        assert_eq!(order.orderType, "LIMIT");
        assert_eq!(order.timeInForce, "IOC");
        assert_eq!(order.postOnly, false);
        assert_eq!(order.price, 1600100000000000000000);
        assert_eq!(order.quantity, 330000000000000000);
        assert_eq!(order.leverage, 1000000000000000000);
    }

    #[test]
    fn should_create_signed_order() {
        let bluefin_client = BluefinClient::new(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
            "wss://notifications.api.sui-staging.bluefin.io",
            1,
        );

        let order =
            bluefin_client.create_limit_ioc_order("ETH-PERP", true, false, 1600.0, 0.33, Some(1));
        let _signature = bluefin_client.sign_order(order);
    }

    #[test]
    fn should_revert_when_placing_order_due_to_insufficient_balance() {
        let bluefin_client = BluefinClient::new(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
            "wss://notifications.api.sui-staging.bluefin.io",
            1,
        );

        let order =
            bluefin_client.create_limit_ioc_order("ETH-PERP", true, false, 1600.67, 10.0, Some(1));

        let signature = bluefin_client.sign_order(order.clone());
        let status = bluefin_client.post_signed_order(order.clone(), signature);
        assert_eq!(status.error.unwrap().code, 3055);
    }

    #[test]
    fn should_place_order_successfully() {
        let bluefin_client = BluefinClient::new(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
            "wss://notifications.api.sui-staging.bluefin.io",
            3,
        );

        let order =
            bluefin_client.create_limit_ioc_order("ETH-PERP", true, false, 1600.67, 0.01, None);

        let signature = bluefin_client.sign_order(order.clone());
        let status = bluefin_client.post_signed_order(order.clone(), signature);

        assert!(status.error.is_none(), "Error while placing order");
    }

    #[test]
    fn should_place_a_market_order() {
        let bluefin_client = BluefinClient::new(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
            "wss://notifications.api.sui-staging.bluefin.io",
            3,
        );

        let order = bluefin_client.create_market_order("ETH-PERP", false, false, 0.01, None);

        let signature = bluefin_client.sign_order(order.clone());
        let status = bluefin_client.post_signed_order(order.clone(), signature);
        assert!(status.error.is_none(), "Error while placing order");
    }

    #[test]
    fn should_connect_bluefin_websocket() {
        let bluefin_client = BluefinClient::new(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
            "wss://notifications.api.sui-staging.bluefin.io",
            1,
        );

        let _ = bluefin_client.listen_to_web_socket();
    }
}
