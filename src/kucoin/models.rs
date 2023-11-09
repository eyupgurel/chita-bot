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
