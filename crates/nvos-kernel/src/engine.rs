//! Core deterministic discrete-event simulation engine.
//!
//! Evaluates time monotonically in discrete units of `Tick(u64)`.
//! Implements the Two-Phase Commit tick cycle:
//! - Phase 1: Passive Request Collection (read-only)
//! - Phase 2: Canonical Arbitration & State Mutation
//!   1. Process Arrivals & Wakeups (New -> Ready, Sleeping -> Ready)
//!   2. Process I/O & Swap Interrupts (Blocked -> Ready)
//!   3. Memory Translation & Access (Running PID)
//!   4. Step CPU Burst (Advance token progress / program counter)
//!   5. Unified Scheduling Arbitration (tick & schedule_next)
//!   6. Commit Telemetry & Tick Snapshot

use crate::arena::ProcessArena;
use crate::panic_boundary::safe_subsystem_dispatch;
use nvos_core::error::{EngineErrorKind, KernelEngineError};
use nvos_core::events::{EngineStatus, KernelRequest, SpanEvent, TraceEvent, TraceEventKind};
use nvos_core::process::{ExitReason, Pid, ProcessControlBlock, ProcessState};
use nvos_core::tape::InstructionToken;
use nvos_core::tick::Tick;
use nvos_cpu::{Scheduler, SchedulingAction};

/// Deterministic discrete-event simulation kernel engine.
pub struct KernelEngine {
    arena: ProcessArena,
    scheduler: Box<dyn Scheduler>,
    current_tick: Tick,
    running_pid: Option<Pid>,
    is_halted: bool,
    pending_arrivals: Vec<Pid>,
    next_pid_counter: u32,
    trace_events: Vec<TraceEvent>,
    span_events: Vec<SpanEvent>,
}

impl KernelEngine {
    /// Create a new simulation engine with the given scheduler policy.
    pub fn new(scheduler: Box<dyn Scheduler>) -> Self {
        Self {
            arena: ProcessArena::new(),
            scheduler,
            current_tick: Tick::ZERO,
            running_pid: None,
            is_halted: false,
            pending_arrivals: Vec::new(),
            next_pid_counter: 1,
            trace_events: Vec::new(),
            span_events: Vec::new(),
        }
    }

    /// Current virtual clock tick.
    #[must_use]
    pub fn current_tick(&self) -> Tick {
        self.current_tick
    }

    /// Currently executing process PID, if any.
    #[must_use]
    pub fn running_pid(&self) -> Option<Pid> {
        self.running_pid
    }

    /// Check whether the kernel engine has halted.
    #[must_use]
    pub fn is_halted(&self) -> bool {
        self.is_halted
    }

    /// Read immutable reference to the process arena.
    #[must_use]
    pub fn arena(&self) -> &ProcessArena {
        &self.arena
    }

    /// Read active status summary.
    #[must_use]
    pub fn engine_status(&self) -> EngineStatus {
        let active_count = self
            .arena
            .iter()
            .filter(|p| !p.state.is_terminated())
            .count();

        EngineStatus {
            is_running: !self.is_halted,
            current_tick: self.current_tick,
            active_process_count: active_count,
        }
    }

    /// Take drained trace events collected during recent ticks.
    pub fn drain_trace_events(&mut self) -> Vec<TraceEvent> {
        std::mem::take(&mut self.trace_events)
    }

    /// Take drained Gantt span events collected during recent ticks.
    pub fn drain_span_events(&mut self) -> Vec<SpanEvent> {
        std::mem::take(&mut self.span_events)
    }

    /// Admit a new process into the simulation.
    pub fn spawn_process(
        &mut self,
        name: impl Into<String>,
        priority: nvos_core::process::PriorityLevel,
        tape: Vec<InstructionToken>,
    ) -> Pid {
        let pid = Pid(self.next_pid_counter);
        self.next_pid_counter = self.next_pid_counter.saturating_add(1);

        let pcb = ProcessControlBlock::new(pid, None, name, priority, self.current_tick, tape);
        self.arena.insert(pcb);
        self.pending_arrivals.push(pid);
        pid
    }

    /// Advance the simulation by exactly one discrete tick.
    ///
    /// Executes the Two-Phase Commit tick cycle safely within panic boundaries.
    pub fn tick(&mut self) -> Result<(), KernelEngineError> {
        if self.is_halted {
            return Ok(());
        }

        // =========================================================================
        // Phase 1: Passive Request Collection (Read-Only)
        // (In subsequent phases, nvos-io and nvos-mem will be polled here)
        // Note: Schedulers are NEVER called in Phase 1.
        // =========================================================================
        let requests = self.collect_phase1_requests();

        // =========================================================================
        // Phase 2: Canonical Arbitration & State Mutation
        // =========================================================================
        self.apply_phase2_mutations(requests)?;

        // Advance monotonic virtual clock
        self.current_tick = self.current_tick.next();
        Ok(())
    }

