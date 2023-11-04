use serde::{Deserialize, Serialize};

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
    pub bids: Vec<Level2Entry>,
    pub sequence: u64,
    pub timestamp: u64,
    pub ts: u64,
    pub asks: Vec<Level2Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Level2Entry(#[serde(with = "string_or_float")] pub f64, pub u64);

pub mod string_or_float {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
        where
            D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<f64>().map_err(D::Error::custom)
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
