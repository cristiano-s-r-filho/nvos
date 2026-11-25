//! Virtual clock primitives. All simulation time is measured in `Tick(u64)`.
//! Wall-clock time is never used.

use std::fmt;
use std::ops::{Add, AddAssign, Sub};

/// A discrete virtual clock tick. The simulation advances one tick at a time.
/// Never wraps — u64 at 1 GHz simulated ticks takes ~584 years to overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tick(pub u64);

/// A duration expressed in discrete ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DurationTicks(pub u64);

impl Tick {
    /// The simulation epoch (tick zero).
    pub const ZERO: Self = Self(0);

    /// Advance by one tick.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Compute elapsed ticks since `earlier`. Returns 0 if `earlier > self`.
    #[must_use]
    pub fn elapsed_since(self, earlier: Self) -> DurationTicks {
        DurationTicks(self.0.saturating_sub(earlier.0))
    }
}

impl Add<DurationTicks> for Tick {
    type Output = Self;
    fn add(self, rhs: DurationTicks) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl AddAssign<DurationTicks> for Tick {
    fn add_assign(&mut self, rhs: DurationTicks) {
        self.0 = self.0.saturating_add(rhs.0);
    }
}

impl Sub for Tick {
    type Output = DurationTicks;
    fn sub(self, rhs: Self) -> DurationTicks {
        DurationTicks(self.0.saturating_sub(rhs.0))
    }
}

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "T{}", self.0)
    }
}

impl fmt::Display for DurationTicks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}Δt", self.0)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Tick {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Tick {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self(u64::deserialize(d)?))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for DurationTicks {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for DurationTicks {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self(u64::deserialize(d)?))
    }
}
