use serde::{Deserialize, Serialize};
use crate::models::common::OrderBook;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BluefinOrderBook {
    symbol: String,
    orderbook_update_id: u64,
    depth: u8,
    asks: Vec<(String, String)>,
    bids: Vec<(String, String)>,
}

impl From<BluefinOrderBook> for OrderBook {
    fn from(bluefin_ob: BluefinOrderBook) -> Self {
        OrderBook {
            asks: bluefin_ob.asks,
            bids: bluefin_ob.bids,
        }
    }
}