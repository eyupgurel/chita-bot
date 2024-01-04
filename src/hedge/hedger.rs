use crate::bluefin::{parse_user_position, AccountData, BluefinClient, UserPosition, OrderUpdate, parse_order_update};
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{Credentials, KuCoinClient};
use crate::models::common::{CircuitBreakerConfig, Market, OrderBook};
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;
use crate::sockets::kucoin_ob_socket::stream_kucoin_socket;
use crate::models::kucoin_models::KucoinUserPosition;
use crate::kucoin::PositionChangeEvent;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use serde_json::Value;
use std::ops::Add;
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;
use std::thread;
use crate::circuit_breakers::circuit_breaker::CircuitBreakerBase;
use crate::circuit_breakers::cancel_all_orders_breaker::CancelAllOrdersCircuitBreaker;
use std::collections::HashMap;
use crate::circuit_breakers::circuit_breaker::State;
use crate::circuit_breakers::kucoin_breaker::KuCoinBreaker;
use crate::circuit_breakers::circuit_breaker::CircuitBreaker;

static BIGNUMBER_BASE: u128 = 1000000000000000000;

pub struct HGR {
    pub market: Market,
    pub cb_config: CircuitBreakerConfig,
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
    #[allow(dead_code)]
    bluefin_account: AccountData,
    bluefin_position: UserPosition,
    tx_hedger: Sender<f64>,
    rx_bluefin_ob: Receiver<OrderBook>,
    rx_bluefin_ob_diff: Receiver<OrderBook>
}

impl HGR {
    pub fn new(
        market: Market, 
        cb_config: CircuitBreakerConfig, 
        tx_hedger: Sender<f64>,
        rx_bluefin_ob: Receiver<OrderBook>,
        rx_bluefin_ob_diff: Receiver<OrderBook>,
    ) -> HGR {
        
        let vars: EnvVars = env::env_variables();

        let bluefin_client = BluefinClient::new(
            &vars.bluefin_wallet_key,
            &vars.bluefin_endpoint,
            &vars.bluefin_on_boarding_url,
            &vars.bluefin_websocket_url,
            vars.bluefin_leverage,
        );

        let kucoin_client = KuCoinClient::new(
            Credentials::new(
                &vars.kucoin_api_key,
                &vars.kucoin_api_secret,
                &vars.kucoin_api_phrase,
            ),
            &vars.kucoin_endpoint,
            &vars.kucoin_on_boarding_url,
            &vars.kucoin_websocket_url,
            vars.kucoin_leverage,
        );

        let bluefin_market = market.symbols.bluefin.to_owned();

        let bluefin_account = bluefin_client.get_user_account();
        let bluefin_position = bluefin_client.get_user_position(&bluefin_market);

        HGR {
            market,
            cb_config,
            bluefin_client,
            kucoin_client,
            bluefin_position,
            bluefin_account,
            tx_hedger,
            rx_bluefin_ob,
            rx_bluefin_ob_diff,

        }
    }
}

pub trait Hedger {
    fn connect(&mut self);
    fn hedge(&mut self, dry_run:bool, kucoin_position: KucoinUserPosition, ob: Option<&OrderBook>);
    fn calc_limit_order_price(hedge_qty: Decimal, is_buy: bool, ob: &OrderBook) -> f64;
}

