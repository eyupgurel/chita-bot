use serde::{Deserialize, Serialize};
use crate::models::common::OrderBook;
use crate::models::common::deserialize_as_string_tuples;
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrderbookDepthUpdate {
    pub event_name: String,
    pub data: OrderbookData,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrderbookData {
    pub symbol: String,
    #[serde(deserialize_with = "deserialize_as_string_tuples")]
    pub bids: Vec<(f64, f64)>,
    #[serde(deserialize_with = "deserialize_as_string_tuples")]
    pub asks: Vec<(f64, f64)>,
    pub depth: u32,
    pub orderbook_update_id: u64,
}

impl From<OrderbookDepthUpdate> for OrderBook {
    fn from(d_update: OrderbookDepthUpdate) -> Self {
        OrderBook {
            asks: d_update.data.asks,
            bids: d_update.data.bids,
        }
    }
}