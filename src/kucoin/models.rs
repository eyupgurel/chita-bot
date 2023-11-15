use serde::Deserialize;
use serde::de::{self, Deserializer};

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

fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
    where
        D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => s.parse::<f64>().map(Some).map_err(de::Error::custom),
        None => Ok(None),
    }
}

fn deserialize_string_to_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>().map_err(de::Error::custom)
}

#[cfg(test)]
mod tests {
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
}
