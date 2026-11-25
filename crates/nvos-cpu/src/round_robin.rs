//! Round Robin (RR) preemptive scheduler with configurable time quantum.

use std::collections::VecDeque;
use nvos_core::process::{BlockReason, Pid, PriorityLevel};
use nvos_core::tick::Tick;
use crate::{Scheduler, SchedulerError, SchedulingAction};

/// Default time slice quantum in ticks.
pub const DEFAULT_RR_QUANTUM: u32 = 4;

/// Round Robin (RR) preemptive scheduler.
///
/// Preempts a running process once its consumed ticks reach `quantum`.
/// When preempted, the process is returned to the tail of the FIFO queue.
#[derive(Debug)]
pub struct RoundRobinScheduler {
    ready_queue: VecDeque<Pid>,
    quantum: u32,
    quantum_used: u32,
    running: Option<Pid>,
}

impl RoundRobinScheduler {
    /// Create a new Round Robin scheduler with the given quantum (in ticks).
    #[must_use]
    pub fn new(quantum: u32) -> Self {
        Self {
            ready_queue: VecDeque::new(),
            quantum: quantum.max(1),
            quantum_used: 0,
            running: None,
        }
    }

    /// Configured quantum duration in ticks.
    #[must_use]
    pub fn quantum(&self) -> u32 {
        self.quantum
    }

    /// Number of ticks used in the current quantum by the running process.
    #[must_use]
    pub fn quantum_used(&self) -> u32 {
        self.quantum_used
    }

    /// Number of processes currently waiting in the ready queue.
    #[must_use]
    pub fn ready_count(&self) -> usize {
        self.ready_queue.len()
    }
}

impl Default for RoundRobinScheduler {
    fn default() -> Self {
        Self::new(DEFAULT_RR_QUANTUM)
    }
}

impl Scheduler for RoundRobinScheduler {
    fn name(&self) -> &'static str {
        "RoundRobin"
    }

    fn add_process(&mut self, pid: Pid, _priority: PriorityLevel) -> Result<(), SchedulerError> {
        if self.running == Some(pid) || self.ready_queue.contains(&pid) {
            return Err(SchedulerError::DuplicateProcess(pid));
        }
        self.ready_queue.push_back(pid);
        Ok(())
    }

    fn schedule_next(&mut self, _current_tick: Tick) -> Result<Option<Pid>, SchedulerError> {
        if let Some(next_pid) = self.ready_queue.pop_front() {
            self.running = Some(next_pid);
            self.quantum_used = 0;
            Ok(Some(next_pid))
        } else {
            self.running = None;
            self.quantum_used = 0;
            Ok(None)
        }
    }

    fn tick(&mut self, running_pid: Option<Pid>) -> Result<SchedulingAction, SchedulerError> {
        if self.running != running_pid {
            self.running = running_pid;
            self.quantum_used = 0;
        }

        if let Some(pid) = running_pid {
            self.quantum_used = self.quantum_used.saturating_add(1);
            if self.quantum_used >= self.quantum {
                // If there are other waiting processes, preempt and cycle to tail
                if !self.ready_queue.is_empty() {
                    self.ready_queue.push_back(pid);
                    self.running = None;
                    self.quantum_used = 0;
                    return Ok(SchedulingAction::Preempt);
                }
                // If no other processes are ready, reset quantum and continue
                self.quantum_used = 0;
            }
            Ok(SchedulingAction::Continue)
        } else {
            self.quantum_used = 0;
            Ok(SchedulingAction::Continue)
        }
    }

    fn on_process_blocked(&mut self, pid: Pid, _reason: BlockReason) -> Result<(), SchedulerError> {
        if self.running == Some(pid) {
            self.running = None;
            self.quantum_used = 0;
            Ok(())
        } else if let Some(pos) = self.ready_queue.iter().position(|&p| p == pid) {
            self.ready_queue.remove(pos);
            Ok(())
        } else {
            Err(SchedulerError::ProcessNotFound(pid))
        }
    }

    fn on_process_terminated(&mut self, pid: Pid) -> Result<(), SchedulerError> {
        if self.running == Some(pid) {
            self.running = None;
            self.quantum_used = 0;
            Ok(())
        } else if let Some(pos) = self.ready_queue.iter().position(|&p| p == pid) {
            self.ready_queue.remove(pos);
            Ok(())
        } else {
            Err(SchedulerError::ProcessNotFound(pid))
        }
    }
}
