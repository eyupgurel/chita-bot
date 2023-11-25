use crate::models::binance_models::DepthUpdate;
use crate::models::bluefin_models::OrderbookDepthUpdate;
use crate::sockets::binance_ob_socket::BinanceOrderBookStream;
use crate::sockets::bluefin_ob_socket::BluefinOrderBookStream;
use crate::sockets::common::OrderBookStream;
use crate::sockets::kucoin_ob_socket::{stream_kucoin_socket};
#[allow(unused_imports)]
use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use crate::bluefin::{BluefinClient};
use crate::env;
use crate::env::EnvVars;

use crate::models::common::{add, divide, subtract, BookOperations, OrderBook, round_to_precision, Market};
use crate::models::kucoin_models::{Level2Depth};
use crate::sockets::kucoin_ticker_socket::stream_kucoin_ticker_socket;
use crate::sockets::kucoin_utils::get_kucoin_url;
use crate::kucoin::{CallResponse, Credentials, KuCoinClient};

pub struct MM {
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
    pub fn new(market: Market) -> (MM, Sender<f64>) {
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

        (MM {
            market,
            bluefin_client,
            kucoin_client,
            kucoin_ask_order_response: CallResponse { error: None, order_id: None },
            kucoin_bid_order_response: CallResponse { error: None, order_id: None },
            last_mm_instant: Instant::now(),
            rx_stats
        }, tx_stats)
    }
}

