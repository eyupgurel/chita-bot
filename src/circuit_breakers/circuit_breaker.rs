use crate::circuit_breakers::kucoin_breaker::KuCoinBreaker;
use crate::models::common::CircuitBreakerConfig;

pub struct CircuitBreakerBase {
    pub config: CircuitBreakerConfig,
    pub num_failures: u8,
    pub state: State,
    pub kucoin_breaker: KuCoinBreaker,
    pub market: String,
}

#[derive(PartialEq, Debug)]
pub enum State {
    Closed,
    HalfOpen,
    Open
}

pub trait CircuitBreaker {
    fn on_success(&mut self);
    fn on_failure(&mut self);
    fn open(&mut self) -> bool;

    fn is_open(&self) -> bool;
}
