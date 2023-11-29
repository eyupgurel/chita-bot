use crate::bluefin::{parse_user_position, AccountData, AccountUpdateEventData, BluefinClient, UserPosition, OrderUpdate, parse_order_update};
use crate::circuit_breakers::cancel_all_orders_breaker::CancelAllOrdersCircuitBreaker;
use crate::circuit_breakers::circuit_breaker::{CircuitBreaker, CircuitBreakerBase, State};
use crate::circuit_breakers::kucoin_breaker::KuCoinBreaker;
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{Credentials, KuCoinClient};
use crate::models::common::{CircuitBreakerConfig, Market};
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

static BIGNUMBER_BASE: u128 = 1000000000000000000;
static HEDGE_PERIOD_DURATION: u64 = 1;

pub struct HGR {
    pub market: Market,
    pub cb_config: CircuitBreakerConfig,
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
    #[allow(dead_code)]
    bluefin_account: AccountData,
    bluefin_position: UserPosition,
}

impl HGR {
    pub fn new(market: Market, cb_config: CircuitBreakerConfig) -> HGR {
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
        }
    }
}

pub trait Hedger {
    fn connect(&mut self);
    fn hedge(&mut self, dry_run:bool);
    fn hedge_pos(&mut self, dry_run:bool);
}

impl Hedger for HGR {
    fn connect(&mut self) {
        let vars: EnvVars = env::env_variables();
        let (tx_bluefin_pos_update, _rx_bluefin_pos_update) = mpsc::channel();
        let (tx_bluefin_order_update, _rx_bluefin_order_update) = mpsc::channel();
        let (tx_bluefin_account_data_update, _rx_bluefin_account_data_update) = mpsc::channel();

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
                    let mut quantity = Decimal::from_u128(user_position.quantity).unwrap()
                        / Decimal::from(BIGNUMBER_BASE);

                    if !user_position.side{
                        quantity = quantity * Decimal::from_i128(-1).unwrap();
                    }

                    tracing::info!(
                        market = user_position.symbol,
                        bluefin_real_quantity = quantity.to_f64().unwrap(),
                        "Bluefin Quantity"
                    );
                    user_position
                },
            );
        });


        let bluefin_market = self.market.symbols.bluefin.to_owned();
        let bluefin_market_for_order_fill = bluefin_market.clone();
        let bluefin_auth_token = self.bluefin_client.auth_token.clone();
        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_account_data_update = thread::spawn(move || {
            stream_bluefin_private_socket(
                &bluefin_websocket_url,
                &bluefin_market_for_order_fill,
                &bluefin_auth_token,
                "AccountDataUpdate",
                tx_bluefin_account_data_update, // Sender channel of the appropriate type
                |msg: &str| -> AccountData {
                    let account_event_update_data: AccountUpdateEventData =
                        serde_json::from_str(&msg).unwrap();
                    let ad = account_event_update_data.data.account_data;
                    tracing::info!(
                        wallet_balance = ad.wallet_balance,
                        total_position_qty_reduced = ad.total_position_qty_reduced,
                        total_position_qty_reducible = ad.total_position_qty_reducible,
                        total_position_margin = ad.total_position_margin,
                        total_unrealized_profit = ad.total_unrealized_profit,
                        total_expected_pnl = ad.total_expected_pnl,
                        free_collateral = ad.free_collateral,
                        account_value = ad.account_value,
                        "Bluefin Account Data"
                    );
                    ad
                },
            );
        });

        thread::sleep(Duration::from_secs(5));
        let dry_run = vars.dry_run;
        self.hedge(dry_run);
    }

    fn hedge(&mut self, dry_run:bool) {
        loop {
            self.hedge_pos(dry_run);
            // Sleep for one second before next iteration
            thread::sleep(Duration::from_secs(HEDGE_PERIOD_DURATION));
        }
    }
    fn hedge_pos(&mut self, dry_run:bool) {

        let bluefin_market = self.market.symbols.bluefin.to_owned();

        // the call now returns Option(UserPosition)
        let kucoin_position = self.kucoin_client.get_position(&bluefin_market);

        // if we are unable to get KuCoin position just return
        // this is possible due to rate limiting
        if kucoin_position.is_none() {
            return;
        }

        let kucoin_position = kucoin_position.unwrap();

        // unwrap kucoin position and get quantity
        let kucoin_quantity = Decimal::from(kucoin_position.quantity);

        let current_kucoin_qty = kucoin_quantity / Decimal::from(self.market.lot_size);

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

        if order_quantity >= Decimal::from_str(&self.market.min_size).unwrap() && !dry_run {
            {
                tracing::debug!("order quantity as decimal: {}", order_quantity);
                let order_quantity_f64 = order_quantity.to_f64().unwrap();
                tracing::debug!("order quantity as f64: {}", order_quantity_f64);
                let order = self.bluefin_client.create_market_order(
                    &bluefin_market,
                    is_buy,
                    false,
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
