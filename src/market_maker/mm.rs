use crate::bluefin::{AccountData, BluefinClient};
use crate::env;
use crate::env::EnvVars;
use crate::models::binance_models::DepthUpdate;
use crate::models::bluefin_models::OrderbookDepthUpdate;
use crate::sockets::binance_ob_socket::BinanceOrderBookStream;
use crate::sockets::bluefin_ob_socket::BluefinOrderBookStream;
use crate::sockets::common::OrderBookStream;
use crate::sockets::kucoin_ob_socket::stream_kucoin_socket;
#[allow(unused_imports)]
use log::{debug, error, info};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use crate::circuit_breakers::cancel_all_orders_breaker::CancelAllOrdersCircuitBreaker;
use crate::circuit_breakers::threshold_breaker::{ClientType, ThresholdCircuitBreaker};
use crate::circuit_breakers::circuit_breaker::{CircuitBreaker, CircuitBreakerBase, State};
use crate::circuit_breakers::kucoin_breaker::KuCoinBreaker;
use crate::kucoin::{CallResponse, Credentials, KuCoinClient, AvailableBalance};
use crate::models::common::{add, divide, round_to_precision, subtract, abs, BookOperations, Market, OrderBook, CircuitBreakerConfig};
use crate::models::kucoin_models::Level2Depth;
use crate::sockets::kucoin_ticker_socket::stream_kucoin_ticker_socket;
use crate::sockets::kucoin_utils::get_kucoin_url;

pub struct MM {
    pub cb_config: CircuitBreakerConfig,
    pub market: Market,
    #[allow(dead_code)]
    bluefin_client: BluefinClient,
    kucoin_client: KuCoinClient,
    kucoin_ask_order_response: CallResponse,
    kucoin_bid_order_response: CallResponse,
    last_mm_instant: Instant,
    rx_stats: Receiver<f64>,
    rx_account_data: Receiver<AccountData>,
    rx_account_data_kc: Receiver<AvailableBalance>,
    rx_hedger_stats: Receiver<f64>,
    tx_bluefin_hedger_ob: Sender<OrderBook>,
    tx_bluefin_hedger_ob_diff: Sender<OrderBook>,
}

impl MM {
    pub fn new(market: Market, cb_config: CircuitBreakerConfig) -> 
        (MM, Sender<f64>, Sender<AccountData>, Sender<AvailableBalance>, Sender<f64>, Receiver<OrderBook>, Receiver<OrderBook>) {
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
        kucoin_client.cancel_all_orders(Some(&bluefin_market));

        let (tx_stats, rx_stats): (Sender<f64>, Receiver<f64>) = mpsc::channel();
        let (tx_account_data, rx_account_data): (Sender<AccountData>, Receiver<AccountData>) = mpsc::channel();
        let (tx_account_data_kc, rx_account_data_kc) : (Sender<AvailableBalance>, Receiver<AvailableBalance>) = mpsc::channel();
        let (tx_hedger_stats, rx_hedger_stats): (Sender<f64>, Receiver<f64>) = mpsc::channel();
        let (tx_bluefin_hedger_ob, rx_bluefin_hedger_ob): (Sender<OrderBook>, Receiver<OrderBook>) = mpsc::channel();
        let (tx_bluefin_hedger_ob_diff, rx_bluefin_hedger_ob_diff): (Sender<OrderBook>, Receiver<OrderBook>) = mpsc::channel();

        (
            MM {
                cb_config,
                market,
                bluefin_client,
                kucoin_client,
                kucoin_ask_order_response: CallResponse {
                    error: None,
                    order_id: None,
                },
                kucoin_bid_order_response: CallResponse {
                    error: None,
                    order_id: None,
                },
                last_mm_instant: Instant::now(),
                rx_stats,
                rx_account_data,
                rx_account_data_kc,
                rx_hedger_stats,
                tx_bluefin_hedger_ob,
                tx_bluefin_hedger_ob_diff,
            },
            tx_stats,
            tx_account_data,
            tx_account_data_kc,
            tx_hedger_stats,
            rx_bluefin_hedger_ob,
            rx_bluefin_hedger_ob_diff,
        )
    }

    fn cancel_order_breaker(&mut self, market: String) -> CancelAllOrdersCircuitBreaker {
        CancelAllOrdersCircuitBreaker {
            circuit_breaker: CircuitBreakerBase {
                config: self.cb_config.clone(),
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new(),
                market,
            }
        }
    }
}

pub trait MarketMaker {
    fn connect(&mut self);
    fn market_make(
        &mut self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        buy_percent: f64,
        shift: f64,
        kucoin_quantity:f64,
    );

