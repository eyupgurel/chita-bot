use rand::{distributions::Alphanumeric, Rng};
use std::time::{SystemTime, UNIX_EPOCH}; // 0.8

pub fn get_current_time() -> u128 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let in_ms = (since_the_epoch.as_secs() * 1000
        + since_the_epoch.subsec_nanos() as u64 / 1_000_000) as u128;

    return in_ms;
}

pub fn get_time() -> u128 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    since_the_epoch.as_millis()
}

pub fn get_random_number() -> u128 {
    return get_current_time() + u128::from(rand::random::<u32>());
}

pub fn get_random_string() -> String {
    return rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(20)
        .map(char::from)
        .collect();
}
