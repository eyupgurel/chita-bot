use rand::{distributions::Alphanumeric, Rng};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
}; // 0.8

pub fn get_current_time() -> u128 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let in_ms = (since_the_epoch.as_secs() * 1000
        + since_the_epoch.subsec_nanos() as u64 / 1_000_000) as u128;

    return in_ms;
}

pub fn get_random_number() -> u128 {
    return get_current_time() + u128::from(rand::random::<u32>());
}

#[allow(dead_code)]
pub fn get_random_string() -> String {
    return rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(20)
        .map(char::from)
        .collect();
}

/// Formats a query from a provided referenced hash map. Note, ordering is not assured.
#[allow(dead_code)]
pub fn format_query<S: ::std::hash::BuildHasher>(params: &HashMap<String, String, S>) -> String {
    let mut query = String::new();
    for (key, val) in params.iter() {
        let segment = format!("{}={}", key, val);
        if query.is_empty() {
            query = format!("?{}", segment);
        } else {
            query = format!("{}&{}", query, segment);
        }
    }
    query
}