    fn collect_phase1_requests(&mut self) -> Vec<KernelRequest> {
        let mut requests = Vec::new();

        // Check sleeping processes against current tick
        for pcb in self.arena.iter() {
            if let ProcessState::Sleeping(wake_tick) = pcb.state {
                if self.current_tick >= wake_tick {
                    requests.push(KernelRequest::TimerExpired { pid: pcb.pid });
                }
            }
        }

        requests
    }

    fn apply_phase2_mutations(
        &mut self,
        requests: Vec<KernelRequest>,
    ) -> Result<(), KernelEngineError> {
        // Step 1: Process Arrivals (New -> Ready) & Wakeups (Sleeping -> Ready)
        let pending: Vec<Pid> = self.pending_arrivals.drain(..).collect();
        for pid in pending {
            if let Some(pcb) = self.arena.get_mut(pid) {
                pcb.state = ProcessState::Ready;
                let priority = pcb.priority;
                let sched = &mut self.scheduler;
                safe_subsystem_dispatch(sched.name(), || {
                    sched
                        .add_process(pid, priority)
                        .map_err(|e| KernelEngineError {
                            subsystem: "nvos-cpu",
                            kind: EngineErrorKind::SubsystemFault {
                                detail: e.to_string(),
                            },
                        })
                })?;
                self.record_state_change(pid, "New", "Ready");
            }
        }

        // Step 2: Process Wakeups & I/O / Swap completions
        for req in requests {
            if let KernelRequest::TimerExpired { pid } = req {
                if let Some(pcb) = self.arena.get_mut(pid) {
                    pcb.state = ProcessState::Ready;
                    let priority = pcb.priority;
                    let sched = &mut self.scheduler;
                    safe_subsystem_dispatch(sched.name(), || {
                        sched
                            .add_process(pid, priority)
                            .map_err(|e| KernelEngineError {
                                subsystem: "nvos-cpu",
                                kind: EngineErrorKind::SubsystemFault {
                                    detail: e.to_string(),
                                },
                            })
                    })?;
                    self.record_state_change(pid, "Sleeping", "Ready");
                }
            }
        }

        // Step 3 & 4: Step CPU Burst for Running PID (if any)
        let mut current_yielded = false;
        let mut current_terminated = false;

        if let Some(running_pid) = self.running_pid {
            if let Some(pcb) = self.arena.get_mut(running_pid) {
                pcb.cpu_time_consumed = pcb.cpu_time_consumed.saturating_add(1);

                match pcb.current_token().cloned() {
                    Some(InstructionToken::Cpu(burst_len)) => {
                        pcb.current_token_progress = pcb.current_token_progress.saturating_add(1);
                        if pcb.current_token_progress >= burst_len {
                            pcb.retire_current_token();
                            // Check next token immediately (e.g. Exit)
                            match pcb.current_token().cloned() {
                                Some(InstructionToken::Exit(code)) => {
                                    pcb.retire_current_token();
                                    pcb.state = ProcessState::Terminated(ExitReason::Normal(code));
                                    pcb.completion_tick = Some(self.current_tick);
                                    current_terminated = true;
                                    self.record_state_change(running_pid, "Running", "Terminated");
                                }
                                None => {
                                    // Tape exhausted -> implicit Exit(0)
                                    pcb.state = ProcessState::Terminated(ExitReason::Normal(0));
                                    pcb.completion_tick = Some(self.current_tick);
                                    current_terminated = true;
                                    self.record_state_change(running_pid, "Running", "Terminated");
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(InstructionToken::Sleep(duration)) => {
                        pcb.retire_current_token();
                        let wake_tick =
                            self.current_tick + nvos_core::tick::DurationTicks(u64::from(duration));
                        pcb.state = ProcessState::Sleeping(wake_tick);
                        current_yielded = true;
                        self.record_state_change(running_pid, "Running", "Sleeping");
                    }
                    Some(InstructionToken::Exit(code)) => {
                        pcb.retire_current_token();
                        pcb.state = ProcessState::Terminated(ExitReason::Normal(code));
                        pcb.completion_tick = Some(self.current_tick);
                        current_terminated = true;
                        self.record_state_change(running_pid, "Running", "Terminated");
                    }
                    Some(InstructionToken::FaultInject { target, error_code }) => {
                        pcb.retire_current_token();
                        let exit_reason = match target {
                            nvos_core::tape::FaultTarget::SegFault => ExitReason::SegmentationFault,
                            _ => ExitReason::Killed,
                        };
                        pcb.state = ProcessState::Terminated(exit_reason);
                        pcb.completion_tick = Some(self.current_tick);
                        current_terminated = true;
                        self.record_state_change(running_pid, "Running", "Terminated(Fault)");
                        self.trace_events.push(TraceEvent {
                            tick: self.current_tick,
                            pid: Some(running_pid),
                            kind: TraceEventKind::FaultInjected { error_code },
                        });
                    }
                    _ => {
                        // Placeholders for memory / filesystem tokens in Phase 2 & 3
                        pcb.retire_current_token();
                        if pcb.tape_exhausted() {
                            pcb.state = ProcessState::Terminated(ExitReason::Normal(0));
                            pcb.completion_tick = Some(self.current_tick);
                            current_terminated = true;
                            self.record_state_change(running_pid, "Running", "Terminated");
                        }
                    }
                }
            }
        }

        // Step 5: Unified Scheduling Arbitration (Single Authority)
        if current_terminated {
            if let Some(pid) = self.running_pid.take() {
                let sched = &mut self.scheduler;
                safe_subsystem_dispatch(sched.name(), || {
                    sched
                        .on_process_terminated(pid)
                        .map_err(|e| KernelEngineError {
                            subsystem: "nvos-cpu",
                            kind: EngineErrorKind::SubsystemFault {
                                detail: e.to_string(),
                            },
                        })
                })?;
            }
        } else if current_yielded {
            if let Some(pid) = self.running_pid.take() {
                let sched = &mut self.scheduler;
                safe_subsystem_dispatch(sched.name(), || {
                    sched
                        .on_process_blocked(
                            pid,
                            nvos_core::process::BlockReason::Io { device_id: 0 },
                        )
                        .map_err(|e| KernelEngineError {
                            subsystem: "nvos-cpu",
                            kind: EngineErrorKind::SubsystemFault {
                                detail: e.to_string(),
                            },
                        })
                })?;
            }
        }

        // Advance quantum tracking via tick()
        let sched = &mut self.scheduler;
        let action = safe_subsystem_dispatch(sched.name(), || {
            sched.tick(self.running_pid).map_err(|e| KernelEngineError {
                subsystem: "nvos-cpu",
                kind: EngineErrorKind::SubsystemFault {
                    detail: e.to_string(),
                },
            })
        })?;

        if action == SchedulingAction::Preempt {
            if let Some(preempted_pid) = self.running_pid.take() {
                if let Some(pcb) = self.arena.get_mut(preempted_pid) {
                    if pcb.state == ProcessState::Running {
                        pcb.state = ProcessState::Ready;
                        self.record_state_change(preempted_pid, "Running", "Ready");
                    }
                }
            }
        }

        // If CPU core is idle, dispatch next process
        if self.running_pid.is_none() {
            let current_tick = self.current_tick;
            let sched = &mut self.scheduler;
            let next_pid = safe_subsystem_dispatch(sched.name(), || {
                sched
                    .schedule_next(current_tick)
                    .map_err(|e| KernelEngineError {
                        subsystem: "nvos-cpu",
                        kind: EngineErrorKind::SubsystemFault {
                            detail: e.to_string(),
                        },
                    })
            })?;

            if let Some(pid) = next_pid {
                if let Some(pcb) = self.arena.get_mut(pid) {
                    pcb.state = ProcessState::Running;
                    self.running_pid = Some(pid);
                    self.record_state_change(pid, "Ready", "Running");
                }
            }
        }

        // Step 6: Commit telemetry snapshot
        self.trace_events.push(TraceEvent {
            tick: self.current_tick,
            pid: self.running_pid,
            kind: TraceEventKind::SchedulerDecision {
                action: format!("{action:?}"),
            },
        });

        Ok(())
    }

    fn record_state_change(&mut self, pid: Pid, from: &str, to: &str) {
        self.trace_events.push(TraceEvent {
            tick: self.current_tick,
            pid: Some(pid),
            kind: TraceEventKind::StateTransition {
                from: from.to_string(),
                to: to.to_string(),
            },
        });

        self.span_events.push(SpanEvent {
            pid,
            state_label: to.to_string(),
            start_tick: self.current_tick,
            end_tick: None,
        });
    }
}
