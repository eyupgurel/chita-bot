use crate::sockets::binance_ob_socket::{BinanceOrderBookStream};
use crate::sockets::kucoin_ob_socket::stream_kucoin_ob_socket;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use log::debug;
use crate::market_maker::mm::create_mm_pair;

use crate::models::common::{OrderBook};
use crate::sockets::bluefin_ob_socket::{BluefinOrderBookStream};
use crate::sockets::common::OrderBookStream;

use crate::sockets::kucoin_ticker_socket::stream_kucoin_ticker_socket;

pub async fn converge() {
    let (tx_kucoin_ob, rx_kucoin_ob) = mpsc::channel();
    let (tx_kucoin_ticker, rx_kucoin_ticker) = mpsc::channel();
    let (tx_binance_ob, rx_binance_ob) = mpsc::channel();
    let (tx_binance_ob_diff, rx_binance_ob_diff) = mpsc::channel();
    let (tx_bluefin_ob, rx_bluefin_ob) = mpsc::channel();
    let (tx_bluefin_ob_diff, rx_bluefin_ob_diff) = mpsc::channel();

    let _handle_kucoin_ob = thread::spawn(move || {
        stream_kucoin_ob_socket("XBTUSDTM", tx_kucoin_ob);
    });

    let _handle_kucoin_ticker = thread::spawn(move || {
        stream_kucoin_ticker_socket("XBTUSDTM", tx_kucoin_ticker);
    });

    let _handle_binance_ob = thread::spawn(|| {
        let ob_stream = BinanceOrderBookStream{};
        ob_stream.stream_ob_socket("btcusdt", tx_binance_ob, tx_binance_ob_diff);
    });

    let _handle_bluefin_ob = thread::spawn(|| {
        let ob_stream = BluefinOrderBookStream{};
        ob_stream.stream_ob_socket("BTC-PERP", tx_bluefin_ob,tx_bluefin_ob_diff);
    });

    let mut ob_map: HashMap<String, OrderBook> = HashMap::new();

    loop {
        match rx_kucoin_ob.try_recv() {
            Ok((key, value)) => {
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
                debug!("diff of {}: {:?}", key, value);
                if ob_map.len() == 3 {
                    let ref_ob: &OrderBook = ob_map.get("binance").expect("Key not found");
                    let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                    let tkr_ob: &OrderBook = ob_map.get("bluefin").expect("Key not found");
                    let mm = create_mm_pair(ref_ob, mm_ob, tkr_ob, -0.1);
                    debug!("orders: {:?}", mm);
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
            Ok((key, value)) => {
                ob_map.insert(key.to_string(), value);
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No message from binance yet
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("Binance worker has disconnected!");
            }
        }

        match rx_binance_ob_diff.try_recv() {
            Ok((key, value)) => {
                debug!("diff of {}: {:?}", key, value);
                if ob_map.len() == 3 {
                    let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                    let tkr_ob: &OrderBook = ob_map.get("bluefin").expect("Key not found");
                    let mm = create_mm_pair(&value, mm_ob, tkr_ob, -0.1);
                    debug!("orders: {:?}", mm);
                }
                ob_map.insert(key.to_string(), value);
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No message from binance yet
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("Binance worker has disconnected!");
            }
        }


        match rx_bluefin_ob.try_recv() {
            Ok((key, value)) => {
                ob_map.insert(key.to_string(), value);
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No message from binance yet
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("Bluefin worker has disconnected!");
            }
        }

        match rx_bluefin_ob_diff.try_recv() {
            Ok((key, value)) => {
                debug!("diff of {}: {:?}", key, value);
                if ob_map.len() == 3 {
                    let ref_ob: &OrderBook = ob_map.get("binance").expect("Key not found");
                    let mm_ob: &OrderBook = ob_map.get("kucoin").expect("Key not found");
                    let mm = create_mm_pair(ref_ob, mm_ob, &value, -0.1);
                    debug!("orders {:?}", mm);
                }
                ob_map.insert(key.to_string(), value);
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No message from binance yet
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("Bluefin worker has disconnected!");
            }
        }
        //debug_ob_map(&ob_map);
    }
}

#[allow(dead_code)]
fn debug_ob_map(ob_map: &HashMap<String, OrderBook>){
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