use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;
use crate::bluefin::{AccountData, AccountUpdateEventData, BluefinClient};
use crate::env;
use crate::env::EnvVars;
use crate::kucoin::{AvailableBalance, Credentials, KuCoinClient, TransactionHistory};
use crate::models::common::Config;
use crate::models::kucoin_models::PositionList;
use crate::sockets::bluefin_private_socket::stream_bluefin_private_socket;
use crate::sockets::kucoin_ob_socket::stream_kucoin_socket;

static ACCOUNT_STATS_PERIOD_DURATION: u64 = 60;
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
}

impl AccountStats {
    pub fn new(config: Config, v_tx_account_data: Vec<Sender<AccountData>>) -> AccountStats {
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
            v_tx_account_data
        }
    }
}

impl AccountStatistics for AccountStats{
    fn log(&mut self) {
        let vars: EnvVars = env::env_variables();
        let (tx_bluefin_account_data_update, rx_bluefin_account_data_update) = mpsc::channel();
        let (tx_kucoin_available_balance, _rx_kucoin_available_balance) = mpsc::channel();

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


        let topic = format!("/contractAccount/wallet");
        let kucoin_private_socket_url = self.kucoin_client.get_kucoin_private_socket_url().clone();


        let _handle_kucoin_of = thread::spawn(move || {
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
            let tx_history = self.kucoin_client.get_transaction_history();
            let total_account_balance = <AccountStats as AccountStatistics>::process_transaction_history(&tx_history);

            let position_list = self.kucoin_client.get_position_list();

            let total_unrealised_pnl = <AccountStats as AccountStatistics>::sum_unrealised_pnl(&position_list);


            position_list.data.iter()
                .for_each(|position| {
                    let bluefin_market =  <AccountStats as AccountStatistics>::get_bluefin_symbol(&self.config, &position.symbol);
                    match bluefin_market{
                        Some(value) => tracing::info!(market = value,
                                                            current_qty=position.current_qty,
                                                            pos_margin=position.pos_margin,
                                                             unrealised_pnl= position.unrealised_pnl,
                                                            "Kucoin Market Data"),
                        _ => {}
                    }

                });

            tracing::info!(total_account_balance=total_account_balance,
                           total_unrealised_pnl=total_unrealised_pnl,
                           "Kucoin Account Data");

            match rx_bluefin_account_data_update.try_recv() {
                Ok(value) => {
                    tracing::debug!("AccountData: {:?}", value);
                    let _ = self.v_tx_account_data.iter()
                        .try_for_each(|sender| {
                            let clone = value.clone();
                            sender.send(clone)
                        });
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("AccountData socket has disconnected!");
                }
            };

            thread::sleep(Duration::from_secs(ACCOUNT_STATS_PERIOD_DURATION));
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