use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use crate::bluefin::models::TradeOrderUpdate;
use crate::bluefin::{AccountData, AccountUpdateEventData, BluefinClient};
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{AvailableBalance, Credentials, KuCoinClient, TransactionHistory};
use crate::models::common::Config;
use crate::models::kucoin_models::PositionList;
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;
use crate::sockets::kucoin_socket::stream_kucoin_socket;
use serde_json::Value;
use crate::bluefin::models::parse_user_trade_order_update;


static ACCOUNT_STATS_PERIOD_DURATION: u64 = 3;

pub trait AccountStatistics {
    fn log(&mut self);
    fn process_transaction_history(transaction_history: &TransactionHistory) -> f64;
    fn sum_unrealised_pnl(position_list: &PositionList) -> f64;
    fn get_bluefin_symbol(config: &Config, kucoin_symbol: &str) -> Option<String>;
}
pub struct AccountStats {
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
    config: Config,
    v_tx_account_data: Vec<Sender<AccountData>>,
    v_tx_account_data_kc: Vec<Sender<AvailableBalance>>,
    v_tx_account_data_bluefin_user_trade: Vec<Sender<TradeOrderUpdate>>,
}

impl AccountStats {
    pub fn new(
        config: Config, 
        v_tx_account_data: Vec<Sender<AccountData>>,
        v_tx_account_data_kc: Vec<Sender<AvailableBalance>>,
        v_tx_account_data_bluefin_user_trade: Vec<Sender<TradeOrderUpdate>>) -> AccountStats {
        
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

        AccountStats {
            bluefin_client,
            kucoin_client,
            config,
            v_tx_account_data,
            v_tx_account_data_kc,
            v_tx_account_data_bluefin_user_trade
        }
    }
}

impl AccountStatistics for AccountStats{
    fn log(&mut self) {
        let vars: EnvVars = env::env_variables();
        let (tx_bluefin_account_data_update, rx_bluefin_account_data_update) = mpsc::channel();
        let (tx_kucoin_available_balance, rx_kucoin_available_balance) = mpsc::channel();
        let (tx_bluefin_trade_order_update, rx_bluefin_trade_order_update) = mpsc::channel();


        let bluefin_auth_token = self.bluefin_client.auth_token.clone();
        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_account_data_update = thread::spawn(move || {
            stream_bluefin_private_socket(
                &bluefin_websocket_url,
                &"",
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

        let bluefin_auth_token = self.bluefin_client.auth_token.clone();
        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_account_data_update = thread::spawn(move || {
            stream_bluefin_private_socket(
                &bluefin_websocket_url,
                &"",
                &bluefin_auth_token,
                "UserTrade",
                tx_bluefin_trade_order_update, // Sender channel of the appropriate type
                |msg: &str| -> TradeOrderUpdate {
                    tracing::info!("User Trade: {}", msg);

                    let v: Value = serde_json::from_str(&msg).unwrap();
                    let user_trade: TradeOrderUpdate = parse_user_trade_order_update(serde_json::from_value(v["data"]["trade"].clone()).unwrap());

                    tracing::info!(
                        bluefin_market=user_trade.symbol,
                        bluefin_commission_fee=user_trade.commission,
                        "Bluefin User Trade"
                    );

                    user_trade
                },
            );
        });



        let topic = format!("/contractAccount/wallet");
        let kucoin_private_socket_url = self.kucoin_client.get_kucoin_private_socket_url().clone();
        let _handle_kucoin_of = thread::spawn(move || {
            tracing::info!("Creating Kucoin Account Balance handler...");
            stream_kucoin_socket(
                &kucoin_private_socket_url,
                &"",
                &topic,
                tx_kucoin_available_balance, // Sender channel of the appropriate type
                |msg: &str| -> AvailableBalance {
                    let available_balance: AvailableBalance =
                        serde_json::from_str(&msg).expect("Can't parse");
                    tracing::info!(available_balance=available_balance.data.available_balance,
                                   hold_balance=available_balance.data.hold_balance,
                                   "Kucoin Account Balance");
                    available_balance
                },
                &"availableBalance.change",
                true
            );
        });

        loop {

            match rx_bluefin_trade_order_update.try_recv() {
                Ok(value) => {
                    let _ = self.v_tx_account_data_bluefin_user_trade.iter()
                        .try_for_each(|sender| {
                            let clone = value.clone();
                            sender.send(clone)
                        });
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::info!("Bluefin UserTrade socket has disconnected!");
                }
            }
           
            match rx_kucoin_available_balance.try_recv() {
                Ok(value) => {
                    let balance = value.1;
                    tracing::info!("Kucoin Available Balance: {:?}", balance);
                    let _ = self.v_tx_account_data_kc.iter()
                        .try_for_each(|sender| {
                            let clone = balance.clone();
                            sender.send(clone)
                        });
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::info!("Kucoin AvailableBalance socket has disconnected!");
                }
            }

            match rx_bluefin_account_data_update.try_recv() {
                Ok(value) => {
                    tracing::info!("Bluefin AccountData: {:?}", value);
                    let _ = self.v_tx_account_data.iter()
                        .try_for_each(|sender| {
                            let clone = value.clone();
                            sender.send(clone)
                        });
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from bluefin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::info!("Bluefin AccountData socket has disconnected!");
                }
            }

            // thread::sleep(Duration::from_secs(ACCOUNT_STATS_PERIOD_DURATION));

        }
    }

    fn process_transaction_history(transaction_history: &TransactionHistory) -> f64 {
        // Extract the initial account equity from the first transaction, or default to 0 if empty
        let initial_account_equity = transaction_history
            .data
            .data_list
            .first()
            .map_or(0.0, |t| t.account_equity);

        // Calculate the total amount using functional style
        let total_amount: f64 = transaction_history
            .data
            .data_list
            .iter()
            .fold(0.0, |acc, t| acc + t.amount);

        // Add the total amount to the initial account equity
        initial_account_equity + total_amount
    }

    fn sum_unrealised_pnl(position_list: &PositionList) -> f64 {
        position_list
            .data
            .iter()
            .map(|position| position.unrealised_pnl)
            .sum()
    }

    fn get_bluefin_symbol(config: &Config, kucoin_symbol: &str) -> Option<String> {
        for market in &config.markets {
            if market.symbols.kucoin == kucoin_symbol {
                return Some(market.symbols.bluefin.clone());
            }
        }
        None
    }

}