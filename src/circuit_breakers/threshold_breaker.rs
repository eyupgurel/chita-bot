use chrono::Duration;
use chrono::Utc;
use chrono::DateTime;

use crate::models::common::CircuitBreakerConfig;

pub struct ThresholdCircuitBreaker {
    config: CircuitBreakerConfig,
    kucoin_balance_stack: Vec<f64>, 
    bluefin_balance_stack: Vec<f64>,

    daily_base_balance: f64,
    prev_poll: DateTime<Utc>,
}


impl ThresholdCircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> ThresholdCircuitBreaker {
        ThresholdCircuitBreaker {
            config,
            kucoin_balance_stack: vec![],
            bluefin_balance_stack: vec![],

            daily_base_balance: -1.0,
            prev_poll: Utc::now(),
    
        }
    }

    pub fn push_kucoin_balance(&mut self, kucoin_balance: f64) {
        self.kucoin_balance_stack.push(kucoin_balance);
    }

    pub fn push_bluefin_balance(&mut self, bluefin_balance: f64) {
        self.bluefin_balance_stack.push(bluefin_balance);
    } 
    
    pub fn cache_base_account_balance(&mut self) {
        let now = Utc::now();
        if self.bluefin_balance_stack.is_empty() || self.kucoin_balance_stack.is_empty() {
            return;
        } else if self.daily_base_balance == -1.0 || self.prev_poll - now >= Duration::days(1) {
            self.daily_base_balance = self.bluefin_balance_stack.pop().unwrap() + self.kucoin_balance_stack.pop().unwrap();
            self.prev_poll = now;
        } 
    }

    pub fn is_balance_critical(&mut self) -> bool {
        if !self.kucoin_balance_stack.is_empty() && !self.bluefin_balance_stack.is_empty() {
            let kc_balance = self.kucoin_balance_stack.pop().expect("Could not fetch balance from Kucoin account stats");
            let bf_balance = self.bluefin_balance_stack.pop().expect("Could not fetch balance from Bluefin account stats");

            let user_balance = bf_balance + kc_balance;

            return !(user_balance >= self.daily_base_balance - (self.daily_base_balance * ({self.config.loss_threshold_bps as f64} /10_000.0)));   
        }
        
        return false;
    }
}

#[cfg(test)]
mod test {
    use rand::Rng;
    use super::*;

    fn prepare_breaker() -> (CircuitBreakerConfig, ThresholdCircuitBreaker) {
        let config = CircuitBreakerConfig {
            num_retries: 3,
            failure_threshold: 3,
            loss_threshold_bps: 3.0,
        };
        let breaker = ThresholdCircuitBreaker::new(config);

        (config, breaker)
    }

    #[test]
    fn test_stacks_daily_balance() {
        let (_config, mut breaker) = prepare_breaker();
        breaker.cache_base_account_balance();
        assert_eq!(-1.0, breaker.daily_base_balance); 

        let mut rng = rand::thread_rng();
        let mut i = 0;
        while i < 10 {
            breaker.daily_base_balance = -1.0;

            let bluefin_rand = rng.gen_range(0.0..10.0);
            let kucoin_rand = rng.gen_range(0.0..10.0);

            breaker.push_bluefin_balance(bluefin_rand);
            breaker.push_kucoin_balance(kucoin_rand);

            breaker.cache_base_account_balance();

            // print!("bluefin: {}, kucoin: {}, daily base balance: {}", breaker.bluefin_balance_stack.last().unwrap(), breaker.kucoin_balance_stack.last().unwrap(), breaker.daily_base_balance);

            assert_eq!((breaker.bluefin_balance_stack.last().unwrap() + breaker.kucoin_balance_stack.last().unwrap()), breaker.daily_base_balance); 

            print!("\n");
            i += 1;
        }        
    }

    #[test]
    fn test_is_balance_critical_pass() {
        let (_config, mut breaker) = prepare_breaker();
        
        let bluefin_balance = 2.0;
        let kucoin_balance = 3.0;

        breaker.push_bluefin_balance(bluefin_balance);
        breaker.push_kucoin_balance(kucoin_balance);
        breaker.daily_base_balance = bluefin_balance + kucoin_balance;

        assert_eq!(false, breaker.is_balance_critical());
    }

    #[test]
    fn test_is_balance_critical_fail() {
        let (_config, mut breaker) = prepare_breaker();
        
        let bluefin_balance = 2.0;
        let kucoin_balance = 3.0;

        breaker.push_bluefin_balance(bluefin_balance);
        breaker.push_kucoin_balance(kucoin_balance);
        breaker.daily_base_balance = 2.0 * (bluefin_balance + kucoin_balance);

        assert!(breaker.is_balance_critical());
    }
}