use crate::circuit_breakers::circuit_breaker::{CircuitBreaker, CircuitBreakerBase};

struct ThresholdCircuitBreaker {

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