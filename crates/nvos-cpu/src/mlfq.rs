//! Multi-Level Feedback Queue (MLFQ) scheduler.
//!
//! Modeled after classic OS literature (Remzi OSTEP & Tanenbaum):
//! - Rule 1: Priority(A) > Priority(B) => A runs.
//! - Rule 2: Priority(A) == Priority(B) => Round Robin with queue's quantum.
//! - Rule 3: Processes enter at highest priority (Q0).
//! - Rule 4: Once a process uses its allotment at Qi, it is demoted to Qi+1.
//! - Rule 5: Periodic starvation priority boost moves all processes back to Q0.

use std::collections::{BTreeMap, VecDeque};
use nvos_core::process::{BlockReason, Pid, PriorityLevel};
use nvos_core::tick::Tick;
use crate::{Scheduler, SchedulerError, SchedulingAction};

/// Default number of priority queues.
pub const DEFAULT_NUM_QUEUES: usize = 3;
/// Default base quantum for highest priority queue Q0.
pub const DEFAULT_BASE_QUANTUM: u32 = 2;
/// Default boost interval in ticks.
pub const DEFAULT_BOOST_INTERVAL: u64 = 100;

/// Multi-Level Feedback Queue (MLFQ) scheduler.
#[derive(Debug)]
pub struct MlfqScheduler {
    queues: Vec<VecDeque<Pid>>,
    quantums: Vec<u32>,
    process_levels: BTreeMap<Pid, usize>,
    current_quantum_used: u32,
    allotment_used: BTreeMap<Pid, u32>,
    boost_interval: u64,
    last_boost_tick: u64,
    running: Option<Pid>,
}

impl MlfqScheduler {
    /// Create a new MLFQ scheduler.
    ///
    /// - `num_queues`: Number of priority queues (at least 1).
    /// - `base_quantum`: Time quantum of Q0 (doubles each lower level).
    /// - `boost_interval`: Ticks between global priority boosts (0 = disabled).
    #[must_use]
    pub fn new(num_queues: usize, base_quantum: u32, boost_interval: u64) -> Self {
        let n = num_queues.max(1);
        let base_q = base_quantum.max(1);
        let mut queues = Vec::with_capacity(n);
        let mut quantums = Vec::with_capacity(n);

        let mut q = base_q;
        for _ in 0..n {
            queues.push(VecDeque::new());
            quantums.push(q);
            q = q.saturating_mul(2); // Each lower queue gets double quantum
        }

        Self {
            queues,
            quantums,
            process_levels: BTreeMap::new(),
            current_quantum_used: 0,
            allotment_used: BTreeMap::new(),
            boost_interval,
            last_boost_tick: 0,
            running: None,
        }
    }

    /// Number of priority levels.
    #[must_use]
    pub fn num_queues(&self) -> usize {
        self.queues.len()
    }

    /// Total count of ready processes across all queues.
    #[must_use]
    pub fn total_ready_count(&self) -> usize {
        self.queues.iter().map(VecDeque::len).sum()
    }

    /// Check which priority level a process currently belongs to.
    #[must_use]
    pub fn process_level(&self, pid: Pid) -> Option<usize> {
        self.process_levels.get(&pid).copied()
    }

    /// Trigger an explicit priority boost: all processes moved to Q0.
    pub fn boost_all(&mut self) {
        let mut all_ready = Vec::new();
        for q in &mut self.queues {
            all_ready.extend(q.drain(..));
        }
        for &pid in &all_ready {
            self.process_levels.insert(pid, 0);
            self.allotment_used.insert(pid, 0);
        }
        self.queues[0].extend(all_ready);

        if let Some(running_pid) = self.running {
            self.process_levels.insert(running_pid, 0);
            self.allotment_used.insert(running_pid, 0);
        }
    }
}

impl Default for MlfqScheduler {
    fn default() -> Self {
        Self::new(DEFAULT_NUM_QUEUES, DEFAULT_BASE_QUANTUM, DEFAULT_BOOST_INTERVAL)
    }
}

