use crate::models::kucoin_models::Response;
use reqwest;
use crate::constants::{KUCOIN_FUTURES_BASE_WSS_URL, KUCOIN_FUTURES_TOKEN_REQUEST_URL};

pub fn get_kucoin_url() -> String {
    let client = reqwest::blocking::Client::new();

    let j: Result<Response, Box<dyn std::error::Error>> = client
        .post(KUCOIN_FUTURES_TOKEN_REQUEST_URL)
        .send()
        .map_err(|e| format!("Error making the request: {}", e).into())
        .and_then(|res| {
            res.text()
                .map_err(|e| format!("Error reading the response body: {}", e).into())
        })
        .and_then(|body| serde_json::from_str(&body).map_err(Into::into));

    let _token = j
        .map(|response| response.data.token)
        .map_err(|e| {
            println!("Error: {}", e);
            e
        })
        .unwrap();

    let _kucoin_futures_wss_url = format!("{}?token={}", KUCOIN_FUTURES_BASE_WSS_URL, _token);

    return _kucoin_futures_wss_url;
}