use serde::{Deserialize, Serialize};
use crate::models::common::OrderBook;
use crate::models::common::deserialize_as_mix_tuples;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceServer {
    pub endpoint: String,
    pub encrypt: bool,
    pub protocol: String,
    pub ping_interval: u64,
    pub ping_timeout: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub token: String,
    pub instance_servers: Vec<InstanceServer>,
}
#[derive(Deserialize)]
pub struct Response {
    pub code: String,
    pub data: Data,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Comm {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Level2Depth {
    pub topic: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub subject: String,
    pub sn: u64,
    pub data: Level2Data,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Level2Data {
    #[serde(deserialize_with = "deserialize_as_mix_tuples")]
    pub bids: Vec<(f64, f64)>,
    pub sequence: u64,
    pub timestamp: u64,
    pub ts: u64,
    #[serde(deserialize_with = "deserialize_as_mix_tuples")]
    pub asks: Vec<(f64, f64)>,
}

impl From<Level2Depth> for OrderBook {
    fn from(l2_depth: Level2Depth) -> Self {
        OrderBook {
            asks: l2_depth.data.asks,
            bids: l2_depth.data.bids,
        }
    }
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeOrderMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub topic: String,
    pub subject: String,
    pub channel_type: String,
    pub data: OrderData,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderData {
    pub order_id: String,
    pub symbol: String,
    pub type_: String,
    pub status: String,
    pub match_size: String,
    pub match_price: String,
    pub order_type: String,
    pub side: String,
    pub price: String,
    pub size: String,
    pub remain_size: String,
    pub filled_size: String,
    pub canceled_size: String,
    pub trade_id: String,
    pub client_oid: String,
    pub order_time: i64,
    pub old_size: Option<String>,
    pub liquidity: String,
    pub ts: i64,
}

// Define a struct for the "data" field in the JSON
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TickerData {
    pub symbol: String,
    pub sequence: u64,
    pub best_bid_size: u32,
    pub best_bid_price: String,
    pub best_ask_price: String,
    pub best_ask_size: u32,
    pub ts: u64,
}

// Define a struct for the top-level JSON object
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TickerV2 {
    pub topic: String,
    #[serde(rename = "type")]  // Explicitly rename this one since it's a reserved keyword
    pub message_type: String,
    pub subject: String,
    pub sn: u64,
    pub data: TickerData,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_trade_order_message_deserialization() {
        use crate::models::kucoin_models::TradeOrderMessage;
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
        assert_eq!(parsed_message.message_type, "message");
    }
}
