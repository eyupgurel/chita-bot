use crate::bluefin::models::{parse_order_settlement_cancellation, OrderSettlementCancellation};
use crate::bluefin::{
    parse_order_settlement_update, parse_order_update, parse_user_position, AccountData,
    BluefinClient, OrderSettlementUpdate, OrderUpdate, UserPosition,
};
use crate::circuit_breakers::cancel_all_orders_breaker::CancelAllOrdersCircuitBreaker;
use crate::circuit_breakers::circuit_breaker::CircuitBreaker;
use crate::circuit_breakers::circuit_breaker::CircuitBreakerBase;
use crate::circuit_breakers::circuit_breaker::State;
use crate::circuit_breakers::kucoin_breaker::KuCoinBreaker;
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::PositionChangeEvent;
use crate::kucoin::{Credentials, KuCoinClient};
use crate::models::common::{CircuitBreakerConfig, Market, OrderBook};
use crate::models::kucoin_models::KucoinUserPosition;
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;
use crate::sockets::kucoin_socket::stream_kucoin_socket;
use bigdecimal::Signed;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;
use std::ops::{Add, Div, Mul};
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;
use std::time::Instant;

static BIGNUMBER_BASE: u128 = 1000000000000000000;

pub struct HGR {
    pub market: Market,
    pub cb_config: CircuitBreakerConfig,
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
    #[allow(dead_code)]
    bluefin_account: AccountData,
    bluefin_position: UserPosition,
    kucoin_position: KucoinUserPosition,
    tx_hedger: Sender<f64>,
    rx_bluefin_ob: Receiver<OrderBook>,
}

impl HGR {
    pub fn new(
        market: Market,
        cb_config: CircuitBreakerConfig,
        tx_hedger: Sender<f64>,
        rx_bluefin_ob: Receiver<OrderBook>,
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

        let bluefin_account = bluefin_client.get_user_account();

        let bluefin_market = market.symbols.bluefin.to_owned();
        let bluefin_position = bluefin_client.get_user_position(&bluefin_market);
        let kucoin_position = kucoin_client
            .get_position(&bluefin_market)
            .expect("Could not fetch Kucoin Position on Hedger startup");

        tracing::info!(
            bluefin_position_qty = bluefin_position.quantity as f64 / BIGNUMBER_BASE as f64,
            bluefin_position_symbol = bluefin_position.symbol,
            "Bluefin Initial Hedger Position"
        );

        tracing::info!(
            kucoin_position_qty = kucoin_position.current_qty as f64/ 100.0,
            kucoin_position_symbol = kucoin_position.symbol,
            "Kucoin Initial Hedger Position"
        );

        HGR {
            market,
            cb_config,
            bluefin_client,
            kucoin_client,
            bluefin_position,
            kucoin_position,
            bluefin_account,
            tx_hedger,
            rx_bluefin_ob,
        }
    }
}

pub trait Hedger {
    fn connect(&mut self);
    fn hedge(&mut self, dry_run: bool, ob: Option<&OrderBook>, is_periodic: bool);
    fn calc_limit_order_price(hedge_qty: Decimal, is_buy: bool, ob: &OrderBook) -> f64;
    fn calc_net_pos_qty(&mut self) -> (String, Decimal, bool);
    fn update_positions(&mut self);
}

