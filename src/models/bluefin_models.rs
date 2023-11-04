use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BluefinOrderBook {
    symbol: String,
    orderbook_update_id: u64,
    depth: u8,
    asks: Vec<[String; 2]>,
    bids: Vec<[String; 2]>,
}