    fn create_mm_pair(
        &self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        shift: f64,
        kucoin_quantity: f64
    ) -> ((Vec<f64>, Vec<f64>), (Vec<f64>, Vec<f64>));
    fn extract_top_price_and_size(
        &self,
        prices_and_sizes: &(Vec<f64>, Vec<f64>),
    ) -> Option<(f64, u128)>;

    fn has_valid_kucoin_ask_order_id(&self) -> bool;

    fn has_valid_kucoin_bid_order_id(&self) -> bool;

    fn place_maker_orders(&mut self, mm: &((Vec<f64>, Vec<f64>), (Vec<f64>, Vec<f64>)));

    fn debug_ob_map(&self, ob_map: &HashMap<String, OrderBook>);
}

impl MarketMaker for MM {
    fn connect(&mut self) {
        let vars: EnvVars = env::env_variables();

        let (tx_kucoin_ob, rx_kucoin_ob) = mpsc::channel();
        let (tx_kucoin_ticker, rx_kucoin_ticker) = mpsc::channel();
        let (tx_binance_ob, rx_binance_ob) = mpsc::channel();
        let (tx_binance_ob_diff, rx_binance_ob_diff) = mpsc::channel();
        let (tx_bluefin_ob, rx_bluefin_ob) = mpsc::channel();
        let (tx_bluefin_ob_diff, rx_bluefin_ob_diff) = mpsc::channel();

        let kucoin_market = self.market.symbols.kucoin.to_owned();
        let kucoin_market_for_ob = kucoin_market.clone();

        let _handle_kucoin_ob = thread::spawn(move || {
            stream_kucoin_socket(
                &get_kucoin_url(),
                &kucoin_market_for_ob.clone(),
                &vars.kucoin_depth_topic,
                tx_kucoin_ob, // Sender channel of the appropriate type
                |msg: &str| -> OrderBook {
                    let parsed_kucoin_ob: Level2Depth =
                        serde_json::from_str(&msg).expect("Can't parse");
                    let ob: OrderBook = parsed_kucoin_ob.into();
                    ob
                },
                "",
                false,
            );
        });

        let kucoin_market_for_ticker = kucoin_market.clone();
        let _handle_kucoin_ticker = thread::spawn(move || {
            stream_kucoin_ticker_socket(&kucoin_market_for_ticker.clone(), tx_kucoin_ticker);
        });

        let binance_market = self.market.symbols.binance.to_owned();
        let binance_market_for_ob = binance_market.clone();

        let _handle_binance_ob = thread::spawn(move || {
            let ob_stream = BinanceOrderBookStream::<DepthUpdate>::new();
            let url = format!(
                "{}/ws/{}@depth5@100ms",
                &vars.binance_websocket_url, &binance_market_for_ob
            );
            ob_stream.stream_ob_socket(
                &url,
                &binance_market_for_ob,
                tx_binance_ob,
                tx_binance_ob_diff,
            );
        });

        let bluefin_market = self.market.symbols.bluefin.to_owned();
        let bluefin_market_for_ob = bluefin_market.clone();

        let bluefin_websocket_url = vars.bluefin_websocket_url.clone();
        let _handle_bluefin_ob = thread::spawn(move || {
            let ob_stream = BluefinOrderBookStream::<OrderbookDepthUpdate>::new();
            ob_stream.stream_ob_socket(
                &bluefin_websocket_url,
                &bluefin_market_for_ob.clone(),
                tx_bluefin_ob,
                tx_bluefin_ob_diff,
            );
        });

        let mut ob_map: HashMap<String, OrderBook> = HashMap::new();
        let mut buy_percent: f64 = 50.0;
        let mut kucoin_quantity: f64 = 0.0;


        // ---- Circuit Breakers ---- //
        let vars = env::env_variables();

        let mut kucoin_ob_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());
        let mut kucoin_ticker_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());

        let mut binance_ob_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());
        let mut binance_ob_diff_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());

        let mut bluefin_ob_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());
        let mut bluefin_ob_diff_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());


        let mut rx_stats_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());


        
        //Account Balance threshold breaker
        let mut account_balance_threshold_breaker = ThresholdCircuitBreaker::new(self.cb_config.clone());

        

        loop {
            match rx_kucoin_ob.try_recv() {
                Ok((key, value)) => {
                    tracing::debug!("kucoin ob: {:?}", value);
                    kucoin_ob_disconnect_breaker.on_success();
                    ob_map.insert(key.to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Kucoin worker has disconnected!");
                    if !kucoin_ob_disconnect_breaker.is_open() {
                        kucoin_ob_disconnect_breaker.on_failure();
                    }
                }
            }

            match rx_kucoin_ticker.try_recv() {
                Ok((key, value)) => {
                    tracing::debug!("kucoin ticker {}: {:?}", key, value);
                    kucoin_ticker_disconnect_breaker.on_success();
                    if ob_map.len() == 3 {
                        let ref_ob: &OrderBook = ob_map.get("binance").expect("Key not found");
                        let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                        let tkr_ob: &OrderBook = ob_map.get("bluefin").expect("Key not found");
                        self.market_make(ref_ob, mm_ob, tkr_ob, buy_percent, 0.0, kucoin_quantity);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Kucoin worker has disconnected!");
                    if !kucoin_ticker_disconnect_breaker.is_open() {
                        kucoin_ticker_disconnect_breaker.on_failure();
                    }
                }
            }

            match rx_binance_ob.try_recv() {
                Ok(value) => {
                    tracing::debug!("binance ob: {:?}", value);
                    binance_ob_disconnect_breaker.on_success();
                    ob_map.insert("binance".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Binance worker has disconnected!");
                    if !binance_ob_disconnect_breaker.is_open() {
                        binance_ob_disconnect_breaker.on_failure();
                    }
                }
            }

            match rx_binance_ob_diff.try_recv() {
                Ok(value) => {
                    tracing::debug!("diff of binance ob: {:?}", value);
                    binance_ob_diff_disconnect_breaker.on_success();
                    if ob_map.len() == 3 {
                        let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                        let tkr_ob: &OrderBook = ob_map.get("bluefin").expect("Key not found");
                        self.market_make(&value, mm_ob, tkr_ob, buy_percent, 0.0, kucoin_quantity);
                    }
                    ob_map.insert("binance".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Binance worker has disconnected!");
                    if !binance_ob_diff_disconnect_breaker.is_open() {
                        binance_ob_diff_disconnect_breaker.on_failure();
                    }

                }
            }

            match rx_bluefin_ob.try_recv() {
                Ok(value) => {
                    tracing::debug!("bluefin ob: {:?}", value);
                    bluefin_ob_disconnect_breaker.on_success();
                    let _ = self.tx_bluefin_hedger_ob.send(value.clone());

                    ob_map.insert("bluefin".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Bluefin worker has disconnected!");
                    if !bluefin_ob_disconnect_breaker.is_open() {
                        bluefin_ob_disconnect_breaker.on_failure();
                    }
                }
            }

            match rx_bluefin_ob_diff.try_recv() {
                Ok(value) => {
                    tracing::debug!("diff of bluefin ob: {:?}", value);
                    bluefin_ob_diff_disconnect_breaker.on_success();
                    let _ = self.tx_bluefin_hedger_ob_diff.send(value.clone());

                    if ob_map.len() == 3 {
                        let ref_ob: &OrderBook = ob_map.get("binance").expect("Key not found");
                        let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                        self.market_make(ref_ob, mm_ob, &value, buy_percent, 0.0, kucoin_quantity);
                    }
                    ob_map.insert("bluefin".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Bluefin worker has disconnected!");
                    if !bluefin_ob_diff_disconnect_breaker.is_open() {
                        bluefin_ob_diff_disconnect_breaker.on_failure();
                    }
                }
            }

            match self.rx_stats.try_recv() {
                Ok(percent) => {
                    tracing::debug!("buy percent: {:?}", percent);
                    rx_stats_disconnect_breaker.on_success();
                    buy_percent = percent;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("statistic worker has disconnected!");
                    if !rx_stats_disconnect_breaker.is_open() {
                        rx_stats_disconnect_breaker.on_failure();
                    }
                }
            }
            

            match self.rx_account_data_kc.try_recv() {
                Ok(value) => {
                    tracing::debug!("Kucoin available balance: {:?}", value.data.available_balance);
                    account_balance_threshold_breaker.check_user_balance(value.data.available_balance, ClientType::KUCOIN, &bluefin_market, vars.dry_run);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("Kucoin account data worker has disconnected!");
                }
            }
            
            match self.rx_account_data.try_recv() {
                Ok(value) => {
                    tracing::debug!("Bluefin available balance: {:?}", value.free_collateral);
                    account_balance_threshold_breaker.check_user_balance(value.free_collateral, ClientType::BLUEFIN, &bluefin_market, vars.dry_run);
                    
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from bluefin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("bluefin account data worker has disconnected!");
                }
            }


            match self.rx_hedger_stats.try_recv() {
                Ok(value) => {
                    tracing::debug!("kucoin quantity: {:?}", value);
                    kucoin_quantity = value;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::debug!("hedger worker has disconnected!");
                }
            }

            self.debug_ob_map(&ob_map);
        }
    }

    

    fn market_make(
        &mut self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        buy_percent: f64,
        shift: f64,
        kucoin_quantity:f64,
    ) {
        let vars: EnvVars = env::env_variables();
        if self.last_mm_instant.elapsed()
            >= Duration::from_millis(vars.market_making_time_throttle_period)
        {
            let mm = self.create_mm_pair(
                ref_book,
                mm_book,
                tkr_book,
                shift,
                kucoin_quantity,
            );

            tracing::debug!("ref ob: {:?}", &ref_book);
            tracing::debug!("mm ob: {:?}", &mm_book);
            tracing::debug!("tkr_ob: {:?}", &tkr_book);
            tracing::debug!("market making orders: {:?}", &mm);


            let mut mm_asks = mm.0;
            let mut mm_bids = mm.1;

            if buy_percent < 50.0
            {
                mm_asks = (Vec::new(), Vec::new());
            }

            if buy_percent > 50.0
            {
                mm_bids = (Vec::new(), Vec::new());
            }

            let (ask_prices, ask_sizes) = mm_asks.clone();

            let (tkr_bid_prices, tkr_bid_sizes): (Vec<f64>, Vec<f64>) = tkr_book.bids.clone().into_iter().unzip();


            let filtered_mm_asks: Vec<(f64, f64)> = ask_prices.into_iter().zip(ask_sizes.into_iter())
                .zip(tkr_bid_prices.into_iter().zip(tkr_bid_sizes.into_iter()))
                .map(|((left1, right1), (left2, right2))| (left1, right1, left2, right2))
                .filter(|&(ask_price, ask_size, tkr_bid_price, tkr_bid_size)|
                    ask_price > tkr_bid_price && ask_price * (10000.0 - 2.0) / 10000.0 >= tkr_bid_price && ask_size <= tkr_bid_size )
                .collect::<Vec<_>>().iter()
                .map(|&(a, b, _, _)| (a, b)) // Keep only the first two elements of each tuple
                .collect();


            let (bid_prices, bid_sizes) = mm_bids.clone();

            let (tkr_ask_prices, tkr_ask_sizes): (Vec<f64>, Vec<f64>) = tkr_book.asks.clone().into_iter().unzip();


            let filtered_mm_bids: Vec<(f64, f64)> = bid_prices.into_iter().zip(bid_sizes.into_iter())
                .zip(tkr_ask_prices.into_iter().zip(tkr_ask_sizes.into_iter()))
                .map(|((left1, right1), (left2, right2))| (left1, right1, left2, right2))
                .filter(|&(bid_price, bid_size, tkr_ask_price, tkr_ask_size)|
                   bid_price < tkr_ask_price && bid_price * (10000.0 + 2.0) / 10000.0 <= tkr_ask_price && bid_size <= tkr_ask_size)
                .collect::<Vec<_>>().iter()
                .map(|&(a, b, _, _)| (a, b)) // Keep only the first two elements of each tuple
                .collect();

            let (ask_prices, ask_sizes): (Vec<f64>, Vec<f64>) = filtered_mm_asks.into_iter().unzip();
            let (bid_prices, bid_sizes): (Vec<f64>, Vec<f64>) = filtered_mm_bids.into_iter().unzip();

            self.place_maker_orders(&((ask_prices, ask_sizes), (bid_prices, bid_sizes)));
            self.last_mm_instant = Instant::now();
        }
    }

    fn create_mm_pair(
        &self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        shift: f64,
        kucoin_quantity: f64
    ) -> ((Vec<f64>, Vec<f64>), (Vec<f64>, Vec<f64>)) {
        let ref_mid_price = ref_book.calculate_mid_prices();
        let mm_mid_price = mm_book.calculate_mid_prices();
        let spread =  abs(&subtract(&ref_mid_price,&mm_mid_price)); // use absolute value for spread
        let half_spread = divide(&spread, 2.0);
        tracing::debug!("half_spread: {:?}", half_spread);


        let mut mm_bid_prices = subtract(&mm_mid_price, &half_spread);

        if kucoin_quantity > 0.0
        {
            mm_bid_prices = subtract(&mm_mid_price, &half_spread);
        }

        let mut mm_ask_prices = add(&mm_mid_price, &half_spread);

        if kucoin_quantity < 0.0
        {
            mm_ask_prices = add(&mm_mid_price, &half_spread);
        }

        let mm_bid_sizes = tkr_book.bid_shift(shift);
        let mm_ask_sizes = tkr_book.ask_shift(shift);
        ((mm_ask_prices, mm_ask_sizes), (mm_bid_prices, mm_bid_sizes))
    }

    fn extract_top_price_and_size(
        &self,
        prices_and_sizes: &(Vec<f64>, Vec<f64>),
    ) -> Option<(f64, u128)> {
        let (prices, sizes) = prices_and_sizes;
        // Check the first element of prices and sizes
        if let (Some(&price), Some(&size)) = (prices.first(), sizes.first()) {
            // Ensure the size is positive and non-zero
            if size.is_sign_positive() && size > 0.0 {
                let lot_size = (Decimal::from_f64(size).unwrap()
                    * Decimal::from_u128(self.market.lot_size).unwrap())
                .floor();
                let upper_bound = Decimal::from_u128(self.market.mm_lot_upper_bound).unwrap();
                let order_size = if lot_size > upper_bound {
                    upper_bound
                } else {
                    lot_size
                };
                Some((price, Decimal::to_u128(&order_size).unwrap()))
            } else {
                None
            }
        } else {
            // Return None if there is no first element
            None
        }
    }

    fn has_valid_kucoin_ask_order_id(&self) -> bool {
        self.kucoin_ask_order_response.order_id.is_some()
    }

    fn has_valid_kucoin_bid_order_id(&self) -> bool {
        self.kucoin_bid_order_response.order_id.is_some()
    }

    fn place_maker_orders(&mut self, mm: &((Vec<f64>, Vec<f64>), (Vec<f64>, Vec<f64>))) {
        let bluefin_market = self.market.symbols.bluefin.to_owned();
        let res = self.kucoin_client.cancel_all_orders(Some(&bluefin_market));
        let can_place_order = res.error.is_none();
        let vars: EnvVars = env::env_variables();
        let dry_run = vars.dry_run;
        // Check if the first and second elements are empty
        let are_asks_empty = mm.0.0.is_empty() && mm.0.1.is_empty();
        let are_bids_empty = mm.1.0.is_empty() && mm.1.1.is_empty();


        if can_place_order && !dry_run {
            let bluefin_market = self.market.symbols.bluefin.to_owned();

            if !are_asks_empty {
                if let Some(top_ask) = self.extract_top_price_and_size(&mm.0) {
                    tracing::debug!("top ask to be posted as limit on Kucoin:{:?}", top_ask);

                    let price = round_to_precision(top_ask.0, self.market.price_precision);
                    let quantity = top_ask.1;

                    tracing::info!("price: {} quantity: {}", price, quantity);
                    tracing::info!("Attempting to market make...");
                    let ask_order_response =
                        self.kucoin_client
                            .place_limit_order(&bluefin_market, false, price, quantity);
                    self.kucoin_ask_order_response = ask_order_response;
                }
            }
            if !are_bids_empty {
                if let Some(top_bid) = self.extract_top_price_and_size(&mm.1) {
                    tracing::debug!("top bid to be posted as limit on Kucoin:{:?}", top_bid);

                    let price = round_to_precision(top_bid.0, self.market.price_precision);
                    let quantity = top_bid.1;

                    tracing::debug!("price: {} quantity: {}", price, quantity);

                    let bid_order_response =
                        self.kucoin_client
                            .place_limit_order(&bluefin_market, true, price, quantity);
                    self.kucoin_bid_order_response = bid_order_response;
                }
            }
        }
    }

    #[allow(dead_code)]
    fn debug_ob_map(&self, ob_map: &HashMap<String, OrderBook>) {
        if ob_map.len() == 3 {
            let binance_ob: &OrderBook = ob_map.get("binance").expect("Key not found");
            let kucoin_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
            let bluefin_ob: &OrderBook = ob_map.get("bluefin").expect("Key not found");

            for (i, (ask, size)) in binance_ob.asks.iter().enumerate() {
                tracing::debug!("{}. ask: {}, size: {}", i, ask, size);
            }

            for (i, (bid, size)) in binance_ob.bids.iter().enumerate() {
                tracing::debug!("{}. bid: {}, size: {}", i, bid, size);
            }

            for (i, ask) in kucoin_ob.asks.iter().enumerate() {
                tracing::debug!("{}. ask: {:?}", i, ask);
            }

            for (i, bid) in kucoin_ob.bids.iter().enumerate() {
                tracing::debug!("{}. bid: {:?}", i, bid);
            }

            for (i, (ask, size)) in bluefin_ob.asks.iter().enumerate() {
                tracing::debug!("{}. ask: {}, size: {}", i, ask, size);
            }

            for (i, (bid, size)) in bluefin_ob.bids.iter().enumerate() {
                tracing::debug!("{}. bid: {}, size: {}", i, bid, size);
            }
        }
    }
}
