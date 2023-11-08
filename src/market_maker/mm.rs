use crate::models::common::{add, divide, BookOperations, OrderBook, subtract};
pub fn create_mm_pair(ref_book:&OrderBook, mm_book:&OrderBook, tkr_book:&OrderBook, shift:f64) -> ((Vec<f64>, Vec<f64>), (Vec<f64>, Vec<f64>)) {
    let ref_mid_price = ref_book.calculate_mid_prices();
    let mm_mid_price = mm_book.calculate_mid_prices();
    let spread = subtract(&ref_mid_price, &mm_mid_price);
    let half_spread = divide(&spread,2.0);
    let mm_bid_prices = subtract(&mm_mid_price, &half_spread);
    let mm_ask_prices = add(&mm_mid_price, &half_spread);
    let mm_bid_sizes = tkr_book.bid_shift(shift);
    let mm_ask_sizes = tkr_book.ask_shift(shift);
    ((mm_ask_prices, mm_ask_sizes), (mm_bid_prices, mm_bid_sizes))
}