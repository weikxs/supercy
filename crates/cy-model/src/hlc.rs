//! Hybrid logical clocks: timestamps that order events across devices even when wall clocks disagree.

use serde::{Deserialize, Serialize};

/// Length of the [`Hlc::to_bytes`] encoding.
pub const HLC_ENCODED_LEN: usize = 20;

/// Identifies a device inside clocks and items (8 bytes derived from its identity).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct DeviceTag(pub [u8; 8]);

/// A hybrid logical clock stamp: physical milliseconds, a counter for events within the same
/// millisecond, and the device. Ordered by `(wall_ms, counter, device)`, so stamps from different
/// devices never tie.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Hlc {
    wall_ms: u64,
    counter: u32,
    device: DeviceTag,
}

impl Hlc {
    /// A stamp from its parts.
    #[must_use]
    pub const fn new(wall_ms: u64, counter: u32, device: DeviceTag) -> Self {
        Self {
            wall_ms,
            counter,
            device,
        }
    }

    /// Physical time in milliseconds since the Unix epoch.
    #[must_use]
    pub const fn wall_ms(&self) -> u64 {
        self.wall_ms
    }

    /// Counter for events in the same millisecond.
    #[must_use]
    pub const fn counter(&self) -> u32 {
        self.counter
    }

    /// The device that issued the stamp.
    #[must_use]
    pub const fn device(&self) -> DeviceTag {
        self.device
    }

    /// Big-endian encoding whose byte order equals the stamp order, so storage can sort by it.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; HLC_ENCODED_LEN] {
        let mut out = [0u8; HLC_ENCODED_LEN];
        out[..8].copy_from_slice(&self.wall_ms.to_be_bytes());
        out[8..12].copy_from_slice(&self.counter.to_be_bytes());
        out[12..].copy_from_slice(&self.device.0);
        out
    }

    /// Decodes [`Hlc::to_bytes`]; `None` when the length is wrong.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let bytes: &[u8; HLC_ENCODED_LEN] = bytes.try_into().ok()?;
        let (wall, rest) = bytes.split_at(8);
        let (counter, device) = rest.split_at(4);
        Some(Self {
            wall_ms: u64::from_be_bytes(wall.try_into().ok()?),
            counter: u32::from_be_bytes(counter.try_into().ok()?),
            device: DeviceTag(device.try_into().ok()?),
        })
    }
}

/// Issues stamps for one device. The clock is a value: [`HlcClock::tick`] returns the next clock
/// together with the stamp.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HlcClock {
    last: Hlc,
}

impl HlcClock {
    /// A clock for `device` that has issued nothing yet.
    #[must_use]
    pub const fn new(device: DeviceTag) -> Self {
        Self {
            last: Hlc::new(0, 0, device),
        }
    }

    /// A clock that continues after `last` (for example the newest stamp in storage).
    #[must_use]
    pub const fn resume(last: Hlc) -> Self {
        Self { last }
    }

    /// The last stamp issued.
    #[must_use]
    pub const fn last(&self) -> Hlc {
        self.last
    }

    /// The next stamp for a local event at physical time `now_ms`. Stamps always increase, even
    /// when the wall clock goes backwards.
    #[must_use]
    pub fn tick(self, now_ms: u64) -> (Self, Hlc) {
        let device = self.last.device;
        let stamp = if now_ms > self.last.wall_ms {
            Hlc::new(now_ms, 0, device)
        } else {
            Hlc::new(
                self.last.wall_ms,
                self.last.counter.saturating_add(1),
                device,
            )
        };
        (Self { last: stamp }, stamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const DEMO_DEVICE: DeviceTag = DeviceTag([1; 8]);

    #[test]
    fn tick_follows_the_wall_clock() {
        let clock = HlcClock::new(DEMO_DEVICE);

        let (clock, first) = clock.tick(1_000);
        let (_, second) = clock.tick(2_000);

        assert_eq!(first, Hlc::new(1_000, 0, DEMO_DEVICE));
        assert_eq!(second, Hlc::new(2_000, 0, DEMO_DEVICE));
    }

    #[test]
    fn tick_never_goes_backwards_when_the_wall_clock_does() {
        let (clock, first) = HlcClock::new(DEMO_DEVICE).tick(5_000);

        let (clock, second) = clock.tick(4_000);
        let (_, third) = clock.tick(5_000);

        assert!(first < second && second < third);
        assert_eq!(second.wall_ms(), 5_000);
        assert_eq!(third.counter(), 2);
    }

    #[test]
    fn resume_continues_after_the_given_stamp() {
        let stored = Hlc::new(9_000, 3, DEMO_DEVICE);

        let (_, next) = HlcClock::resume(stored).tick(1_000);

        assert!(next > stored);
        assert_eq!(HlcClock::resume(stored).last(), stored);
    }

    #[test]
    fn from_bytes_rejects_wrong_lengths() {
        assert_eq!(Hlc::from_bytes(&[0; 3]), None);
    }

    proptest! {
        #[test]
        fn encoding_round_trips_and_keeps_order(
            a in (any::<u64>(), any::<u32>(), any::<[u8; 8]>()),
            b in (any::<u64>(), any::<u32>(), any::<[u8; 8]>()),
        ) {
            let a = Hlc::new(a.0, a.1, DeviceTag(a.2));
            let b = Hlc::new(b.0, b.1, DeviceTag(b.2));

            prop_assert_eq!(Hlc::from_bytes(&a.to_bytes()), Some(a));
            prop_assert_eq!(a.cmp(&b), a.to_bytes().cmp(&b.to_bytes()));
            prop_assert_eq!(a.device(), DeviceTag(a.to_bytes()[12..].try_into().unwrap()));
        }
    }
}
