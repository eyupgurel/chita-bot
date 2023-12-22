use chrono::Duration;
use chrono::Utc;
use chrono::DateTime;

use crate::models::common::CircuitBreakerConfig;

use super::kucoin_breaker::KuCoinBreaker;

pub struct ThresholdCircuitBreaker {
    config: CircuitBreakerConfig,
    kucoin_breaker: KuCoinBreaker,
    kucoin_balance_stack: Vec<f64>, 
    bluefin_balance_stack: Vec<f64>,

    daily_base_balance: f64,
    prev_poll: DateTime<Utc>,
}

pub enum ClientType {
    KUCOIN,
    BLUEFIN,
}


impl ThresholdCircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> ThresholdCircuitBreaker {
        ThresholdCircuitBreaker {
            config,
            kucoin_breaker: KuCoinBreaker::new(),
            kucoin_balance_stack: vec![],
            bluefin_balance_stack: vec![],

            daily_base_balance: -1.0,
            prev_poll: Utc::now(),
    
        }
    }

    fn push_balance(&mut self, balance: f64, client_type: ClientType) {
        match client_type {
            ClientType::KUCOIN => self.kucoin_balance_stack.push(balance),
            ClientType::BLUEFIN => self.bluefin_balance_stack.push(balance),
        };
    }
    
    fn cache_base_account_balance(&mut self) {
        let now = Utc::now();
        if self.bluefin_balance_stack.is_empty() || self.kucoin_balance_stack.is_empty() {
            return;
        } else if self.daily_base_balance == -1.0 || self.prev_poll - now >= Duration::days(1) {
            self.daily_base_balance = self.bluefin_balance_stack.last().unwrap() + self.kucoin_balance_stack.last().unwrap();
            self.prev_poll = now;

            tracing::info!("Caching daily user balance of {}, on date: {}", self.daily_base_balance, self.prev_poll);
        } 
    }

    fn is_balance_critical(&mut self) -> bool {
        if !self.kucoin_balance_stack.is_empty() && !self.bluefin_balance_stack.is_empty() {
            let kc_balance = self.kucoin_balance_stack.pop().expect("Could not fetch balance from Kucoin account stats");
            let bf_balance = self.bluefin_balance_stack.pop().expect("Could not fetch balance from Bluefin account stats");

            let user_balance = bf_balance + kc_balance;

            return !(user_balance >= self.daily_base_balance - (self.daily_base_balance * ({self.config.loss_threshold_bps as f64} /10_000.0)));   
        }
        
        return false;
    }

    fn open_breaker(&mut self, market: &String, dry_run: bool) -> bool {
        self.kucoin_breaker.cancel_all_orders(&self.config, market, dry_run)
    }

    pub fn check_user_balance(&mut self, balance: f64, client_type: ClientType, market: &String, dry_run: bool) {
        self.push_balance(balance, client_type);
        self.cache_base_account_balance();
        if self.is_balance_critical() {
            tracing::info!("User balance is critically low. Cancelling all orders and shutting the bot down...");
            self.open_breaker(market, dry_run);
            panic!("User balance critically low. Cancelling all orders and shutting the bot down to prevent further loss...");
        }
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
    fn test_push_to_stack_bf() {
        let (_, mut breaker) = prepare_breaker();
        breaker.push_balance(3.0, ClientType::BLUEFIN);
        assert_eq!(1, breaker.bluefin_balance_stack.len());
        assert_eq!(&3.0, breaker.bluefin_balance_stack.last().unwrap());
    }

    #[test]
    fn test_push_to_stack_kc() {
        let (_, mut breaker) = prepare_breaker();
        breaker.push_balance(3.0, ClientType::KUCOIN);
        assert_eq!(1, breaker.kucoin_balance_stack.len());
        assert_eq!(3.0, breaker.kucoin_balance_stack.pop().unwrap());
    }

    #[test]
    fn test_neg_stacks() {
        let (_, mut breaker) = prepare_breaker();
        breaker.push_balance(3.0, ClientType::BLUEFIN);
        breaker.push_balance(1.0, ClientType::KUCOIN);

        assert_eq!(1, breaker.bluefin_balance_stack.len());
        assert_eq!(1, breaker.kucoin_balance_stack.len());

        assert_ne!(1.0, breaker.bluefin_balance_stack.pop().unwrap());
        assert_ne!(3.0, breaker.kucoin_balance_stack.pop().unwrap());
    }

    #[test]
    fn test_check_user_balance() {
        let (_, mut breaker) = prepare_breaker();
        breaker.check_user_balance(3.0, ClientType::BLUEFIN, &String::from("ETHUSDTM"), true);
        breaker.check_user_balance(3.0, ClientType::KUCOIN, &String::from("ETHUSDTM"), true);
        
        assert!(breaker.bluefin_balance_stack.is_empty());
        assert!(breaker.kucoin_balance_stack.is_empty());

        assert_eq!(6.0, breaker.daily_base_balance);
        
    }

    #[test]
    fn test_open_breaker() {
        let (_, mut breaker) = prepare_breaker();
        assert!(breaker.open_breaker(&String::from("ETHUSDTM"), true));
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

            breaker.push_balance(bluefin_rand, ClientType::BLUEFIN);
            breaker.push_balance(kucoin_rand, ClientType::KUCOIN);

            breaker.cache_base_account_balance();

            // print!("bluefin: {}, kucoin: {}, daily base balance: {}", breaker.bluefin_balance_stack.last().unwrap(), breaker.kucoin_balance_stack.last().unwrap(), breaker.daily_base_balance);

            assert_eq!((bluefin_rand + kucoin_rand), breaker.daily_base_balance); 

            print!("\n");
            i += 1;
        }        
    }

    #[test]
    fn test_is_balance_critical_pass() {
        let (_config, mut breaker) = prepare_breaker();
        
        let bluefin_balance = 2.0;
        let kucoin_balance = 3.0;

        breaker.push_balance(bluefin_balance, ClientType::BLUEFIN);
        breaker.push_balance(kucoin_balance, ClientType::KUCOIN);
        breaker.daily_base_balance = bluefin_balance + kucoin_balance;

        assert_eq!(false, breaker.is_balance_critical());
    }

    #[test]
    fn test_is_balance_critical_fail() {
        let (_config, mut breaker) = prepare_breaker();
        
        let bluefin_balance = 2.0;
        let kucoin_balance = 3.0;

        breaker.push_balance(bluefin_balance, ClientType::BLUEFIN);
        breaker.push_balance(kucoin_balance, ClientType::KUCOIN);
        breaker.daily_base_balance = 2.0 * (bluefin_balance + kucoin_balance);

        assert!(breaker.is_balance_critical());
    }
}