impl Hedger for HGR {
    fn connect(&mut self) {
        let vars: EnvVars = env::env_variables();
        let (tx_bluefin_pos_update, rx_bluefin_pos_update) = mpsc::channel();
        let (tx_bluefin_order_update, _rx_bluefin_order_update) = mpsc::channel();
        let (tx_kucoin_pos_change, rx_kucoin_pos_change) = mpsc::channel();
        let (tx_bluefin_order_settlement_update, rx_bluefin_order_settlement_update) =
            mpsc::channel();
        let (tx_bluefin_order_settlement_cancellation, rx_bluefin_order_settlement_cancellation) =
            mpsc::channel();


        let market = self.market.clone();
        
        let bluefin_market = self.market.symbols.bluefin.to_owned();

        let bluefin_market_for_settlement_update = bluefin_market.clone();
        let bluefin_auth_token = self.bluefin_client.auth_token.clone();
        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_order_settlement_update = thread::spawn(move || {
            stream_bluefin_private_socket(
                &bluefin_websocket_url,
                &bluefin_market_for_settlement_update,
                &bluefin_auth_token,
                "OrderSettlementUpdate",
                tx_bluefin_order_settlement_update, // Sender channel of the appropriate type
                |msg: &str| -> OrderSettlementUpdate {
                    tracing::info!("Bluefin Order Settlement Update {}", msg);
                    let v: Value = serde_json::from_str(msg).unwrap();
                    let order_settlement: OrderSettlementUpdate =
                        parse_order_settlement_update(v["data"].clone());

                    tracing::info!(
                        market = market.name,
                        quantity_sent_for_settlement =
                            order_settlement.quantity_sent_for_settlement,
                        is_buy = order_settlement.is_buy,
                        "Bluefin Order Settlement Update"
                    );

                    order_settlement
                },
            );
        });

        let bluefin_market_for_settlement_cancellation = bluefin_market.clone();
        let bluefin_auth_token = self.bluefin_client.auth_token.clone();
        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_order_settlement_cancellation = thread::spawn(move || {
            stream_bluefin_private_socket(
                &bluefin_websocket_url,
                &bluefin_market_for_settlement_cancellation,
                &bluefin_auth_token,
                "OrderCancelledOnReversionUpdate",
                tx_bluefin_order_settlement_cancellation, // Sender channel of the appropriate type
                |msg: &str| -> OrderSettlementCancellation {
                    tracing::info!("Bluefin Order Settlement Cancellation {}", msg);
                    let v: Value = serde_json::from_str(msg).unwrap();
                    let order_settlement_cancellation: OrderSettlementCancellation =
                        parse_order_settlement_cancellation(v["data"].clone());

                    tracing::info!(
                        market = market.symbols.bluefin,
                        quantity_sent_for_cancellation =
                            order_settlement_cancellation.quantity_sent_for_cancellation,
                        is_buy = order_settlement_cancellation.is_buy,
                        "Bluefin Order Settlement Cancellation"
                    );

                    order_settlement_cancellation
                },
            );
        });

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
                    tracing::info!("Bluefin Order Update {}", &msg);

                    let v: Value = serde_json::from_str(&msg).unwrap();
                    let order_update: OrderUpdate = parse_order_update(v["data"]["order"].clone());

                    if !order_update.order_status.eq("CANCELLED") {
                        let open_qty = Decimal::from_u128(order_update.open_qty).unwrap()
                            / Decimal::from(BIGNUMBER_BASE);

                        if order_update.symbol == bluefin_market {
                            let quantity = Decimal::from_u128(order_update.quantity).unwrap()
                                / Decimal::from(BIGNUMBER_BASE);
                            let avg_fill_price = Decimal::from_u128(order_update.avg_fill_price)
                                .unwrap()
                                / Decimal::from(BIGNUMBER_BASE);
                            let volume = (quantity * avg_fill_price).to_f64().unwrap();
                            tracing::info!(
                                market = order_update.symbol,
                                bluefin_volume = volume,
                                bluefin_order_status = order_update.order_status,
                                "Bluefin Volume"
                            );
                        }
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
                    tracing::info!("Bluefin Position Update {}", &msg);

                    let v: Value = serde_json::from_str(&msg).unwrap();
                    let user_position: UserPosition =
                        parse_user_position(v["data"]["position"].clone());
                    if user_position.symbol == bluefin_market {
                        let mut quantity = Decimal::from_u128(user_position.quantity).unwrap()
                            / Decimal::from(BIGNUMBER_BASE);

                        if !user_position.side {
                            quantity = quantity * Decimal::from_i128(-1).unwrap();
                        }

                        let unrealized_pnl = Decimal::from_i128(user_position.unrealized_profit)
                            .unwrap()
                            .div(Decimal::from_u128(BIGNUMBER_BASE).unwrap());

                        tracing::info!(
                            market = user_position.symbol,
                            bluefin_real_quantity = quantity.to_f64().unwrap(),
                            bluefin_unrealized_pnl = unrealized_pnl.to_f64().unwrap(),
                            "Bluefin Position Update"
                        );
                    }
                    user_position
                },
            );
        });

        let kucoin_market_for_pos_update = self.market.symbols.kucoin.clone();
        let topic = "/contract/position";
        let kucoin_private_socket_url = self.kucoin_client.get_kucoin_private_socket_url().clone();
        let kucoin_lot_size = self.market.lot_size.clone();
        let _handle_kucoin_pos_change = thread::spawn(move || {
            stream_kucoin_socket(
                &kucoin_private_socket_url,
                &kucoin_market_for_pos_update,
                &topic,
                tx_kucoin_pos_change, // Sender channel of the appropriate type
                |msg: &str| -> KucoinUserPosition {
                    let kucoin_user_pos: PositionChangeEvent =
                        serde_json::from_str(&msg).expect("Can't parse");

                    let quantity = Decimal::from_i128(kucoin_user_pos.data.current_qty).unwrap()
                        / Decimal::from(kucoin_lot_size);

                    let avg_entry_price =
                        Decimal::from_f64(kucoin_user_pos.data.avg_entry_price).unwrap();

                    let volume = quantity.mul(avg_entry_price).abs().to_f64().unwrap();

                    tracing::info!(
                        market = kucoin_user_pos.data.symbol,
                        kucoin_real_quantity = quantity.to_f64().unwrap(),
                        kucoin_avg_entry_price = avg_entry_price.to_f64().unwrap(),
                        kucoin_volume = volume,
                        kucoin_unrealized_pnl = kucoin_user_pos.data.unrealised_pnl,
                        kucoin_realized_pnl = kucoin_user_pos.data.realised_pnl,
                        kucoin_gross_realized_pnl = kucoin_user_pos.data.realised_gross_pnl,
                        kucoin_position_update_curr_timestamp = kucoin_user_pos.data.current_timestamp,
                        "Kucoin Position Update"
                    );

                    kucoin_user_pos.data
                },
                &"position.change",
                true,
            );
        });

        let dry_run = vars.dry_run;
        let bluefin = "bluefin".to_string();

        let mut ob_map: HashMap<String, OrderBook> = HashMap::new();

        let bluefin_market_for_ob_update_breaker = self.market.symbols.bluefin.clone();
        let mut bluefin_ob_breaker = CancelAllOrdersCircuitBreaker {
            name: "Bluefin Orderbook breaker".to_string(),
            circuit_breaker: CircuitBreakerBase {
                config: self.cb_config.clone(),
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new(
                    "Kucoin Breaker for Bluefin Orderbook breaker".to_string(),
                ),
                market: bluefin_market_for_ob_update_breaker,
            },
        };

        let bluefin_market_for_pos_update_breaker = self.market.symbols.bluefin.clone();
        let mut kucoin_pos_update_disconnect_breaker = CancelAllOrdersCircuitBreaker {
            name: "Kucoin Position Update Disconnect breaker".to_string(),
            circuit_breaker: CircuitBreakerBase {
                config: self.cb_config.clone(),
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new(
                    "Kucoin Breaker for Kucoin Position Update Disconnect breaker".to_string(),
                ),
                market: bluefin_market_for_pos_update_breaker,
            },
        };

        let bluefin_market_for_bluefin_pos_update_breaker = self.market.symbols.bluefin.clone();
        let mut bluefin_pos_update_disconnect_breaker = CancelAllOrdersCircuitBreaker {
            name: "Bluefin Position Update Disconnect breaker".to_string(),
            circuit_breaker: CircuitBreakerBase {
                config: self.cb_config.clone(),
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new(
                    "Kucoin Breaker for Bluefin Position Update Disconnect breaker".to_string(),
                ),
                market: bluefin_market_for_bluefin_pos_update_breaker,
            },
        };

        let mut last_hedge_time = Instant::now();

        let periodic_hedging_enabled = vars.periodic_hedging_enabled;
        let periodic_hedging_period = vars.periodic_hedging_period;

        loop {
            match rx_bluefin_order_settlement_update.try_recv() {
                Ok(value) => {
                    tracing::info!("Bluefin Order Settlement update: {:?}", value);
                    let curr_side: i128 = if self.bluefin_position.side { 1 } else { -1 };
                    let new_qty = (self.bluefin_position.quantity as i128 * curr_side)
                        + ((value.quantity_sent_for_settlement as i128)
                            * (if value.is_buy { 1 } else { -1 }));

                    tracing::info!("Old Bluefin Position {:?}", self.bluefin_position);

                    self.bluefin_position.quantity = new_qty.abs() as u128;
                    self.bluefin_position.side = if new_qty > 0 { true } else { false };

                    tracing::info!("New Bluefin Position {:?}", self.bluefin_position);

                    let (_bluefin_market, mut order_quantity, is_buy) = self.calc_net_pos_qty();
                    let diff = if is_buy {
                        order_quantity.to_f64().unwrap()
                    } else {
                        order_quantity.set_sign_negative(true);
                        order_quantity.to_f64().unwrap()
                    };

                    self.tx_hedger
                        .send(diff.to_f64().unwrap())
                        .expect("Could not send current net position from hedger to mm!");
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::info!("Bluefin Order Settlement update worker has disconnected!");
                }
            }

            match rx_bluefin_order_settlement_cancellation.try_recv() {
                Ok(value) => {
                    tracing::info!("Bluefin Order Settlement Cancellation: {:?}", value);
                    let curr_side: i128 = if self.bluefin_position.side { 1 } else { -1 };
                    let new_qty = (self.bluefin_position.quantity as i128 * curr_side)
                        - ((value.quantity_sent_for_cancellation as i128)
                            * (if value.is_buy { 1 } else { -1 }));

                    tracing::info!("Old Bluefin Position {:?}", self.bluefin_position);

                    self.bluefin_position.quantity = new_qty.abs() as u128;
                    self.bluefin_position.side = if new_qty > 0 { true } else { false };

                    tracing::info!("New Bluefin Position {:?}", self.bluefin_position);

                    let (_bluefin_market, mut order_quantity, is_buy) = self.calc_net_pos_qty();
                    let diff = if is_buy {
                        order_quantity.to_f64().unwrap()
                    } else {
                        order_quantity.set_sign_negative(true);
                        order_quantity.to_f64().unwrap()
                    };

                    self.tx_hedger
                        .send(diff.to_f64().unwrap())
                        .expect("Could not send current net position from hedger to mm!");
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::info!(
                        "Bluefin Order Settlement Cancellation worker has disconnected!"
                    );
                }
            }

            match rx_bluefin_pos_update.try_recv() {
                Ok(value) => {
                    tracing::info!("Bluefin position update: {:?}.", value);
                    bluefin_pos_update_disconnect_breaker.on_success();
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::info!("Bluefin position update worker has disconnected!");
                    if !bluefin_pos_update_disconnect_breaker.is_open() {
                        bluefin_pos_update_disconnect_breaker.on_failure();
                    }
                }
            }

            match self.rx_bluefin_ob.try_recv() {
                Ok(value) => {
                    tracing::debug!("hedger bluefin ob: {:?}", value);
                    bluefin_ob_breaker.on_success();
                    ob_map.insert(bluefin.clone(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::info!("Bluefin Hedger OB worker has disconnected!");
                    if !bluefin_ob_breaker.is_open() {
                        bluefin_ob_breaker.on_failure();
                    }
                }
            }

            match rx_kucoin_pos_change.try_recv() {
                Ok(value) => {
                    tracing::info!("Kucoin position update: {:?}", value.1);
                    kucoin_pos_update_disconnect_breaker.on_success();
                    self.kucoin_position = value.1;

                    tracing::info!(periodic_hedge = false, "Kucoin Position Hedger");
                    self.hedge(dry_run, ob_map.get(&bluefin), false);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::info!("Kucoin position update worker has disconnected!");
                    if !kucoin_pos_update_disconnect_breaker.is_open() {
                        kucoin_pos_update_disconnect_breaker.on_failure();
                    }
                }
            }

            //hedge every second regardless of socket logic
            if periodic_hedging_enabled
                && (last_hedge_time.elapsed() >= Duration::from_secs_f64(periodic_hedging_period)
                    && ob_map.contains_key(&bluefin))
            {
                tracing::debug!(periodic_hedge = true, "Periodic Hedger");
                self.hedge(dry_run, ob_map.get(&bluefin), true);
                last_hedge_time = Instant::now();
            }
        }
    }

    fn update_positions(&mut self) {
        let bluefin_market_for_position = self.market.symbols.bluefin.to_owned().clone();
        self.kucoin_position = self
            .kucoin_client
            .get_position(&bluefin_market_for_position)
            .expect("Could not fetch Kucoin Position on Period Hedge");
        self.bluefin_position = self
            .bluefin_client
            .get_user_position(&bluefin_market_for_position);
    }

    fn calc_net_pos_qty(&mut self) -> (String, Decimal, bool) {

        tracing::info!("Calculating net position quantity for market bluefin: {}, kucoin: {}...", 
            self.bluefin_position.symbol, self.kucoin_position.symbol);

        let bluefin_market = self.market.symbols.bluefin.to_owned();

        // unwrap kucoin position and get quantity
        let kucoin_quantity = Decimal::from(self.kucoin_position.current_qty);  

        let current_kucoin_qty = kucoin_quantity / Decimal::from(self.market.lot_size);

        let mut bluefin_quantity = Decimal::from_u128(self.bluefin_position.quantity).unwrap()
            / Decimal::from(BIGNUMBER_BASE);

        if !self.bluefin_position.side {
            bluefin_quantity = bluefin_quantity * Decimal::from(-1);
        }

        tracing::info!("Target Quantity before check: {}, kucoin quantity: {}, bluefin quantity: {}", 
        current_kucoin_qty.to_f64().unwrap(), current_kucoin_qty.to_f64().unwrap(), bluefin_quantity.to_f64().unwrap());
        
        // let target_quantity = current_kucoin_qty * Decimal::from(-1);

        let target_quantity = 
        if (current_kucoin_qty.is_sign_positive() && bluefin_quantity.is_sign_negative()) || 
            (current_kucoin_qty.is_sign_negative() && bluefin_quantity.is_sign_positive()) || 
            (bluefin_quantity.is_zero()) {
                current_kucoin_qty * Decimal::from(-1)
            } else if current_kucoin_qty.is_sign_positive() && bluefin_quantity.is_sign_positive() || 
                current_kucoin_qty.is_sign_negative() && bluefin_quantity.is_sign_negative() {
                    bluefin_quantity = bluefin_quantity * Decimal::from(-1);
                    current_kucoin_qty * Decimal::from(-1)
            } else {
                current_kucoin_qty
            };

        tracing::info!("Target Quantity after check: {}", target_quantity.to_f64().unwrap());

        let diff = target_quantity - bluefin_quantity;

        let order_quantity = diff.abs();

        let is_buy = diff.is_sign_positive();

        // if order_quantity > Decimal::from(0) {
        tracing::info!(
            market = bluefin_market,
            current_kucoin_qty = current_kucoin_qty.to_f64().unwrap(),
            bluefin_quantity = bluefin_quantity.to_f64().unwrap(),
            order_quantity = order_quantity.to_f64().unwrap(),
            is_buy = is_buy,
            diff = diff.to_f64().unwrap(),
            "Positions Across"
        );

        tracing::info!(
            market = bluefin_market,
            avg_entry_price = self.kucoin_position.avg_entry_price,
            realised_pnl = self.kucoin_position.realised_pnl,
            unrealised_pnl = self.kucoin_position.unrealised_pnl,
            unrealised_pnl_pcnt = self.kucoin_position.unrealised_pnl_pcnt,
            unrealised_roe_pcnt = self.kucoin_position.unrealised_roe_pcnt,
            liquidation_price = self.kucoin_position.liquidation_price,
            "Kucoin position"
        );
        // }

        return (bluefin_market, order_quantity, is_buy);
    }

    fn calc_limit_order_price(hedge_qty: Decimal, is_buy: bool, ob: &OrderBook) -> f64 {
        let ob_pairs = if is_buy { &ob.asks } else { &ob.bids };

        let mut cumulative_qty: Decimal = Decimal::new(0, hedge_qty.scale());

        let price = ob_pairs.iter().find_map(|price_and_qty| {
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
            tracing::info!(
                "Could not match hedge price in Bluefin OB DOM, hedging at max depth {}",
                max_depth_price
            );
            max_depth_price
        } else {
            tracing::info!("Hedging at {}", price.unwrap());
            price.unwrap()
        }
    }

    fn hedge(&mut self, dry_run: bool, ob: Option<&OrderBook>, is_periodic: bool) {

        // let bluefin_market = self.market.symbols.bluefin.to_owned();
        // let bluefin_pos_before_hedging = self.bluefin_client.get_user_position(&bluefin_market);
        // tracing::info!(
        //     bluefin_position_before_hedging = bluefin_pos_before_hedging.quantity as f64 / BIGNUMBER_BASE as f64,
        //     "Bluefin Position Before Hedging"
        // );


        let (bluefin_market, mut order_quantity, is_buy) = self.calc_net_pos_qty();

        if order_quantity >= Decimal::from_str(&self.market.min_size).unwrap()
            && !dry_run
            && ob.is_some()
        {
            {
                let scale_factor = if self.market.name.eq("btc") {
                    10.0
                } else {
                    //eth
                    100.0
                };

                let order_quantity_f64 = order_quantity.to_f64().unwrap();

                tracing::debug!("Hedge Order Quantity {:?}", &order_quantity_f64);

                let mut price = HGR::calc_limit_order_price(order_quantity, is_buy, ob.unwrap());


                price = f64::trunc(price * scale_factor) / scale_factor;

                tracing::info!(
                    hedger_order_price = price,
                    hedger_order_quantity = order_quantity_f64,
                    "Hedger Limit Order"
                );

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

                if status.error.is_some() {
                    tracing::error!(
                        "Error posting Hedge Position on Bluefin. {:?}",
                        status.error.unwrap()
                    );
                } else {
                    tracing::info!("Placed Hedge limit order on Bluefin");

                    let diff = if is_buy {
                        order_quantity.to_f64().unwrap()
                    } else {
                        order_quantity.set_sign_negative(true);
                        order_quantity.to_f64().unwrap()
                    };

                    self.tx_hedger
                        .send(diff.to_f64().unwrap())
                        .expect("Could not send current net position from hedger to mm!");
                }

                // //Optimistic approach to prevent oscillations. For now update local position as if the position if filled immediately.
                // let bf_pos_sign: i128 = if self.bluefin_position.side { 1 } else { -1 };
                // let bf_signed_pos: i128 = (self.bluefin_position.quantity as i128) * bf_pos_sign;
                // let order_pos_sign: i128 = if order.isBuy { 1 } else { -1 };
                // let order_signed_pos: i128 = (order.quantity as i128) * order_pos_sign;
                // let new_pos = bf_signed_pos + order_signed_pos;

                // self.bluefin_position.quantity = new_pos.abs() as u128;
                // self.bluefin_position.side = if new_pos > 0 { true } else { false };
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
        let asks = vec![
            (44988.0, 0.001),
            (44990.9, 2.222),
            (44997.600000000006, 4.444),
            (45001.5, 0.001),
            (45015.200000000004, 0.001),
        ];
        let bids = vec![
            (44977.9, 0.1),
            (44976.3, 1.867),
            (44975.0, 1.334),
            (44972.100000000006, 2.223),
            (44969.0, 4.447),
        ];

        let ob = OrderBook { asks, bids };

        let hedge_qty = 3.000;
        let limit_order_price =
            HGR::calc_limit_order_price(Decimal::from_f64(hedge_qty).unwrap(), true, &ob);
        let expected_limit_order_price = 44997.600000000006;

        assert_eq!(expected_limit_order_price, limit_order_price);
    }

    #[test]
    fn test_calc_limit_order_price_sell() {
        let asks = vec![
            (44988.0, 0.001),
            (44990.9, 2.222),
            (44997.600000000006, 4.444),
            (45001.5, 0.001),
            (45015.200000000004, 0.001),
        ];
        let bids = vec![
            (44977.9, 0.1),
            (44976.3, 1.867),
            (44975.0, 1.334),
            (44972.100000000006, 2.223),
            (44969.0, 4.447),
        ];

        let ob = OrderBook { asks, bids };

        let hedge_qty = 3.000;
        let limit_order_price =
            HGR::calc_limit_order_price(Decimal::from_f64(hedge_qty).unwrap(), false, &ob);
        let expected_limit_order_price = 44975.0;

        assert_eq!(expected_limit_order_price, limit_order_price);
    }

    #[test]
    fn test_calc_limit_order_price_no_match_buy() {
        let asks = vec![
            (44988.0, 0.001),
            (44990.9, 2.222),
            (44997.600000000006, 4.444),
            (45001.5, 0.001),
            (45015.200000000004, 0.001),
        ];
        let bids = vec![
            (44977.9, 0.1),
            (44976.3, 1.867),
            (44975.0, 1.334),
            (44972.100000000006, 2.223),
            (44969.0, 4.447),
        ];

        let ob = OrderBook { asks, bids };

        let hedge_qty = 100.00;
        let limit_order_price =
            HGR::calc_limit_order_price(Decimal::from_f64(hedge_qty).unwrap(), true, &ob);
        let expected_limit_order_price = 45015.200000000004;

        assert_eq!(expected_limit_order_price, limit_order_price);
    }

    #[test]
    fn test_calc_limit_order_price_no_match_sell() {
        let asks = vec![
            (44988.0, 0.001),
            (44990.9, 2.222),
            (44997.600000000006, 4.444),
            (45001.5, 0.001),
            (45015.200000000004, 0.001),
        ];
        let bids = vec![
            (44977.9, 0.1),
            (44976.3, 1.867),
            (44975.0, 1.334),
            (44972.100000000006, 2.223),
            (44969.0, 4.447),
        ];

        let ob = OrderBook { asks, bids };

        let hedge_qty = 100.00;
        let limit_order_price =
            HGR::calc_limit_order_price(Decimal::from_f64(hedge_qty).unwrap(), false, &ob);
        let expected_limit_order_price = 44969.0;

        assert_eq!(expected_limit_order_price, limit_order_price);
    }
}
