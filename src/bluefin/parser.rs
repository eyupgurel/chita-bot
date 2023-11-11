use serde_json::Value;

use super::client::client::UserPosition;

pub fn parse_user_position(position: Value) -> UserPosition {
    let ep_str: String = serde_json::from_str(&position["avgEntryPrice"].to_string()).unwrap();
    let quantity_str: String = serde_json::from_str(&position["quantity"].to_string()).unwrap();
    let margin_str: String = serde_json::from_str(&position["margin"].to_string()).unwrap();
    let leverage_str: String = serde_json::from_str(&position["leverage"].to_string()).unwrap();
    let side_str: String = serde_json::from_str(&position["side"].to_string()).unwrap();

    return UserPosition {
        symbol: serde_json::from_str(&position["symbol"].to_string()).unwrap(),
        side: if side_str == "SELL" { false } else { true },
        quantity: quantity_str.parse::<u128>().unwrap(),
        avg_entry_price: ep_str.parse::<u128>().unwrap(),
        margin: margin_str.parse::<u128>().unwrap(),
        leverage: leverage_str.parse::<u128>().unwrap(),
    };
}