impl Scheduler for MlfqScheduler {
    fn name(&self) -> &'static str {
        "MLFQ"
    }

    fn add_process(&mut self, pid: Pid, _priority: PriorityLevel) -> Result<(), SchedulerError> {
        if self.running == Some(pid) || self.process_levels.contains_key(&pid) {
            return Err(SchedulerError::DuplicateProcess(pid));
        }

        // Rule 3: Enters at highest priority Q0
        self.process_levels.insert(pid, 0);
        self.allotment_used.insert(pid, 0);
        self.queues[0].push_back(pid);
        Ok(())
    }

    fn schedule_next(&mut self, current_tick: Tick) -> Result<Option<Pid>, SchedulerError> {
        // Check Rule 5: Periodic Starvation Boost
        if self.boost_interval > 0 {
            let elapsed = current_tick.0.saturating_sub(self.last_boost_tick);
            if elapsed >= self.boost_interval {
                self.boost_all();
                self.last_boost_tick = current_tick.0;
            }
        }

        // Rule 1: Find highest priority non-empty queue
        for level in 0..self.queues.len() {
            if let Some(next_pid) = self.queues[level].pop_front() {
                self.running = Some(next_pid);
                self.current_quantum_used = 0;
                return Ok(Some(next_pid));
            }
        }

        self.running = None;
        self.current_quantum_used = 0;
        Ok(None)
    }

    fn tick(&mut self, running_pid: Option<Pid>) -> Result<SchedulingAction, SchedulerError> {
        if self.running != running_pid {
            self.running = running_pid;
            self.current_quantum_used = 0;
        }

        if let Some(pid) = running_pid {
            let current_level = self.process_levels.get(&pid).copied().unwrap_or(0);
            let queue_quantum = self.quantums[current_level];

            self.current_quantum_used = self.current_quantum_used.saturating_add(1);
            let total_allotment = self.allotment_used.entry(pid).or_insert(0);
            *total_allotment = total_allotment.saturating_add(1);

            // Rule 4: Check if allotment used at this level requires demotion
            if *total_allotment >= queue_quantum {
                let next_level = (current_level + 1).min(self.queues.len() - 1);
                self.process_levels.insert(pid, next_level);
                self.allotment_used.insert(pid, 0); // reset for the new level
                self.queues[next_level].push_back(pid);
                self.running = None;
                self.current_quantum_used = 0;
                return Ok(SchedulingAction::Preempt);
            }

            // Check if higher priority process arrived
            for level in 0..current_level {
                if !self.queues[level].is_empty() {
                    self.queues[current_level].push_back(pid);
                    self.running = None;
                    self.current_quantum_used = 0;
                    return Ok(SchedulingAction::Preempt);
                }
            }

            // Check Round Robin within same level if quantum expired
            if self.current_quantum_used >= queue_quantum {
                if !self.queues[current_level].is_empty() {
                    self.queues[current_level].push_back(pid);
                    self.running = None;
                    self.current_quantum_used = 0;
                    return Ok(SchedulingAction::Preempt);
                }
                self.current_quantum_used = 0;
            }

            Ok(SchedulingAction::Continue)
        } else {
            self.current_quantum_used = 0;
            Ok(SchedulingAction::Continue)
        }
    }

    fn on_process_blocked(&mut self, pid: Pid, _reason: BlockReason) -> Result<(), SchedulerError> {
        if self.running == Some(pid) {
            self.running = None;
            self.current_quantum_used = 0;
            Ok(())
        } else if let Some(&level) = self.process_levels.get(&pid) {
            if let Some(pos) = self.queues[level].iter().position(|&p| p == pid) {
                self.queues[level].remove(pos);
                Ok(())
            } else {
                Err(SchedulerError::ProcessNotFound(pid))
            }
        } else {
            Err(SchedulerError::ProcessNotFound(pid))
        }
    }

    fn on_process_terminated(&mut self, pid: Pid) -> Result<(), SchedulerError> {
        let level = self.process_levels.remove(&pid);
        self.allotment_used.remove(&pid);

        if self.running == Some(pid) {
            self.running = None;
            self.current_quantum_used = 0;
            Ok(())
        } else if let Some(lvl) = level {
            if let Some(pos) = self.queues[lvl].iter().position(|&p| p == pid) {
                self.queues[lvl].remove(pos);
                Ok(())
            } else {
                Err(SchedulerError::ProcessNotFound(pid))
            }
        } else {
            Err(SchedulerError::ProcessNotFound(pid))
        }
    }
}
