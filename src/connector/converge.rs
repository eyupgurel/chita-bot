use crate::sockets::binance_socket::stream_binance_socket;
use crate::sockets::kucoin_socket::stream_kucoin_socket;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use log::debug;
use crate::models::common::OrderBook;

pub async fn converge() {
    let (tx_kucoin, rx_kucoin) = mpsc::channel();
    let (tx_binance, rx_binance) = mpsc::channel();

    let _handle_kucoin = thread::spawn(move || {
        stream_kucoin_socket(tx_kucoin);
    });

    let _handle_binance = thread::spawn(|| {
        stream_binance_socket(tx_binance);
    });

    let mut shared_map: HashMap<String, OrderBook> = HashMap::new();

    loop {
        // Reap results from kucoin
        match rx_kucoin.try_recv() {
            Ok((key, value)) => {
                shared_map.insert(key, value);
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No message from kucoin yet
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("Kucoin worker has disconnected!");
            }
        }

        // Reap results from binance
        match rx_binance.try_recv() {
            Ok((key, value)) => {
                shared_map.insert(key, value);
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No message from binance yet
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("Binance worker has disconnected!");
            }
        }

        if shared_map.len() == 2 {
            let kucoin_ob: &OrderBook;
            let binance_ob: &OrderBook;

            match shared_map.get("kucoin_ob") {
                Some(_val) => kucoin_ob = _val,
                None => panic!("Key not found"),
            }

            match shared_map.get("binance_ob") {
                Some(_val) => binance_ob = _val,
                None => panic!("Key not found"),
            }

            for (i, (ask, size)) in binance_ob.asks.iter().enumerate() {
                debug!("{}. ask: {}, size: {}", i, ask, size);
            }

            for (i, ask) in kucoin_ob.asks.iter().enumerate() {
                debug!("{}. ask: {:?}", i, ask);
            }

            for (i, bid) in kucoin_ob.bids.iter().enumerate() {
                debug!("{}. bid: {:?}", i, bid);
            }
        }
    }
}
