# Subsystem Specification 02: CPU Schedulers & Algorithmic Verification

* **Module:** `nvos-cpu`
* **Parent Architecture:** [nvos_simulator.md](../nvos_simulator.md)
* **Status:** Approved Specification

---

## 1. Executive Responsibilities

The `nvos-cpu` crate isolates CPU scheduling policies behind a pure Rust trait contract:
1. **Zero-Monolith Coupling:** Schedulers operate strictly on `Pid` and `PriorityLevel` abstractions without inspecting memory tables or file descriptors.
2. **Fallible & Resilient Interface:** Trait methods return `Result` types, preventing silent corruption while integrating with the kernel's panic boundary.
3. **Pluggable Policy Implementations:** Provides benchmarked implementations of classical scheduling algorithms: FCFS, Round Robin (preemptive), Shortest Job First / Shortest Remaining Time First (SJF/SRTF), and Multi-Level Feedback Queue (MLFQ).
4. **Property & Mutation Testing Verification:** System invariants (starvation bounds, turnaround accuracy) are verified using randomized property fuzzing (`proptest`) and deliberate code mutation testing (`cargo-mutants`).

---

## 2. Core Trait Contract

```rust
use nvos_core::process::{Pid, PriorityLevel};
use nvos_core::tick::Tick;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingAction {
    /// Keep current process running on the CPU core.
    Continue,
    /// Current process voluntary yields the CPU (e.g., entered I/O or sleep).
    Yield,
    /// Current process is preempted (e.g., quantum expired or higher priority process arrived).
    Preempt,
}

#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("Process {0:?} already enrolled in scheduler")]
    DuplicateProcess(Pid),
    #[error("Process {0:?} not found in ready queues")]
    ProcessNotFound(Pid),
    #[error("Internal scheduler queue corruption: {0}")]
    InternalCorruption(String),
}

pub trait Scheduler: Send + Sync {
    /// Name of the scheduling policy (e.g., "MLFQ", "RoundRobin-Q4").
    fn name(&self) -> &'static str;

    /// Enroll a newly arrived or awakened process into the ready queue.
    fn add_process(&mut self, pid: Pid, priority: PriorityLevel) -> Result<(), SchedulerError>;

    /// Select the next process to execute on the CPU core.
    fn schedule_next(&mut self, current_tick: Tick) -> Result<Option<Pid>, SchedulerError>;

    /// Advance quantum tracking for the currently running process at each tick.
    fn tick(&mut self, running_pid: Option<Pid>) -> Result<SchedulingAction, SchedulerError>;

    /// Notify scheduler that a process has blocked (I/O, page fault, sleep).
    fn on_process_blocked(&mut self, pid: Pid) -> Result<(), SchedulerError>;

    /// Notify scheduler that a process has terminated.
    fn on_process_terminated(&mut self, pid: Pid) -> Result<(), SchedulerError>;
}
```

### Architectural Simplification: No Dispatch Payload in `Preempt`
`SchedulingAction::Preempt` is a clean signal without an embedded `Option<Pid>` payload. When `tick()` returns `Preempt`, the kernel re-evaluates queues and invokes `schedule_next(current_tick)` directly, eliminating dual-dispatch authority.

---

## 3. Implemented Scheduling Algorithms

### 3.1. First-Come, First-Served (FCFS)
* **Data Structure:** `VecDeque<Pid>` maintained strictly by arrival tick.
* **Preemption:** Non-preemptive. `tick()` always returns `Continue` until the process yields, blocks, or exits.
* **Complexity:** $O(1)$ dispatch and enqueue.

### 3.2. Round Robin (RR)
* **Configuration:** `quantum: u32` (default: 4 ticks).
* **Mechanism:** Tracks `quantum_used: u32` for the running process.
* **Preemption:** When `quantum_used >= quantum`, returns `SchedulingAction::Preempt`, moving the process to the tail of the FIFO ready queue.
* **Resumption:** Processes preempted mid-token retain their `current_token_progress` in their PCB, resuming execution seamlessly on subsequent turns.

### 3.3. Shortest Job First / Shortest Remaining Time First (SJF / SRTF)
* **Mechanism:** Maintains priority queue sorted by estimated remaining CPU burst time.
* **Preemption:** When a newly awakened process arrives with a remaining burst shorter than the currently running process's remaining burst, triggers `SchedulingAction::Preempt`.

### 3.4. Multi-Level Feedback Queue (MLFQ)
Modeled after standard OS literature (Remzi OSTEP & Tanenbaum):
* **Queues:** $N$ distinct priority queues ($Q_0$ highest to $Q_{N-1}$ lowest).
* **Rule 1:** If $\text{Priority}(A) > \text{Priority}(B)$, process $A$ runs.
* **Rule 2:** If $\text{Priority}(A) == \text{Priority}(B)$, processes run in Round Robin using $Q_i$'s time quantum.
* **Rule 3:** When a process enters the system, it is placed at highest priority ($Q_0$).
* **Rule 4 (Accounting & Demotion):** Once a process uses its time allotment at a given level (regardless of how many times it yields for I/O), its priority is reduced by one level ($Q_i \rightarrow Q_{i+1}$). This prevents CPU gaming.
* **Rule 5 (Starvation Priority Boost):** After a configurable period $S$ ticks (e.g., $S = 500$), all processes in the system are moved to the highest queue $Q_0$.

---

## 4. Verification Harness: Property & Mutation Testing

### 4.1. Property-Based Testing (`crates/nvos-cpu/tests/scheduler_proptests.rs`)
Using `proptest`, we generate randomized process arrival sequences, variable burst demands, and random quantum values:

* **Property 1 (No Starvation under MLFQ):**
  $$\forall p \in \text{Processes}, \quad \text{WaitTime}(p, \text{window}) \le S + (N \times \text{quantum}_{\max})$$
  Asserts that no process is starved indefinitely when periodic priority boost is active.
* **Property 2 (Queue Conservation Invariant):**
  At any tick $T$:
  $$\sum \text{len}(Q_i) + \mathbb{I}(\text{running}) == \text{total\_active\_pids}$$
  No PID is duplicated or dropped across preemption and demotion transitions.

### 4.2. Mutation Testing (`mutants.toml`)
Configured to ensure test suites catch deliberate operator mutations in `nvos-cpu`:
```toml
# mutants.toml
[filter]
exclude_files = ["tests/*", "benches/*"]
include_files = ["crates/nvos-cpu/src/*"]

[mutations]
# Tests must catch mutated comparison operators (< to <=, > to >=)
# in MLFQ demotion thresholds and quantum expiration.
```
