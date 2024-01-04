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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub id: String,
    pub symbol: String,
    pub auto_deposit: bool,
    pub maint_margin_req: f64,
    pub risk_limit: i64,
    pub real_leverage: f64,
    pub cross_mode: bool,
    pub delev_percentage: f64,
    pub opening_timestamp: i64,
    pub current_timestamp: i64,
    pub current_qty: i64,
    pub current_cost: f64,
    pub current_comm: f64,
    pub unrealised_cost: f64,
    pub realised_gross_cost: f64,
    pub realised_cost: f64,
    pub is_open: bool,
    pub mark_price: f64,
    pub mark_value: f64,
    pub pos_cost: f64,
    pub pos_cross: f64,
    pub pos_cross_margin: f64,
    pub pos_init: f64,
    pub pos_comm: f64,
    pub pos_comm_common: f64,
    pub pos_loss: f64,
    pub pos_margin: f64,
    pub pos_maint: f64,
    pub maint_margin: f64,
    pub realised_gross_pnl: f64,
    pub realised_pnl: f64,
    pub unrealised_pnl: f64,
    pub unrealised_pnl_pcnt: f64,
    pub unrealised_roe_pcnt: f64,
    pub avg_entry_price: f64,
    pub liquidation_price: f64,
    pub bankrupt_price: f64,
    pub settle_currency: String,
    pub is_inverse: bool,
    pub maintain_margin: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PositionList {
    pub code: String,
    pub data: Vec<Position>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KucoinUserPosition {
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
    pub risk_limit: f64,
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
    pub change_reason: String, //changeReason:marginChange、positionChange、liquidation、autoAppendMarginStatusChange、adl
    pub current_cost: f64,
    pub opening_timestamp: u64,
    pub current_qty: i128,
    pub delev_percentage: f64,
    pub current_comm: f64,
    pub realised_gross_cost: f64,
    pub is_open: bool,
    pub pos_cross: f64,
    pub current_timestamp: u64,
    pub unrealised_roe_pcnt: f64,
    pub unrealised_pnl_pcnt: f64,
    pub settle_currency: String,
}