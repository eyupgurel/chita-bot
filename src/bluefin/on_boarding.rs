use base64::encode;
use blake2b_simd::Params;
use ed25519_dalek::*;
use hex;
use reqwest;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct AUTH {
    pub token: String,
}

pub async fn on_board(
    wallet_key: &str,
    bluefin_on_boarding_url: &str,
    bluefin_endpoint: &str,
) -> String {
    let bytes = hex::decode(wallet_key).expect("Decoding failed");
    let mut private_key_bytes: [u8; 32] = [0; 32];
    private_key_bytes.copy_from_slice(&bytes[0..32]);

    let signing_key = SigningKey::from_bytes(&private_key_bytes);

    // Generate the corresponding public key
    let public_key: VerifyingKey = (&signing_key).into();

    // Generate the b64 of the public key
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
    let wallet_address = "0x".to_string() + &hash.to_hex().to_ascii_lowercase();

    let mut msg_dict = HashMap::new();
    msg_dict.insert("onboardingUrl", bluefin_on_boarding_url);

    let msg_str = serde_json::to_string(&msg_dict).unwrap();
    let mut intent: Vec<u8> = vec![3, 0, 0, msg_str.len() as u8];
    intent.extend_from_slice(msg_str.as_bytes());

    let hash = Params::new()
        .hash_length(32)
        .to_state()
        .update(&intent)
        .finalize();

    let onboarding_sig_temp = signing_key.sign(&hash.as_bytes());
    let onboarding_sig = onboarding_sig_temp.to_string().to_ascii_lowercase() + "1";

    let onboarding_sig_full = onboarding_sig + &public_key_b64;

    let mut body = HashMap::new();
    body.insert("signature", onboarding_sig_full);
    body.insert("userAddress", wallet_address);
    body.insert("isTermAccepted", "True".to_string());

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{bluefin_endpoint}/authorize"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let auth_token: AUTH = serde_json::from_str(&res).unwrap();
    // println!("{:#?}", auth_token);
    return auth_token.token;
}

#[tokio::test]
async fn should_on_board_user() {
    let auth_token = on_board(
        "c501312ca9eb1aaac6344edbe160e41d3d8d79570e6440f2a84f7d9abf462270",
        "https://testnet.bluefin.io",
        "https://dapi.api.sui-staging.bluefin.io",
    )
    .await;
    assert_ne!(auth_token, "");
}
