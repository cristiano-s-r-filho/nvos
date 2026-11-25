//! Virtual programmable interval timer generating discrete tick interrupts.

use nvos_core::tick::Tick;

/// Virtual Programmable Interval Timer (PIT).
#[derive(Debug)]
pub struct VirtualTimer {
    frequency_ticks: u64,
    last_interrupt_tick: u64,
    total_interrupts_fired: u64,
}

impl VirtualTimer {
    /// Create a new virtual timer that fires every `frequency_ticks` (e.g. 1 tick).
    #[must_use]
    pub fn new(frequency_ticks: u64) -> Self {
        Self {
            frequency_ticks: frequency_ticks.max(1),
            last_interrupt_tick: 0,
            total_interrupts_fired: 0,
        }
    }

    /// Check if the timer expired at `current_tick`.
    /// Returns `true` and updates internal counters if an interrupt fires.
    pub fn poll(&mut self, current_tick: Tick) -> bool {
        let elapsed = current_tick.0.saturating_sub(self.last_interrupt_tick);
        if elapsed >= self.frequency_ticks {
            self.last_interrupt_tick = current_tick.0;
            self.total_interrupts_fired = self.total_interrupts_fired.saturating_add(1);
            true
        } else {
            false
        }
    }

    /// Total interrupts fired since creation.
    #[must_use]
    pub fn total_interrupts(&self) -> u64 {
        self.total_interrupts_fired
    }
}

impl Default for VirtualTimer {
    fn default() -> Self {
        Self::new(1)
    }
}
