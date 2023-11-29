use ed25519_dalek::*;
use serde::Deserialize;
use serde_json::Value;
use crate::models::common::deserialize_to_f64_via_decimal;

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
    pub quantity: u128,
    pub open_qty: u128,
    pub avg_fill_price: u128
}

pub fn parse_order_update(v: Value) -> OrderUpdate {
    let quantity_str: String = serde_json::from_str(&v["quantity"].to_string()).unwrap();
    let open_qty_str: String = serde_json::from_str(&v["openQty"].to_string()).unwrap();
    let avg_fill_price_str: String = serde_json::from_str(&v["avgFillPrice"].to_string()).unwrap();
    return OrderUpdate {
        hash: serde_json::from_str(&v["hash"].to_string()).unwrap(),
        symbol: serde_json::from_str(&v["symbol"].to_string()).unwrap(),
        order_status: serde_json::from_str(&v["orderStatus"].to_string()).unwrap(),
        cancel_reason: serde_json::from_str(&v["cancelReason"].to_string()).unwrap(),
        quantity: quantity_str.parse::<u128>().unwrap(),
        open_qty: open_qty_str.parse::<u128>().unwrap(),
        avg_fill_price: avg_fill_price_str.parse::<u128>().unwrap(),
    };
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdateEventData {
    pub event_name: String,
    pub data: AccountUpdateData,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdateData {
    #[serde(rename = "accountData")]
    pub account_data: AccountData,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountData {
    pub address: String,
    pub can_trade: bool,
    pub update_time: u64,
    pub fee_tier: String,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub wallet_balance: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub total_position_qty_reduced: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub total_position_qty_reducible: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub total_position_margin: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub total_unrealized_profit: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub total_expected_pnl: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub free_collateral: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub account_value: f64,
    pub account_data_by_market: Vec<MarketData>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarketData {
    pub symbol: String,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub position_qty_reduced: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub position_qty_reducible: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub position_margin: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub unrealized_profit: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub expected_pnl: f64,
    #[serde(deserialize_with = "deserialize_to_f64_via_decimal")]
    pub selected_leverage: f64,
}