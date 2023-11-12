use ed25519_dalek::*;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug)]
pub struct Auth {
    pub token: String,
}

#[derive(Deserialize, Debug)]
pub struct Error {
    pub code: u64,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct PostResponse {
    pub error: Option<Error>,
}

#[derive(Deserialize, Debug)]
pub struct UserPosition {
    pub symbol: String,
    pub side: bool,
    pub avg_entry_price: u128,
    pub quantity: u128,
    pub margin: u128,
    pub leverage: u128,
}

#[derive(Debug, Clone)]
pub struct Wallet {
    pub signing_key: SigningKey,
    pub public_key: String,
    pub address: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrderUpdate {
    pub hash: String,
    pub symbol: String,
    pub order_status: String,
    pub cancel_reason: String,
}

pub fn parse_user_position(position: Value) -> UserPosition {
    let ep_str: String = serde_json::from_str(&position["avgEntryPrice"].to_string()).unwrap();
    let quantity_str: String = serde_json::from_str(&position["quantity"].to_string()).unwrap();
    let margin_str: String = serde_json::from_str(&position["margin"].to_string()).unwrap();
    let leverage_str: String = serde_json::from_str(&position["leverage"].to_string()).unwrap();
    let side_str: String = serde_json::from_str(&position["side"].to_string()).unwrap();

    return UserPosition {
        symbol: serde_json::from_str(&position["symbol"].to_string()).unwrap(),
        side: if side_str == "SELL" { false } else { true },
        quantity: quantity_str.parse::<u128>().unwrap(),
        avg_entry_price: ep_str.parse::<u128>().unwrap(),
        margin: margin_str.parse::<u128>().unwrap(),
        leverage: leverage_str.parse::<u128>().unwrap(),
    };
}
