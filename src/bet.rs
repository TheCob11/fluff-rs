use std::{
    cmp::Ordering::{self, Equal, Greater, Less},
    fmt::{Display, Formatter, Write},
    num::NonZeroUsize,
};

use thiserror::Error;

// SAFETY: 1≠0 :/
const NONZERO_ONE: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub struct Bet {
    // Field order is necessary for ord derivation, since its a lexicographic ordering of (count, roll)
    pub count: NonZeroUsize,
    pub roll: NonZeroUsize,
}

#[derive(Error, Debug, Serialize, Deserialize, Copy, Clone)]
pub enum RaiseError {
    #[error("Count can not be decreased, but it changed from {prev} to {new}")]
    CountDecreased {
        prev: NonZeroUsize,
        new: NonZeroUsize,
    },
    #[error("Bet can not be the same, but it stayed as {0}")]
    SameBet(Bet),
    #[error("Roll can not be decreased unless count is increased, but roll changed from {prev} to {new}")]
    SameCountLowerRoll {
        prev: NonZeroUsize,
        new: NonZeroUsize,
    },
}

// this is pretty much copy-pasted from src and made const, probably not really doing anything worthwhile but i get a little dopamine boost from seeing "const" lol
#[inline]
const fn const_cmp(a: NonZeroUsize, b: NonZeroUsize) -> Ordering {
    let (a, b) = (a.get(), b.get());
    // The order here is important to generate more optimal assembly.
    // See <https://github.com/rust-lang/rust/issues/63758> for more info.
    if a < b {
        Less
    } else if a == b {
        Equal
    } else {
        Greater
    }
}

impl Bet {
    #[must_use]
    pub const fn new(count: NonZeroUsize, roll: NonZeroUsize) -> Self {
        Self { count, roll }
    }
    pub const fn raise(&self, count: NonZeroUsize, roll: NonZeroUsize) -> Result<Self, RaiseError> {
        let new_bet = Self { count, roll };
        match new_bet.is_raised_from(self) {
            Ok(()) => Ok(new_bet),
            Err(err) => Err(err),
        }
    }
    pub const fn is_raised_from(&self, previous: &Self) -> Result<(), RaiseError> {
        match const_cmp(self.count, previous.count) {
            Less => Err(RaiseError::CountDecreased {
                prev: previous.count,
                new: self.count,
            }),
            Greater => Ok(()),
            Equal => match const_cmp(self.roll, previous.roll) {
                Less => Err(RaiseError::SameCountLowerRoll {
                    prev: previous.roll,
                    new: self.roll,
                }),
                Equal => Err(RaiseError::SameBet(*self)),
                Greater => Ok(()),
            },
        }
    }

    pub fn count_matches(&self, rolls: impl IntoIterator<Item = NonZeroUsize>) -> usize {
        rolls
            .into_iter()
            .filter(|x| self.roll.eq(x) || NONZERO_ONE.eq(x))
            .count()
    }

    pub fn is_fluff(&self, rolls: impl IntoIterator<Item = NonZeroUsize>) -> bool {
        self.count_matches(rolls) < self.count.get()
    }
}

impl Display for Bet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.count, self.roll)?;
        if self.count != NONZERO_ONE {
            f.write_char('s')?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_raised_from() {
        const RANGE: std::ops::Range<usize> = 1..4;
        for (new_count, new_roll, prev_count, prev_roll) in
            itertools::iproduct!(RANGE, RANGE, RANGE, RANGE)
        {
            // convert usize to NonZeroUsize
            let new_count = new_count.try_into().unwrap();
            let new_roll = new_roll.try_into().unwrap();
            let prev_count = prev_count.try_into().unwrap();
            let prev_roll = prev_roll.try_into().unwrap();
            let b_prev = Bet {
                count: prev_count,
                roll: prev_roll,
            };
            let b_new = Bet {
                count: new_count,
                roll: new_roll,
            };
            // println!("{:?} from {:?}: {:?}", b_new, b_prev, b_new.is_raised_from(&b_prev));
            let raised = b_new.is_raised_from(&b_prev);
            assert_eq!(raised.is_ok(), b_new > b_prev);
            match raised {
                Ok(()) => assert!(
                    (new_count > prev_count) || (new_count == prev_count && new_roll > prev_roll)
                ),
                Err(RaiseError::SameBet(_)) => assert_eq!(b_new, b_prev),
                Err(RaiseError::SameCountLowerRoll { prev, new }) => {
                    assert_eq!(new_count, prev_count);
                    assert_eq!(new_roll, new);
                    assert_eq!(prev_roll, prev);
                    assert!(new < prev);
                }
                Err(RaiseError::CountDecreased { prev, new }) => {
                    assert_eq!(new_count, new);
                    assert_eq!(prev_count, prev);
                    assert!(new < prev);
                }
            }
        }
    }
}