pub trait MarketMaker {
    fn connect(&mut self);
    fn market_make(
        &mut self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        shift: f64,
    );
    fn create_mm_pair(
        &self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        shift: f64,
    ) -> ((Vec<f64>, Vec<f64>), (Vec<f64>, Vec<f64>));
    fn extract_top_price_and_size(
        &self,
        prices_and_sizes: &(Vec<f64>, Vec<f64>)
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
                false
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
            let url = format!("{}/ws/{}@depth5@100ms", &vars.binance_websocket_url, &binance_market_for_ob);
            ob_stream.stream_ob_socket(&url, &binance_market_for_ob, tx_binance_ob, tx_binance_ob_diff);
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
        let mut _stat:f64;
        loop {
            match rx_kucoin_ob.try_recv() {
                Ok((key, value)) => {
                    debug!("kucoin ob: {:?}", value);
                    ob_map.insert(key.to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Kucoin worker has disconnected!");
                }
            }

            match rx_kucoin_ticker.try_recv() {
                Ok((key, value)) => {
                    debug!("kucoin ticker {}: {:?}", key, value);
                    if ob_map.len() == 3 {
                        let ref_ob: &OrderBook = ob_map.get("binance").expect("Key not found");
                        let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                        let tkr_ob: &OrderBook = ob_map.get("bluefin").expect("Key not found");
                        self.market_make(ref_ob, mm_ob, tkr_ob, -0.1);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Kucoin worker has disconnected!");
                }
            }

            match rx_binance_ob.try_recv() {
                Ok(value) => {
                    debug!("binance ob: {:?}", value);
                    ob_map.insert("binance".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Binance worker has disconnected!");
                }
            }

            match rx_binance_ob_diff.try_recv() {
                Ok(value) => {
                    debug!("diff of binance ob: {:?}", value);
                    if ob_map.len() == 3 {
                        let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                        let tkr_ob: &OrderBook = ob_map.get("bluefin").expect("Key not found");
                        self.market_make(&value, mm_ob, tkr_ob, -0.1);
                    }
                    ob_map.insert("binance".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Binance worker has disconnected!");
                }
            }

            match rx_bluefin_ob.try_recv() {
                Ok(value) => {
                    debug!("bluefin ob: {:?}", value);
                    ob_map.insert("bluefin".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Bluefin worker has disconnected!");
                }
            }

            match rx_bluefin_ob_diff.try_recv() {
                Ok(value) => {
                    debug!("diff of bluefin ob: {:?}", value);
                    if ob_map.len() == 3 {
                        let ref_ob: &OrderBook = ob_map.get("binance").expect("Key not found");
                        let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                        self.market_make(ref_ob, mm_ob, &value, -0.1);
                    }
                    ob_map.insert("bluefin".to_string(), value);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from binance yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Bluefin worker has disconnected!");
                }
            }

            match self.rx_stats.try_recv(){
                Ok(buy_percent) => {
                    debug!("buy percent: {:?}", buy_percent);
                   _stat = buy_percent;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message from kucoin yet
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("statistic worker has disconnected!");
                }
            }
            self.debug_ob_map(&ob_map);
        }
    }

    fn market_make(&mut self, ref_book: &OrderBook, mm_book: &OrderBook, tkr_book: &OrderBook, shift: f64) {
        let vars: EnvVars = env::env_variables();
        if self.last_mm_instant.elapsed() >= Duration::from_millis(vars.market_making_time_throttle_period) {
            let mm = self.create_mm_pair(ref_book, mm_book, tkr_book, shift);

            debug!("ref ob: {:?}", &ref_book);
            debug!("mm ob: {:?}", &mm_book);
            debug!("tkr_ob: {:?}", &tkr_book);
            info!("market making orders: {:?}", &mm);

            self.place_maker_orders(&mm);
            self.last_mm_instant = Instant::now();
        }
    }

    fn create_mm_pair(
        &self,
        ref_book: &OrderBook,
        mm_book: &OrderBook,
        tkr_book: &OrderBook,
        shift: f64,
    ) -> ((Vec<f64>, Vec<f64>), (Vec<f64>, Vec<f64>)) {
        let ref_mid_price = ref_book.calculate_mid_prices();
        let mm_mid_price = mm_book.calculate_mid_prices();
        let spread = subtract(&ref_mid_price, &mm_mid_price);
        let half_spread = divide(&spread, 2.0);
        let mm_bid_prices = subtract(&mm_mid_price, &half_spread);
        let mm_ask_prices = add(&mm_mid_price, &half_spread);
        let mm_bid_sizes = tkr_book.bid_shift(shift);
        let mm_ask_sizes = tkr_book.ask_shift(shift);
        ((mm_ask_prices, mm_ask_sizes), (mm_bid_prices, mm_bid_sizes))
    }

    fn extract_top_price_and_size(
        &self,
        prices_and_sizes: &(Vec<f64>, Vec<f64>)
    ) -> Option<(f64, u128)> {
        let (prices, sizes) = prices_and_sizes;
        // Check the first element of prices and sizes
        if let (Some(&price), Some(&size)) = (prices.first(), sizes.first()) {
            // Ensure the size is positive and non-zero
            if size.is_sign_positive() && size > 0.0 {
                let lot_size = (Decimal::from_f64(size).unwrap() * Decimal::from_u128(self.market.lot_size).unwrap()).floor();
                let upper_bound = Decimal::from_u128(self.market.mm_lot_upper_bound).unwrap();
                let order_size = if lot_size > upper_bound {upper_bound} else {lot_size};
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
        let res =self.kucoin_client.cancel_all_orders(Some(&bluefin_market));
        let can_place_order = res.error.is_none();
        let vars: EnvVars = env::env_variables();
        let dry_run = vars.dry_run;

        if can_place_order && !dry_run {
            let bluefin_market = self.market.symbols.bluefin.to_owned();

            if let Some(top_ask) = self.extract_top_price_and_size(&mm.0) {
                debug!("top ask to be posted as limit on Kucoin:{:?}",top_ask);

                let price = round_to_precision(top_ask.0,self.market.price_precision);
                let quantity = top_ask.1;

                debug!("price: {} quantity: {}", price, quantity);
                let ask_order_response = self.kucoin_client.place_limit_order(&bluefin_market, false, price, quantity);
                self.kucoin_ask_order_response = ask_order_response;
            }
            if let Some(top_bid) = self.extract_top_price_and_size(&mm.1) {
                debug!("top bid to be posted as limit on Kucoin:{:?}",top_bid);

                let price = round_to_precision(top_bid.0,self.market.price_precision);
                let quantity = top_bid.1;

                debug!("price: {} quantity: {}", price, quantity);

                let bid_order_response = self.kucoin_client.place_limit_order(&bluefin_market, true, price, quantity);
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
                debug!("{}. ask: {}, size: {}", i, ask, size);
            }

            for (i, (bid, size)) in binance_ob.bids.iter().enumerate() {
                debug!("{}. bid: {}, size: {}", i, bid, size);
            }

            for (i, ask) in kucoin_ob.asks.iter().enumerate() {
                debug!("{}. ask: {:?}", i, ask);
            }

            for (i, bid) in kucoin_ob.bids.iter().enumerate() {
                debug!("{}. bid: {:?}", i, bid);
            }

            for (i, (ask, size)) in bluefin_ob.asks.iter().enumerate() {
                debug!("{}. ask: {}, size: {}", i, ask, size);
            }

            for (i, (bid, size)) in bluefin_ob.bids.iter().enumerate() {
                debug!("{}. bid: {}, size: {}", i, bid, size);
            }
        }
    }

}
