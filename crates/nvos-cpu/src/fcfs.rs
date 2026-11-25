//! First-Come, First-Served (FCFS) non-preemptive scheduler.

use std::collections::VecDeque;
use nvos_core::process::{BlockReason, Pid, PriorityLevel};
use nvos_core::tick::Tick;
use crate::{Scheduler, SchedulerError, SchedulingAction};

/// First-Come, First-Served (FCFS) scheduler.
///
/// Non-preemptive: Once a process starts running on the CPU, it continues
/// running until it voluntarily yields, blocks, or terminates.
#[derive(Debug, Default)]
pub struct FcfsScheduler {
    ready_queue: VecDeque<Pid>,
    running: Option<Pid>,
}

impl FcfsScheduler {
    /// Create a new, empty FCFS scheduler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of processes currently in the ready queue.
    #[must_use]
    pub fn ready_count(&self) -> usize {
        self.ready_queue.len()
    }

    /// The currently tracked running process.
    #[must_use]
    pub fn running_process(&self) -> Option<Pid> {
        self.running
    }
}

impl Scheduler for FcfsScheduler {
    fn name(&self) -> &'static str {
        "FCFS"
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
            Ok(Some(next_pid))
        } else {
            self.running = None;
            Ok(None)
        }
    }

    fn tick(&mut self, running_pid: Option<Pid>) -> Result<SchedulingAction, SchedulerError> {
        self.running = running_pid;
        // Non-preemptive scheduler: always continues running until process blocks or terminates
        Ok(SchedulingAction::Continue)
    }

    fn on_process_blocked(&mut self, pid: Pid, _reason: BlockReason) -> Result<(), SchedulerError> {
        if self.running == Some(pid) {
            self.running = None;
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
            Ok(())
        } else if let Some(pos) = self.ready_queue.iter().position(|&p| p == pid) {
            self.ready_queue.remove(pos);
            Ok(())
        } else {
            Err(SchedulerError::ProcessNotFound(pid))
        }
    }
}
