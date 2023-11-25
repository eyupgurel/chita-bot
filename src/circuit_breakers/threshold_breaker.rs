use crate::circuit_breakers::circuit_breaker::{CircuitBreaker, CircuitBreakerBase, State};
use crate::circuit_breakers::kucoin_breaker::KuCoinBreaker;
use crate::models::common::CircuitBreakerConfig;

pub struct ThresholdFormula {
    user_balance_base: f64,
    user_balance_current: f64,
}

impl ThresholdFormula {
    pub fn get_current_user_balance(&self) -> f64 {

    }

    pub fn cache_user_balance() -> f64 {

    }
}

pub struct ThresholdCircuitBreaker {
    circuit_breaker: CircuitBreakerBase,
    formula: ThresholdFormula,
}

impl CircuitBreaker for ThresholdCircuitBreaker {
    fn on_success(&mut self) {
        todo!()
    }

    fn on_failure(&mut self) {
        todo!()
    }

    fn open(&mut self) -> bool {
        todo!()
    }

    fn is_open(&self) -> bool {
        todo!()
    }
}