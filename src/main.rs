use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use crate::binance_models::DepthUpdate;
use crate::binance_socket::{stream_binance_socket};
use crate::kucoin_models::Level2Depth;
use crate::kucoin_socket::{stream_kucoin_socket};
use crate::r#type::Either;

mod binance_models;
mod kucoin_models;
mod kucoin_socket;
mod binance_socket;
mod r#type;

fn main() {
    let (tx_kucoin, rx_kucoin) = mpsc::channel();
    let (tx_binance, rx_binance) = mpsc::channel();

    let _handle_kucoin = thread::spawn(move || {
        stream_kucoin_socket(tx_kucoin);
    });

    let _handle_binance = thread::spawn(|| {
        stream_binance_socket(tx_binance);
    });

    let mut shared_map: HashMap<String, Either<Level2Depth, DepthUpdate>> = HashMap::new();

    loop {
        // Reap results from kucoin
        match rx_kucoin.try_recv() {
            Ok((key, value)) => {
                shared_map.insert(key, Either::Left(value));
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
                shared_map.insert(key, Either::Right(value));
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No message from binance yet
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("Binance worker has disconnected!");
            }
        }

        if shared_map.len() == 2 {
            let kucoin_ob: &Level2Depth;
            let binance_ob: &DepthUpdate;

            match shared_map.get("kucoin_ob") {
                Some(Either::Left(_val)) => kucoin_ob = _val,
                Some(Either::Right(_val)) => panic!("Error!"),
                None => panic!("Key not found"),
            }

            match shared_map.get("binance_ob") {
                Some(Either::Left(_val)) => panic!("Error!"),
                Some(Either::Right(_val)) => binance_ob = _val,
                None => panic!("Key not found"),
            }

            for (i, (ask, size)) in binance_ob.ask_orders.iter().enumerate() {
                println!("{}. ask: {}, size: {}", i, ask, size);
            }

            for (i, ask) in kucoin_ob.data.asks.iter().enumerate() {
                println!("{}. ask: {:?}", i, ask);
            }

            for (i, bid) in kucoin_ob.data.bids.iter().enumerate() {
                println!("{}. bid: {:?}", i, bid);
            }
        }
    }
}


