use serde::{Deserialize, Serialize};

use crate::item::{WALLET_DIAMOND_AMOUNT, WALLET_GOLD_AMOUNT};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletState {
    pub gold: u64,
    pub diamond: u64,
}

impl Default for WalletState {
    fn default() -> Self {
        Self {
            gold: WALLET_GOLD_AMOUNT,
            diamond: WALLET_DIAMOND_AMOUNT,
        }
    }
}

impl WalletState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_balances(gold: u64, diamond: u64) -> Self {
        Self { gold, diamond }
    }

    pub fn try_deduct_gold(&mut self, amount: u32) -> bool {
        if self.gold >= amount as u64 {
            self.gold -= amount as u64;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_wallet_has_initial_balances() {
        let w = WalletState::default();
        assert_eq!(w.gold, WALLET_GOLD_AMOUNT);
        assert_eq!(w.diamond, WALLET_DIAMOND_AMOUNT);
    }

    #[test]
    fn new_matches_default() {
        let w1 = WalletState::new();
        let w2 = WalletState::default();
        assert_eq!(w1.gold, w2.gold);
        assert_eq!(w1.diamond, w2.diamond);
    }

    #[test]
    fn with_balances_sets_values() {
        let w = WalletState::with_balances(500, 100);
        assert_eq!(w.gold, 500);
        assert_eq!(w.diamond, 100);
    }

    #[test]
    fn try_deduct_gold_success() {
        let mut w = WalletState::with_balances(1000, 0);
        assert!(w.try_deduct_gold(500));
        assert_eq!(w.gold, 500);
    }

    #[test]
    fn try_deduct_gold_exact_amount() {
        let mut w = WalletState::with_balances(1000, 0);
        assert!(w.try_deduct_gold(1000));
        assert_eq!(w.gold, 0);
    }

    #[test]
    fn try_deduct_gold_insufficient() {
        let mut w = WalletState::with_balances(100, 0);
        assert!(!w.try_deduct_gold(200));
        // Balance should remain unchanged on failure
        assert_eq!(w.gold, 100);
    }

    #[test]
    fn try_deduct_gold_zero_amount() {
        let mut w = WalletState::with_balances(100, 0);
        assert!(w.try_deduct_gold(0));
        assert_eq!(w.gold, 100);
    }

    #[test]
    fn wallet_serialization_roundtrip() {
        let w = WalletState::with_balances(12345, 67890);
        let json = serde_json::to_string(&w).unwrap();
        let decoded: WalletState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.gold, 12345);
        assert_eq!(decoded.diamond, 67890);
    }
}