impl Hedger for HGR {
    fn connect(&mut self) {
        let vars: EnvVars = env::env_variables();
        let (tx_bluefin_pos_update, _rx_bluefin_pos_update) = mpsc::channel();
        let (tx_bluefin_order_update, _rx_bluefin_order_update) = mpsc::channel();
        let (tx_kucoin_pos_change, rx_kucoin_pos_change) = mpsc::channel();

        let bluefin_market = self.market.symbols.bluefin.to_owned();
        let bluefin_market_for_order_fill = bluefin_market.clone();
        let bluefin_auth_token = self.bluefin_client.auth_token.clone();
        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_order_update = thread::spawn(move || {
            stream_bluefin_private_socket(
                &bluefin_websocket_url,
                &bluefin_market_for_order_fill,
                &bluefin_auth_token,
                "OrderUpdate",
                tx_bluefin_order_update, // Sender channel of the appropriate type
                |msg: &str| -> OrderUpdate {

                    let v: Value = serde_json::from_str(&msg).unwrap();
                    let order_update: OrderUpdate = parse_order_update(v["data"]["order"].clone());

                    let open_qty = Decimal::from_u128(order_update.open_qty).unwrap()
                        / Decimal::from(BIGNUMBER_BASE);

                    if open_qty.is_zero() && order_update.symbol == bluefin_market {
                        let quantity = Decimal::from_u128(order_update.quantity).unwrap()
                            / Decimal::from(BIGNUMBER_BASE);
                        let avg_fill_price = Decimal::from_u128(order_update.avg_fill_price).unwrap()
                            / Decimal::from(BIGNUMBER_BASE);
                        let volume = (quantity * avg_fill_price).to_f64().unwrap();
                        tracing::info!(
                            market = order_update.symbol,
                            bluefin_volume = volume,
                            "Bluefin Volume"
                        );
                    }
                    order_update
                },
            );
        });

        let bluefin_market = self.market.symbols.bluefin.to_owned();
        
        let bluefin_market_for_order_fill = bluefin_market.clone();
        let bluefin_auth_token = self.bluefin_client.auth_token.clone();
        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_pos_update = thread::spawn(move || {
            stream_bluefin_private_socket(
                &bluefin_websocket_url,
                &bluefin_market_for_order_fill,
                &bluefin_auth_token,
                "PositionUpdate",
                tx_bluefin_pos_update, // Sender channel of the appropriate type
                |msg: &str| -> UserPosition {
                    let v: Value = serde_json::from_str(&msg).unwrap();
                    let user_position: UserPosition = parse_user_position(v["data"]["position"].clone());
                    if user_position.symbol == bluefin_market {
                        let mut quantity = Decimal::from_u128(user_position.quantity).unwrap()
                            / Decimal::from(BIGNUMBER_BASE);

                        if !user_position.side{
                            quantity = quantity * Decimal::from_i128(-1).unwrap();
                        }

                        tracing::info!(
                        market = user_position.symbol,
                        bluefin_real_quantity = quantity.to_f64().unwrap(),
                        "Bluefin Quantity");
                    }
                    user_position
                },
            );
        });


        let kucoin_market_for_pos_update = self.market.symbols.kucoin.clone();
        let topic = "/contract/position";
        // let topic = format!("/contract/position:{}", self.market.symbols.bluefin);
        let kucoin_private_socket_url = self.kucoin_client.get_kucoin_private_socket_url().clone();

        let _handle_kucoin_pos_change = thread::spawn(move || {
            stream_kucoin_socket(
                &kucoin_private_socket_url,
                &kucoin_market_for_pos_update,
                &topic,
                tx_kucoin_pos_change, // Sender channel of the appropriate type
                |msg: &str| -> KucoinUserPosition {
                    let kucoin_user_pos: PositionChangeEvent =
                        serde_json::from_str(&msg).expect("Can't parse");

                        tracing::info!(kucoin_current_qty=kucoin_user_pos.data.current_qty,
                            "Kucoin User Position");
                        
                    kucoin_user_pos.data
                },
                &"position.change",
                true
            );
        });

        let dry_run = vars.dry_run;
        let bluefin = "bluefin".to_string();

        let mut ob_map: HashMap<String, OrderBook> = HashMap::new();

        let bluefin_market_for_ob_diff_update_breaker = self.market.symbols.bluefin.clone();
        let mut bluefin_ob_diff_breaker = CancelAllOrdersCircuitBreaker {
            circuit_breaker: CircuitBreakerBase {
                config: self.cb_config.clone(),
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new(),
                market: bluefin_market_for_ob_diff_update_breaker,
            }
        };

        let bluefin_market_for_ob_update_breaker = self.market.symbols.bluefin.clone();
        let mut bluefin_ob_breaker = CancelAllOrdersCircuitBreaker {
            circuit_breaker: CircuitBreakerBase {
                config: self.cb_config.clone(),
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new(),
                market: bluefin_market_for_ob_update_breaker,
            }
        };

        let bluefin_market_for_pos_update_breaker = self.market.symbols.bluefin.clone();
        let mut kucoin_pos_update_disconnect_breaker = CancelAllOrdersCircuitBreaker {
            circuit_breaker: CircuitBreakerBase {
                config: self.cb_config.clone(),
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new(),
                market: bluefin_market_for_pos_update_breaker,
            }
        };

        loop {
            match self.rx_bluefin_ob.try_recv() {
                Ok(value) => {
                    tracing::debug!("hedger bluefin ob: {:?}", value);
                    bluefin_ob_breaker.on_success();
                    ob_map.insert(bluefin.clone(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {

                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Bluefin Hedger OB worker has disconnected!");
                    if !bluefin_ob_breaker.is_open() {
                        bluefin_ob_breaker.on_failure();
                    }
                }
            }

            match self.rx_bluefin_ob_diff.try_recv() {
                Ok(value) => {
                    tracing::debug!("hedger bluefin ob diff: {:?}", value);
                    bluefin_ob_diff_breaker.on_success();
                    ob_map.insert(bluefin.clone(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {

                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Bluefin Hedger OB DIFF worker has disconnected!");
                    if !bluefin_ob_diff_breaker.is_open() {
                        bluefin_ob_diff_breaker.on_failure();
                    }
                }
            }

            match rx_kucoin_pos_change.try_recv() {
                Ok(value) => {
                    tracing::debug!("kucoin position update: {:?}", value.1);
                    kucoin_pos_update_disconnect_breaker.on_success();
                    self.hedge(dry_run, value.1, ob_map.get(&bluefin));
                }
                Err(mpsc::TryRecvError::Empty) => {
                    
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Kucoin position update worker has disconnected!");
                    if !kucoin_pos_update_disconnect_breaker.is_open() {
                        kucoin_pos_update_disconnect_breaker.on_failure();
                    }
                }
            }
        }
    }

    fn calc_limit_order_price(hedge_qty: Decimal, is_buy: bool, ob: &OrderBook) -> f64 {
        let ob_pairs = if is_buy { &ob.asks } else { &ob.bids };

        let mut cumulative_qty: Decimal = Decimal::new(0, hedge_qty.scale());
        
        let price = ob_pairs
            .iter()
            .find_map(|price_and_qty| {
                cumulative_qty = cumulative_qty.add(Decimal::from_f64(price_and_qty.1).unwrap());
                if cumulative_qty.ge(&hedge_qty) {
                    Some(price_and_qty.0)
                } else {
                    None
                }
            });

            if price.is_none() {
                //if we get to the end of the depth and no price match - get last price of max depth
                let max_depth_price = ob_pairs.last().unwrap().0;
                tracing::info!("Could not match hedge price in Bluefin OB DOM, hedging at max depth {}", max_depth_price);
                max_depth_price
            } else {
                tracing::info!("Hedging at {}", price.unwrap());
                price.unwrap()
            }
    }

    fn hedge(&mut self, dry_run:bool, kucoin_position: KucoinUserPosition, ob: Option<&OrderBook>) {
        tracing::info!("Attempting to Hedge.... ");

        let bluefin_market = self.market.symbols.bluefin.to_owned();

        // unwrap kucoin position and get quantity
        let kucoin_quantity = Decimal::from(kucoin_position.current_qty);

        let current_kucoin_qty = kucoin_quantity / Decimal::from(self.market.lot_size);

        self.tx_hedger.send(current_kucoin_qty.to_f64().unwrap()).expect("Could not send current_kucoin_qty through!");

        let target_quantity = current_kucoin_qty * Decimal::from(-1);

        let mut bluefin_quantity = Decimal::from_u128(self.bluefin_position.quantity).unwrap()
            / Decimal::from(BIGNUMBER_BASE);

        if !self.bluefin_position.side {
            bluefin_quantity = bluefin_quantity * Decimal::from(-1);
        }

        let diff = target_quantity - bluefin_quantity;

        let order_quantity = diff.abs();

        let is_buy = diff.is_sign_positive();

        if order_quantity > Decimal::from(0) {
            tracing::info!(
                market = bluefin_market,
                current_kucoin_qty = current_kucoin_qty.to_f64().unwrap(),
                bluefin_quantity = bluefin_quantity.to_f64().unwrap(),
                order_quantity = order_quantity.to_f64().unwrap(),
                is_buy = is_buy,
                "Positions Across"
            );

            tracing::info!(
                market = bluefin_market,
                avg_entry_price = kucoin_position.avg_entry_price,
                realised_pnl = kucoin_position.realised_pnl,
                unrealised_pnl = kucoin_position.unrealised_pnl,
                unrealised_pnl_pcnt = kucoin_position.unrealised_pnl_pcnt,
                unrealised_roe_pcnt = kucoin_position.unrealised_roe_pcnt,
                liquidation_price = kucoin_position.liquidation_price,
                "Kucoin position"
            );
        }

        if order_quantity >= Decimal::from_str(&self.market.min_size).unwrap() && !dry_run && ob.is_some() {
            {
                tracing::debug!("order quantity as decimal: {}", order_quantity);
                let order_quantity_f64 = order_quantity.to_f64().unwrap();
                tracing::debug!("order quantity as f64: {}", order_quantity_f64);

                let price = HGR::calc_limit_order_price(order_quantity, is_buy, ob.unwrap());
                tracing::debug!("order price as f64: {}", price);                

                let order = self.bluefin_client.create_limit_ioc_order(
                    &bluefin_market,
                    is_buy,
                    false,
                    price,
                    order_quantity_f64,
                    None,
                );

                tracing::info!("order {:#?}", order);
                let signature = self.bluefin_client.sign_order(order.clone());
                let status = self
                    .bluefin_client
                    .post_signed_order(order.clone(), signature);
                tracing::info!("status {:?}", status);

                // Optimistic approach to prevent oscillations. For now update local position as if the position if filled immediately.
                let bf_pos_sign: i128 = if self.bluefin_position.side { 1 } else { -1 };
                let bf_signed_pos: i128 = (self.bluefin_position.quantity as i128) * bf_pos_sign;
                let order_pos_sign: i128 = if order.isBuy { 1 } else { -1 };
                let order_signed_pos: i128 = (order.quantity as i128) * order_pos_sign;
                let new_pos = bf_signed_pos + order_signed_pos;

                self.bluefin_position.quantity = new_pos.abs() as u128;
                self.bluefin_position.side = if new_pos > 0 { true } else { false };
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::models::common::OrderBook;

    use super::Hedger;
    use super::HGR;
    use bigdecimal::FromPrimitive;
    use rust_decimal::Decimal;


    #[test]
    fn test_calc_limit_order_price_buy() {
        let asks = vec![(44988.0, 0.001), (44990.9, 2.222), (44997.600000000006, 4.444), (45001.5, 0.001), (45015.200000000004, 0.001)];
        let bids = vec![(44977.9, 0.1), (44976.3, 1.867), (44975.0, 1.334), (44972.100000000006, 2.223), (44969.0, 4.447)];

        let ob = OrderBook {
            asks,
            bids,
        };

        let hedge_qty = 3.000;
        let limit_order_price = HGR::calc_limit_order_price(Decimal::from_f64(hedge_qty).unwrap(), true, &ob);
        let expected_limit_order_price = 44997.600000000006;

        assert_eq!(expected_limit_order_price, limit_order_price);
    }

    #[test]
    fn test_calc_limit_order_price_sell() {
        let asks = vec![(44988.0, 0.001), (44990.9, 2.222), (44997.600000000006, 4.444), (45001.5, 0.001), (45015.200000000004, 0.001)];
        let bids = vec![(44977.9, 0.1), (44976.3, 1.867), (44975.0, 1.334), (44972.100000000006, 2.223), (44969.0, 4.447)];

        let ob = OrderBook {
            asks,
            bids,
        };

        let hedge_qty = 3.000;
        let limit_order_price = HGR::calc_limit_order_price(Decimal::from_f64(hedge_qty).unwrap(), false, &ob);
        let expected_limit_order_price = 44975.0;

        assert_eq!(expected_limit_order_price, limit_order_price);
    }

    #[test]
    fn test_calc_limit_order_price_no_match_buy() {
        let asks = vec![(44988.0, 0.001), (44990.9, 2.222), (44997.600000000006, 4.444), (45001.5, 0.001), (45015.200000000004, 0.001)];
        let bids = vec![(44977.9, 0.1), (44976.3, 1.867), (44975.0, 1.334), (44972.100000000006, 2.223), (44969.0, 4.447)];

        let ob = OrderBook {
            asks,
            bids,
        };

        let hedge_qty = 100.00;
        let limit_order_price = HGR::calc_limit_order_price(Decimal::from_f64(hedge_qty).unwrap(), true, &ob);
        let expected_limit_order_price = 45015.200000000004;

        assert_eq!(expected_limit_order_price, limit_order_price);
    }

    #[test]
    fn test_calc_limit_order_price_no_match_sell() {
        let asks = vec![(44988.0, 0.001), (44990.9, 2.222), (44997.600000000006, 4.444), (45001.5, 0.001), (45015.200000000004, 0.001)];
        let bids = vec![(44977.9, 0.1), (44976.3, 1.867), (44975.0, 1.334), (44972.100000000006, 2.223), (44969.0, 4.447)];

        let ob = OrderBook {
            asks,
            bids,
        };

        let hedge_qty = 100.00;
        let limit_order_price = HGR::calc_limit_order_price(Decimal::from_f64(hedge_qty).unwrap(), false, &ob);
        let expected_limit_order_price = 44969.0;

        assert_eq!(expected_limit_order_price, limit_order_price);
    }
}