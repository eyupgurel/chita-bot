use serde::Deserialize;
use serde_derive::Serialize;
use crate::models::common::deserialize_optional_f64;
use crate::models::common::deserialize_string_to_f64;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstanceServer {
    pub endpoint: String,
    pub encrypt: bool,
    pub protocol: String,
    pub ping_interval: u64,
    pub ping_timeout: u64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub token: String,
    pub instance_servers: Vec<InstanceServer>,
}

#[derive(Deserialize, Debug)]
pub struct Response {
    pub code: String,
    pub data: Data,
}

#[derive(Deserialize, Debug)]
pub struct Error {
    pub code: String,
    pub msg: String,
}

#[derive(Deserialize, Debug)]
pub struct CallResponse {
    pub error: Option<Error>,
    pub order_id: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct UserPosition {
    pub symbol: String,
    // pub side: bool,
    #[serde(rename = "avgEntryPrice")]
    pub avg_entry_price: u128,

    #[serde(rename = "currentQty")]
    pub quantity: u128,
    // pub margin: u128,
    #[serde(rename = "realLeverage")]
    pub leverage: u128,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct TradeOrderMessage {
    #[serde(rename = "type")]
    pub type_field: String,
    pub topic: String,
    pub subject: String,
    pub channel_type: String,
    pub data: TradeOrderData,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct TradeOrderData {
    pub order_id: String,
    pub symbol: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub status: String,
    #[serde(deserialize_with = "deserialize_optional_f64")]
    pub match_size: Option<f64>,
    #[serde(deserialize_with = "deserialize_optional_f64")]
    pub match_price: Option<f64>,
    pub order_type: String,
    pub side: String,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub price: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub size: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub remain_size: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub filled_size: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub canceled_size: f64,
    pub trade_id: Option<String>,
    pub client_oid: String,
    pub order_time: u128,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub old_size: f64,
    pub liquidity: String,
    pub ts: u128,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PositionChangeMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub user_id: Option<String>, // Marked as deprecated, so it's optional
    pub channel_type: String,
    pub topic: String,
    pub subject: String,
    pub data: PositionChangeData,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PositionChangeData {
    pub realised_gross_pnl: f64,
    pub symbol: String,
    pub cross_mode: bool,
    pub liquidation_price: f64,
    pub pos_loss: f64,
    pub avg_entry_price: f64,
    pub unrealised_pnl: f64,
    pub mark_price: f64,
    pub pos_margin: f64,
    pub auto_deposit: bool,
    pub risk_limit: i64,
    pub unrealised_cost: f64,
    pub pos_comm: f64,
    pub pos_maint: f64,
    pub pos_cost: f64,
    pub maint_margin_req: f64,
    pub bankrupt_price: f64,
    pub realised_cost: f64,
    pub mark_value: f64,
    pub pos_init: f64,
    pub realised_pnl: f64,
    pub maint_margin: f64,
    pub real_leverage: f64,
    pub change_reason: String,
    pub current_cost: f64,
    pub opening_timestamp: i64,
    pub current_qty: i32,
    pub delev_percentage: f64,
    pub current_comm: f64,
    pub realised_gross_cost: f64,
    pub is_open: bool,
    pub pos_cross: f64,
    pub current_timestamp: i64,
    pub unrealised_roe_pcnt: f64,
    pub unrealised_pnl_pcnt: f64,
    pub settle_currency: String,
}

#[cfg(test)]
mod tests {
    use crate::kucoin::models::PositionChangeMessage;
    use crate::kucoin::TradeOrderMessage;

    #[test]
    fn test_trade_order_message_deserialization() {
        use std::fs;

        // Read the JSON string from the file
        let json_str = fs::read_to_string("./src/tests/seed/kucoin/trade-orders-per-market.json")
            .expect("Unable to read the file");

        // Deserialize the JSON string into TradeOrderMessage struct
        let parsed_message: TradeOrderMessage =
            serde_json::from_str(&json_str).expect("Failed to parse the JSON");

        // Print and assert or perform tests as necessary
        println!("{:?}", parsed_message);

        // Example assertion
        assert_eq!(parsed_message.type_field, "message");
    }

    #[test]
    fn test_position_change_deserialization() {
        use std::fs;

        // Read the JSON string from the file
        let json_str = fs::read_to_string("./src/tests/seed/kucoin/position-change.json")
            .expect("Unable to read the file");

        // Deserialize the JSON string into TradeOrderMessage struct
        let parsed_message: PositionChangeMessage =
            serde_json::from_str(&json_str).expect("Failed to parse the JSON");

        // Print and assert or perform tests as necessary
        println!("{:?}", parsed_message);

        // Example assertion
        assert_eq!(parsed_message.msg_type, "message");
    }
}
