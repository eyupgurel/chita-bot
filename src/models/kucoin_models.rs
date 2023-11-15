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