use serde::Deserialize;

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
