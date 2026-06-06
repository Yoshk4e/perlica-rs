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
