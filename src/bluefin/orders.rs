use serde::{Deserialize, Serialize};
use sha256::digest;
use web3_unit_converter::Unit;

use crate::bluefin::utils::get_current_time;

#[derive(Debug, Clone)]
#[allow(non_snake_case)]
pub struct Order {
    pub market: String,
    pub price: u128,
    pub isBuy: bool,
    pub reduceOnly: bool,
    pub quantity: u128,
    pub postOnly: bool,
    pub orderbookOnly: bool,
    pub leverage: u128,
    pub expiration: u128,
    pub salt: u128,
    pub maker: String,
    pub ioc: bool,
    pub orderType: String,
    pub timeInForce: String,
    pub hash: String,
    pub serialized: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct OrderJSONRequest {
    pub orderbookOnly: bool,
    pub symbol: String,
    pub price: String,
    pub quantity: String,
    pub triggerPrice: String,
    pub leverage: String,
    pub userAddress: String,
    pub orderType: String,
    pub side: String,
    pub reduceOnly: bool,
    pub salt: u128,
    pub expiration: u128,
    pub orderSignature: String,
    pub timeInForce: String,
    pub postOnly: bool,
    pub cancelOnRevert: bool,
    pub clientId: String,
}

/**
 * Encodes order flags and returns a 16 bit hex
 */
fn get_order_flags(order: &Order) -> u32 {
    let mut flag = 0;

    if order.ioc {
        flag += 1;
    };
    if order.postOnly {
        flag += 2;
    }
    if order.reduceOnly {
        flag += 4;
    }
    if order.isBuy {
        flag += 8
    }
    if order.orderbookOnly {
        flag += 16
    }
    return flag;
}

/**
 * Returns hash of the order
 */
pub fn get_order_hash(order: Order) -> String {
    let serialized_msg = get_serialized_order(&order);
    let order_hash = digest(hex::decode(&serialized_msg).expect("Decoding failed"));
    return order_hash;
}

/**
 * Converts order into OrderJSONRequest
 */
pub fn to_order_request(order: Order, signature: String) -> OrderJSONRequest {
    return OrderJSONRequest {
        orderbookOnly: order.orderbookOnly,
        symbol: order.market.to_string(),
        price: order.price.to_string(),
        quantity: order.quantity.to_string(),
        triggerPrice: "0".to_string(),
        leverage: order.leverage.to_string(),
        userAddress: order.maker.to_string(),
        orderType: order.orderType.to_string(),
        side: if order.isBuy == true {
            "BUY".to_string()
        } else {
            "SELL".to_string()
        },
        reduceOnly: order.reduceOnly,
        salt: order.salt,
        expiration: order.expiration,
        orderSignature: signature,
        timeInForce: order.timeInForce,
        postOnly: order.postOnly,
        cancelOnRevert: false,
        clientId: "chita-bot".to_string(),
    };
}

/**
 * Given an order, returns hash of the order
 */
pub fn get_serialized_order(order: &Order) -> String {
    let flags = get_order_flags(&order);
    let flags_array = format!("{:0>2x}", flags);

    let order_price_hex = format!("{:0>32x}", order.price);
    let order_quantity_hex = format!("{:0>32x}", order.quantity);
    let order_leverage_hex = format!("{:0>32x}", order.leverage);
    let order_salt = format!("{:0>32x}", order.salt);
    let order_expiration = format!("{:0>16x}", order.expiration);
    let order_maker = &order.maker;
    let order_market = &order.market;
    let bluefin_string = hex::encode("Bluefin");

    let order_buffer = order_price_hex
        + &order_quantity_hex
        + &order_leverage_hex
        + &order_salt
        + &order_expiration
        + &order_maker[2..]
        + &order_market[2..]
        + &flags_array
        + &bluefin_string;

    return order_buffer;
}

// ----------------------------------------------------------------------------------- //
//                                   PUBLIC METHODS                                    //
// ----------------------------------------------------------------------------------- //

pub fn create_limit_ioc_order(
    wallet_address: String,
    market_name: String,
    market_id: String,
    is_buy: bool,
    reduce_only: bool,
    price: f64,
    quantity: f64,
    leverage: u128,
) -> Order {
    let mut order = Order {
        market: market_name,
        isBuy: is_buy,
        price: (Unit::Ether(&format!("{}", price)).to_wei_str().unwrap())
            .parse()
            .unwrap(),
        quantity: (Unit::Ether(&format!("{}", quantity)).to_wei_str().unwrap())
            .parse()
            .unwrap(),
        leverage: (Unit::Ether(&format!("{}", leverage)).to_wei_str().unwrap())
            .parse()
            .unwrap(),
        maker: wallet_address,
        reduceOnly: reduce_only,
        postOnly: false,
        orderbookOnly: true,
        expiration: 3655643731,
        salt: get_current_time(),
        ioc: true,
        orderType: "LIMIT".to_string(),
        timeInForce: "IOC".to_string(),
        hash: "".to_string(),
        serialized: "".to_string(),
    };

    // order hash
    let mut order_with_market_id = order.clone();
    order_with_market_id.market = market_id;

    order.hash = get_order_hash(order_with_market_id.clone());

    // serialize order
    order.serialized = get_serialized_order(&order_with_market_id);

    return order;
}
