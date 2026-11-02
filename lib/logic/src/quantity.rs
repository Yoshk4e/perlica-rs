//! Checked positive quantities for inventory, costs and rewards.

use std::num::NonZeroU32;

/// A strictly positive quantity. Proves representation and positivity only,
/// never ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quantity(NonZeroU32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum QuantityError {
    #[error("quantity must be positive")]
    Zero,
    #[error("quantity is out of range")]
    OutOfRange,
}

impl TryFrom<i64> for Quantity {
    type Error = QuantityError;

    fn try_from(raw: i64) -> Result<Self, Self::Error> {
        let value = u32::try_from(raw).map_err(|_| QuantityError::OutOfRange)?;
        Self::from_u32(value)
    }
}

impl TryFrom<u32> for Quantity {
    type Error = QuantityError;

    fn try_from(raw: u32) -> Result<Self, Self::Error> {
        Self::from_u32(raw)
    }
}

impl Quantity {
    fn from_u32(value: u32) -> Result<Self, QuantityError> {
        NonZeroU32::new(value).map(Self).ok_or(QuantityError::Zero)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0
            .get()
            .checked_add(other.0.get())
            .and_then(NonZeroU32::new)
            .map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0
            .get()
            .checked_sub(other.0.get())
            .and_then(NonZeroU32::new)
            .map(Self)
    }
}

impl std::fmt::Display for Quantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_and_negative() {
        assert_eq!(Quantity::try_from(0_i64), Err(QuantityError::Zero));
        assert_eq!(Quantity::try_from(-1_i64), Err(QuantityError::OutOfRange));
    }

    #[test]
    fn rejects_out_of_range() {
        assert_eq!(
            Quantity::try_from(i64::from(u32::MAX) + 1),
            Err(QuantityError::OutOfRange)
        );
    }

    #[test]
    fn accepts_positive_values() {
        let q = Quantity::try_from(7_i64).expect("valid quantity");
        assert_eq!(q.get(), 7);
        assert_eq!(q.to_string(), "7");
    }

    #[test]
    fn checked_add_overflow_is_none() {
        let max = Quantity::try_from(u32::MAX).expect("valid quantity");
        let one = Quantity::try_from(1_u32).expect("valid quantity");
        assert_eq!(max.checked_add(one), None);
        let four = Quantity::try_from(4_u32).expect("valid quantity");
        assert_eq!(four.checked_add(one).map(Quantity::get), Some(5));
    }

    #[test]
    fn checked_sub_to_zero_is_none() {
        let four = Quantity::try_from(4_u32).expect("valid quantity");
        let one = Quantity::try_from(1_u32).expect("valid quantity");
        assert_eq!(four.checked_sub(four), None);
        assert_eq!(four.checked_sub(one).map(Quantity::get), Some(3));
    }
}
