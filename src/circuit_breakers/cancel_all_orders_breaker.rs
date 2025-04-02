use crate::circuit_breakers::circuit_breaker::{CircuitBreaker, CircuitBreakerBase, State};

pub struct CancelAllOrdersCircuitBreaker {
    pub name: String,
    pub circuit_breaker: CircuitBreakerBase
}


impl CircuitBreaker for CancelAllOrdersCircuitBreaker {

    fn on_success(&mut self) {
        self.circuit_breaker.num_failures = 0;
        self.circuit_breaker.state = State::Closed;
    }

    fn on_failure(&mut self) {
        self.circuit_breaker.num_failures += 1;
        if self.circuit_breaker.num_failures > self.circuit_breaker.config.failure_threshold {
            self.circuit_breaker.state = State::Open;
            self.open();
        } else {
            self.circuit_breaker.state = State::HalfOpen;

        }
    }

    fn open(&mut self) -> bool {
        return self.circuit_breaker.kucoin_breaker.cancel_all_orders(&self.circuit_breaker.config, &self.circuit_breaker.market, false);
    }

    fn is_open(&self) -> bool {
        return self.circuit_breaker.state == State::Open;
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use crate::circuit_breakers::kucoin_breaker::KuCoinBreaker;
    use crate::models::common::CircuitBreakerConfig;

    #[test]
    fn test_breaker_failure() {
        let (sender, receiver) = mpsc::channel::<u8>();

        thread::spawn(move || {
            drop(sender);
        });

        let mut breaker: CancelAllOrdersCircuitBreaker = CancelAllOrdersCircuitBreaker {
            name: "Test Breaker".to_string(),
            circuit_breaker: CircuitBreakerBase {
                config: CircuitBreakerConfig {
                    num_retries: 3,
                    failure_threshold: 3
                },
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new("Kucoin Breaker for Test Breaker".to_string()),
                market: "ETH-PERP".to_string(),
            }
        };

        let mut _num_tries = 0;

        match receiver.try_recv() {
            Ok(_value) => {
                breaker.on_success();
                _num_tries += 1;
                return;
            }
            Err(mpsc::TryRecvError::Empty) => {

            }
            Err(mpsc::TryRecvError::Disconnected) => {
                _num_tries += 1;
                breaker.on_failure();

                if _num_tries == 3 {
                    assert_eq!(breaker.circuit_breaker.state, State::HalfOpen);
                } else if _num_tries > 3 {
                    assert_eq!(breaker.circuit_breaker.state, State::Open);
                }

            }
        }
    }

    #[test]
    fn test_breaker_success() {
        let (sender, receiver) = mpsc::channel();

        let var = "something";

        thread::spawn(move || {
            sender.send(var).expect("Error sending message");
        });

        let mut breaker: CancelAllOrdersCircuitBreaker = CancelAllOrdersCircuitBreaker {
            name: "Test Breaker".to_string(),
            circuit_breaker: CircuitBreakerBase {
                config: CircuitBreakerConfig {
                    num_retries: 3,
                    failure_threshold: 3
                },
                num_failures: 0,
                state: State::Closed,
                kucoin_breaker: KuCoinBreaker::new("Kucoin Breaker for Test Breaker".to_string()),
                market: "ETH-PERP".to_string(),
            }
        };

        match receiver.try_recv() {
            Ok(value) => {
                breaker.on_success();
                assert_eq!(var, value);
                assert_eq!(breaker.circuit_breaker.state, State::Closed);
            }
            Err(mpsc::TryRecvError::Empty) => {

            }
            Err(mpsc::TryRecvError::Disconnected) => {
                breaker.on_failure();
                return;
            }
        }
    }
}