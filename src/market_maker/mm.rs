use crate::bluefin::BluefinClient;
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
use crate::circuit_breakers::circuit_breaker::{CircuitBreaker, CircuitBreakerBase, State};
use crate::circuit_breakers::kucoin_breaker::KuCoinBreaker;

use crate::kucoin::{CallResponse, Credentials, KuCoinClient};
use crate::models::common::{abs, add, divide, multiply, round_to_precision, subtract, BookOperations, Market, OrderBook, CircuitBreakerConfig};
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
}

impl MM {
    pub fn new(market: Market, cb_config: CircuitBreakerConfig) -> (MM, Sender<f64>) {
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
            },
            tx_stats,
        )
    }

    fn cancel_order_breaker(&mut self, bluefin_market: String) -> CancelAllOrdersCircuitBreaker {
        CancelAllOrdersCircuitBreaker {
            circuit_breaker: CircuitBreakerBase {
                config: self.cb_config.clone(),
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new(),
                market: bluefin_market.clone(),
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
    );

    fn calculate_skew_multiplier(skewing_coefficient: f64, percent: f64) -> f64;
    fn create_mm_pair(
        &self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        bid_skew_scale: f64,
        ask_skew_scale: f64,
        shift: f64,
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

        // ---- Circuit Breakers ---- //


        let mut kucoin_ob_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());
        let mut kucoin_ticker_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());

        let mut binance_ob_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());
        let mut binance_ob_diff_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());

        let mut bluefin_ob_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());
        let mut bluefin_ob_diff_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());


        let mut rx_stats_disconnect_breaker = self.cancel_order_breaker(bluefin_market.clone());

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
                    tracing::error!("Kucoin worker has disconnected!");
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
                        self.market_make(ref_ob, mm_ob, tkr_ob, buy_percent, 0.0);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::error!("Kucoin worker has disconnected!");
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
                    tracing::error!("Binance worker has disconnected!");
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
                        self.market_make(&value, mm_ob, tkr_ob, buy_percent, 0.0);
                    }
                    ob_map.insert("binance".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::error!("Binance worker has disconnected!");
                    if !binance_ob_diff_disconnect_breaker.is_open() {
                        binance_ob_diff_disconnect_breaker.on_failure();
                    }

                }
            }

            match rx_bluefin_ob.try_recv() {
                Ok(value) => {
                    tracing::debug!("bluefin ob: {:?}", value);
                    bluefin_ob_disconnect_breaker.on_success();
                    ob_map.insert("bluefin".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::error!("Bluefin worker has disconnected!");
                    if !bluefin_ob_disconnect_breaker.is_open() {
                        bluefin_ob_disconnect_breaker.on_failure();
                    }
                }
            }

            match rx_bluefin_ob_diff.try_recv() {
                Ok(value) => {
                    tracing::debug!("diff of bluefin ob: {:?}", value);
                    bluefin_ob_diff_disconnect_breaker.on_success();
                    if ob_map.len() == 3 {
                        let ref_ob: &OrderBook = ob_map.get("binance").expect("Key not found");
                        let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                        self.market_make(ref_ob, mm_ob, &value, buy_percent, 0.0);
                    }
                    ob_map.insert("bluefin".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::error!("Bluefin worker has disconnected!");
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
                    tracing::error!("statistic worker has disconnected!");
                    if !rx_stats_disconnect_breaker.is_open() {
                        rx_stats_disconnect_breaker.on_failure();
                    }
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
    ) {
        let vars: EnvVars = env::env_variables();
        if self.last_mm_instant.elapsed()
            >= Duration::from_millis(vars.market_making_time_throttle_period)
        {
            let sell_percent = 100.0 - buy_percent;
            // bid making will be skewed if and only if sell_percent is less than 50% otherwise business as usual.
            // Skewing is only for discouragement not encouragement.
            let bid_skew_scale = if sell_percent < 50.0 {
                MM::calculate_skew_multiplier(self.market.skewing_coefficient, sell_percent)
            } else {
                1.0
            };
            // same as bid skewing.
            let ask_skew_scale = if buy_percent < 50.0 {
                MM::calculate_skew_multiplier(self.market.skewing_coefficient, buy_percent)
            } else {
                1.0
            };

            tracing::debug!(
                "bid_skew_scale: {} ask_skew_scale: {}",
                bid_skew_scale,
                ask_skew_scale
            );

            let mm = self.create_mm_pair(
                ref_book,
                mm_book,
                tkr_book,
                bid_skew_scale,
                ask_skew_scale,
                shift,
            );

            tracing::debug!("ref ob: {:?}", &ref_book);
            tracing::debug!("mm ob: {:?}", &mm_book);
            tracing::debug!("tkr_ob: {:?}", &tkr_book);
            tracing::debug!("market making orders: {:?}", &mm);

            self.place_maker_orders(&mm);
            self.last_mm_instant = Instant::now();
        }
    }

    /// Calculates the skewing multiplier in order to disincentivize one side of the order making.
    /// This is needed to ensure long/short balancing.
    ///
    /// # Arguments
    ///
    /// * `skewing_coefficient` - Typically 1.0. May be bigger than 1.0 if market making is to be
    /// seriously disincentivized but values less than 1.0 are not accepted.
    /// * `percent` - Order fills percentage volume of the opposite side of the book.
    /// It can be at most 50.0. Values bigger than 50.0 is not accepted since if so skewing will not
    /// be needed.
    ///
    /// # Example
    ///
    /// let result = calculate_skew_multiplier(1.0, 50.0);
    /// assert_eq!(result, 1.0); for 50 there will be no skewing.
    ///
    /// let result = calculate_skew_multiplier(1.0, 25.0);
    /// assert_eq!(result, 1.5); for 25 there will be more skewing.
    ///
    /// let result = calculate_skew_multiplier(1.0, 0.0);
    /// assert_eq!(result, 2); for 0.0 there will be max skewing.
    ///
    /// All values above are calculated for skewing_coefficient = 1.0. Bigger the value more
    /// disincentivization comes to pass.
    fn calculate_skew_multiplier(skewing_coefficient: f64, percent: f64) -> f64 {
        if percent > 50.0 {
            panic!(
                "percent: {} is out of range. It must be in between 0 and 50",
                percent
            );
        }
        if skewing_coefficient < 1.0 {
            panic!(
                "skewing_coefficient: {} is out of range. It must be bigger than 1.0",
                skewing_coefficient
            );
        }
        skewing_coefficient * (1.0 + (50.0 - percent) / 50.0)
    }

    fn create_mm_pair(
        &self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        bid_skew_scale: f64,
        ask_skew_scale: f64,
        shift: f64,
    ) -> ((Vec<f64>, Vec<f64>), (Vec<f64>, Vec<f64>)) {
        let ref_mid_price = ref_book.calculate_mid_prices();
        let mm_mid_price = mm_book.calculate_mid_prices();
        let spread = if ask_skew_scale > bid_skew_scale  {subtract(&mm_mid_price,&ref_mid_price)} else { subtract(&ref_mid_price,&mm_mid_price) };
        let half_spread = divide(&spread, 2.0);
        tracing::debug!("half_spread: {:?}", half_spread);

        let abs_half_spread = abs(&half_spread);
        tracing::debug!("abs_half_spread: {:?}", abs_half_spread);

        let bid_skew_spread = multiply(&abs_half_spread, bid_skew_scale - 1.0);
        tracing::debug!("bid_skew_spread: {:?}", bid_skew_spread);

        let mut mm_bid_prices = subtract(&mm_mid_price, &half_spread);
        mm_bid_prices = subtract(&mm_bid_prices, &bid_skew_spread);

        let ask_skew_spread = multiply(&abs_half_spread, ask_skew_scale - 1.0);
        tracing::debug!("ask_skew_spread: {:?}", ask_skew_spread);

        let mut mm_ask_prices = add(&mm_mid_price, &half_spread);
        mm_ask_prices = add(&mm_ask_prices, &ask_skew_spread);

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

        if can_place_order && !dry_run {
            let bluefin_market = self.market.symbols.bluefin.to_owned();

            if let Some(top_ask) = self.extract_top_price_and_size(&mm.0) {
                tracing::debug!("top ask to be posted as limit on Kucoin:{:?}", top_ask);

                let price = round_to_precision(top_ask.0, self.market.price_precision);
                let quantity = top_ask.1;

                tracing::debug!("price: {} quantity: {}", price, quantity);
                let ask_order_response =
                    self.kucoin_client
                        .place_limit_order(&bluefin_market, false, price, quantity);
                self.kucoin_ask_order_response = ask_order_response;
            }
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
