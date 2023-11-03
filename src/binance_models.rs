use serde::{Serialize, Deserialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct DepthUpdate {
    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "U")]
    pub u_id: i64, // First update ID in event

    #[serde(rename = "u")]
    pub u2_id: i64,  // Final update ID in event

    #[serde(rename = "pu")]
    pub pu_id: i64,  // Final update Id in last stream(ie `u` in last stream)

    #[serde(rename = "b")]
    pub bid_orders: Vec<(String, String)>,

    #[serde(rename = "a")]
    pub ask_orders: Vec<(String, String)>,
}