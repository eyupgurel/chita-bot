pub mod client {
    use async_trait::async_trait;

    #[allow(deprecated)]
    use base64::encode;

    use blake2b_simd::Params;
    use ed25519_dalek::*;
    use serde::Deserialize;
    use serde_json::Value;
    use sha256::digest;
    use std::collections::HashMap;

    // custom modules
    use crate::bluefin::orders::{
        create_market_order, get_serialized_order, to_order_request, Order,
    };

    #[derive(Deserialize, Debug)]
    pub struct Auth {
        pub token: String,
    }

    #[derive(Debug, Clone)]
    pub struct Wallet {
        pub signing_key: SigningKey,
        pub public_key: String,
        pub address: String,
    }

    pub struct BluefinClient {
        wallet: Wallet,
        api_gateway: String,
        onboarding_url: String,
        auth_token: String,
        markets: HashMap<String, String>,
    }

    /**
     * Given a wallet's private key, creates a wallet of ED25519
     */
    pub fn create_wallet(wallet_key: &str) -> Wallet {
        let bytes = hex::decode(wallet_key).expect("Decoding failed");
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

    #[async_trait]
    pub trait ClientMethods {
        async fn init(wallet_key: &str, api_gateway: &str, onboarding_url: &str) -> BluefinClient;
        async fn onboard(&mut self);
        async fn fetch_markets(&mut self);
        fn create_market_order(
            &self,
            market: &str,
            is_buy: bool,
            reduce_only: bool,
            quantity: f64,
            leverage: u128,
        ) -> Order;
        fn sign_order(&self, order: Order) -> String;
        async fn post_signed_order(&self, order: Order, signature: String);
    }

    #[async_trait]
    impl ClientMethods for BluefinClient {
        // creates client object, on-boards the user and fetches market ids
        async fn init(wallet_key: &str, api_gateway: &str, onboarding_url: &str) -> BluefinClient {
            let mut client = BluefinClient {
                wallet: create_wallet(wallet_key),
                api_gateway: api_gateway.to_string(),
                onboarding_url: onboarding_url.to_string(),
                auth_token: "".to_string(),
                markets: HashMap::new(),
            };

            // on-boards user on exchange
            client.onboard().await;

            // get market ids from dapi
            client.fetch_markets().await;

            return client;
        }

        async fn onboard(&mut self) {
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

            let client = reqwest::Client::new();
            let res = client
                .post(format!("{}/authorize", self.api_gateway))
                .json(&body)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            // println!("{:#?}", res);

            let auth: Auth = serde_json::from_str(&res).unwrap();
            self.auth_token = auth.token;
        }

        async fn fetch_markets(&mut self) {
            let client = reqwest::Client::new();

            let markets = ["ETH-PERP", "BTC-PERP"];
            for market in markets.iter() {
                let res = client
                    .get(format!("{}/meta?symbol={}", self.api_gateway, market))
                    .send()
                    .await
                    .unwrap()
                    .text()
                    .await
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

        fn create_market_order(
            &self,
            market: &str,
            is_buy: bool,
            reduce_only: bool,
            quantity: f64,
            leverage: u128,
        ) -> Order {
            // assuming market will exist in markets map
            // TODO add if/else checks
            let market_id = self.markets.get(&market.to_owned()).unwrap().to_string();

            return create_market_order(
                self.wallet.address.clone(),
                market_id,
                is_buy,
                reduce_only,
                quantity,
                leverage,
            );
        }

        fn sign_order(&self, order: Order) -> String {
            let serialized_msg = get_serialized_order(&order);

            let msg_hash_decoded = hex::decode(digest(&serialized_msg)).expect("Decoding failed");
            let sig: Signature = self.wallet.signing_key.sign(&msg_hash_decoded);
            let order_signature = format!(
                "{}1{}",
                sig.to_string().to_ascii_lowercase(),
                &self.wallet.public_key
            );

            return order_signature;
        }

        async fn post_signed_order(&self, order: Order, signature: String) {
            let order_request = to_order_request(order, signature);
            println!("{}", format!("{}/orders", self.api_gateway));

            let client = reqwest::Client::new();
            let res = client
                .post(format!("{}/orders", self.api_gateway))
                .header(
                    "Authorization",
                    format!("Bearer {}", &self.auth_token.to_owned()),
                )
                .json(&order_request)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            println!("Response: {}", res);

            // let v: Value = serde_json::from_str(&res).expect("JSON Decoding failed");
            // let hash: &str = v["hash"].as_str().unwrap();
            // return hash.to_string();
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

    #[tokio::test]
    async fn should_create_bluefin_client() {
        let bluefin_client = BluefinClient::init(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
        )
        .await;
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

    #[tokio::test]
    async fn should_create_market_order() {
        let bluefin_client = BluefinClient::init(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
        )
        .await;

        let order = bluefin_client.create_market_order("ETH-PERP", true, false, 0.33, 1);
        // println!("{:#?}", order);
        assert_eq!(order.orderType, "MARKET");
        assert_eq!(order.postOnly, false);
        assert_eq!(order.quantity, 330000000000000000);
        assert_eq!(order.leverage, 1000000000000000000);
    }

    #[tokio::test]
    async fn should_create_signed_order() {
        let bluefin_client = BluefinClient::init(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
        )
        .await;

        let order = bluefin_client.create_market_order("ETH-PERP", true, false, 0.33, 1);
        let _signature = bluefin_client.sign_order(order);
    }

    #[tokio::test]
    async fn should_place_market_order() {
        let bluefin_client = BluefinClient::init(
            "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
            "https://dapi.api.sui-staging.bluefin.io",
            "https://testnet.bluefin.io",
        )
        .await;

        let order = bluefin_client.create_market_order("ETH-PERP", true, false, 0.33, 1);

        println!("{:#?}", order);

        let signature = bluefin_client.sign_order(order.clone());
        bluefin_client
            .post_signed_order(order.clone(), signature)
            .await;
    }
}
