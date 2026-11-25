# Stage 9 Implementation Readiness Report: `docs/designs/nvos_simulator.md`

* **Target:** `docs/designs/nvos_simulator.md` (`lines 362..456`)  
* **Mode:** `STAGE_9_IMPLEMENTATION_READINESS`  
* **Reviewers:** `egm-implementation-readiness`, `egm-codebase-details`, `egm-style`  
* **Context Audited:** `concepts_brief.md`, `zoom_out_report.md`, `comprehension_report.md`, `critic_report.md`, `intermediate_zoom_out_report.md`  
* **Date:** 2026-11-18

---

## Reviewer Status

| Reviewer | Status | Focus Area |
| :--- | :--- | :--- |
| `egm-implementation-readiness` | completed | File enumeration completeness, target compilation, panic/error infrastructure, test suite gaps |
| `egm-codebase-details` | completed | Crate dependency graph, Cargo workspace manifests, type coupling, async channel orchestration |
| `egm-style` | completed | Path precision, structural naming consistency, specification hygiene |

---

## Executive Summary & Severity Breakdown

A rigorous line-by-line audit of the **Detailed Implementation Plan (Section 7, lines 362–456)** against the architectural requirements from Stages 5–8 reveals that while the sequenced milestone progression (Phases 0–6) provides a solid skeleton, it contains **three implementation-blocking defects** and **four major technical gaps** that would cause immediate compiler failures, runtime crashes, and testing inadequacies if an implementer began writing code today.

* 🔴 **Blocker (3 findings):**
  1. `nvos-web` target compilation deadlock and missing static asset packaging.
  2. Total omission of panic boundary (`catch_unwind`) and error hierarchy infrastructure.
  3. Undeclared crate dependencies in `nvos-kernel` manifest for memory and I/O adapters.
* 🟡 **Major (4 findings):**
  4. `nvos-core` zero-dependency paradox vs. `slotmap::Key` and `serde` serialization.
  5. Omission of async kernel worker/runner (`runner.rs`) and telemetry channel bridging.
  6. Test suite deficits: complete absence of property-based (`proptest`) and mutation testing (`cargo-mutants`).
  7. Multi-tick token resumption tracking and hardware chaos tokens omitted from enumerated files.
* 🟢 **Nit (1 finding):**
  8. Missing web frontend entry files (`index.html`, Dioxus/Trunk assets).

---

## Prioritized Findings (Blockers -> Major -> Nits)

### 1. `nvos-web` Dual-Target Compilation Deadlock & Missing Asset Packaging - Severity: 🔴 Blocker
* **Reviewer Lens:** `egm-implementation-readiness` / `egm-codebase-details`
* **Affected Scope:** `docs/designs/nvos_simulator.md#L431-L440`, `L449-L454`
* **Discrepancy Description:**
  In Phase 5, crate `./crates/nvos-web/Cargo.toml` specifies dependencies on `dioxus-web` (which targets `wasm32-unknown-unknown` and browser DOM APIs) alongside `axum`, `tokio`, and `tower-http` (which require native OS network sockets) within a single flat manifest. The file tree places `src/server.rs` (native Axum server) and `src/app.rs` / `src/components/gantt.rs` (Dioxus web client) in the exact same crate root without target-specific dependency gating or feature flags.
  Furthermore, in Phase 6, `./crates/nvos-cli/Cargo.toml` depends on `nvos-web` to support `nvos web <scenario.toml> --port <port>`.
  Under standard Cargo builds, compiling `dioxus-web` on native desktop platforms will trigger immediate target compilation errors. Conversely, attempting to compile the Axum server for browser WASM fails. Additionally, the plan does not specify how the Axum server serves the compiled Dioxus client assets (e.g. via `rust-embed`, `include_dir!`, or Dioxus Fullstack server functions).
* **Rationale & Recommendation:**
  1. **Split Target Dependencies in `nvos-web/Cargo.toml`:**
     ```toml
     [dependencies]
     nvos-core = { workspace = true, features = ["serde"] }
     serde = { workspace = true }
     
     [target.'cfg(not(target_arch = "wasm32"))'.dependencies]
     axum = { workspace = true }
     tokio = { workspace = true, features = ["full"] }
     tower-http = { workspace = true, features = ["fs", "cors"] }
     rust-embed = "8.5"
     
     [target.'cfg(target_arch = "wasm32")'.dependencies]
     dioxus = { workspace = true }
     dioxus-web = { workspace = true }
     ```
  2. **Enumerate Server Asset Embedding:** Add `./crates/nvos-web/src/assets.rs` to embed precompiled WASM/HTML/CSS assets into the native Axum binary, or configure `dioxus fullstack` if targeting unified builds.

---

### 2. Total Omission of Panic Boundary & Unified Error Hierarchy Infrastructure - Severity: 🔴 Blocker
* **Reviewer Lens:** `egm-implementation-readiness` / `egm-codebase-details`
* **Affected Scope:** `docs/designs/nvos_simulator.md#L366-L389`, `L414`
* **Discrepancy Description:**
  Stages 5, 7, and 8 explicitly mandated:
  - Transitioning subsystem and scheduler trait methods from infallible `()` returns to `Result<T, E>`.
  - Establishing a two-tier error hierarchy (recoverable guest faults vs unrecoverable engine errors).
  - Wrapping student/user policy invocations inside an explicit Panic Isolation Boundary (`std::panic::catch_unwind`) to prevent student bugs from crashing the host Tokio runtime and presentation servers.
  However, Section 7 enumerates **zero** files for error or panic management in Phase 0 and Phase 1:
  - `nvos-core` lists only `lib.rs`, `tick.rs`, `process.rs`, `tape.rs`, `events.rs`.
  - `nvos-kernel` lists only `lib.rs`, `arena.rs`, `engine.rs`.
  Without an explicit `error.rs` and `panic_boundary.rs`, an implementer will implement infallible traits or permit raw unwinding panics, directly violating the core architectural robustness mandate.
* **Rationale & Recommendation:**
  1. **Add `crates/nvos-core/src/error.rs` in Phase 0:** Define `SimulationError`, `KernelError`, and guest trap enums (`TrapReason`, `FaultKind`).
  2. **Add `crates/nvos-kernel/src/panic_boundary.rs` in Phase 1:** Implement the panic isolation wrapper:
     ```rust
     pub fn safe_subsystem_dispatch<F, R>(subsystem_name: &'static str, f: F) -> Result<R, SimulationError>
     where F: FnOnce() -> Result<R, SimulationError> + std::panic::UnwindSafe;
     ```
     Transition the simulated kernel into a deterministic `SimulatedKernelPanic` diagnostic snapshot upon caught unwind without killing the host Tokio process.

---

### 3. Dependency Graph Incoherence & Undeclared Subsystem Dependencies in `nvos-kernel` - Severity: 🔴 Blocker
* **Reviewer Lens:** `egm-codebase-details`
* **Affected Scope:** `docs/designs/nvos_simulator.md#L384`, `L399`, `L414`
* **Discrepancy Description:**
  In Phase 1 (line 384), `./crates/nvos-kernel/Cargo.toml` is defined with dependencies strictly on `nvos-core`, `nvos-cpu`, and `slotmap`.
  However, in Phase 2 (line 399), the plan introduces `./crates/nvos-kernel/src/memory_bridge.rs` ("Kernel adapter integrating `nvos-mem` into Phase 1 memory evaluation and Phase 2 page fault handling").
  Then, in Phase 3 (line 414), it introduces `./crates/nvos-kernel/src/io_bridge.rs` ("Kernel adapter dispatching device interrupts and managing process transitions to and from `BlockedIo`").
  The manifest for `nvos-kernel` is never updated in Phase 2 or Phase 3 to declare dependencies on `nvos-mem`, `nvos-io`, or `nvos-fs`. If an engineer implements `memory_bridge.rs` and `io_bridge.rs`, `nvos-kernel` will fail compilation due to unresolved extern crates.
* **Rationale & Recommendation:**
  Explicitly update `./crates/nvos-kernel/Cargo.toml` in Phase 2 and Phase 3:
  - Phase 2: Add `nvos-mem` dependency to `nvos-kernel/Cargo.toml`.
  - Phase 3: Add `nvos-io` and `nvos-fs` dependencies to `nvos-kernel/Cargo.toml`.
  Alternatively, declare optional subsystem dependencies with feature flags (`features = ["mem", "io", "fs"]`) in `nvos-kernel/Cargo.toml`.

---

### 4. `nvos-core` Zero-Dependency Paradox vs. `slotmap::Key` and `serde` Serialization - Severity: 🟡 Major
* **Reviewer Lens:** `egm-codebase-details`
* **Affected Scope:** `docs/designs/nvos_simulator.md#L367-L374`, `L386`, `L417-L421`
* **Discrepancy Description:**
  - In Phase 0 (line 368), `./crates/nvos-core/Cargo.toml` is described as a "foundational zero-dependency" crate implementing `Pid` and `ProcessControlBlock`.
  - In Phase 1 (line 386), `./crates/nvos-kernel/src/arena.rs` specifies `slotmap::DenseSlotMap<Pid, ProcessControlBlock>`.
  In the `slotmap` crate, any type used as a key in `DenseSlotMap<K, V>` must implement the `slotmap::Key` trait. If `nvos-core` has zero external dependencies, `Pid` cannot implement `slotmap::Key`.
  - Furthermore, `ProcessControlBlock`, `ProcessState`, and `Tick` must be serialized across the wire to `nvos-trace`, `nvos-tui`, and `nvos-web`. If `nvos-core` has no dependency on `serde`, either `nvos-core` cannot derive `Serialize, Deserialize`, or telemetry requires maintaining redundant mirror DTO types.
* **Rationale & Recommendation:**
  Resolve this architectural contract:
  - Add optional feature flags to `crates/nvos-core/Cargo.toml`:
    ```toml
    [dependencies]
    serde = { workspace = true, optional = true }
    slotmap = { workspace = true, optional = true }

    [features]
    default = []
    serde = ["dep:serde"]
    slotmap = ["dep:slotmap"]
    ```
  - Alternatively, define `Pid(pub u32)` in `nvos-core` with no external dependencies, and in `nvos-kernel/src/arena.rs`, define an internal key type (`slotmap::new_key_type! { struct ProcessKey; }`) paired with a bidirectional lookup mapping `Pid <-> ProcessKey`.

---

### 5. Omission of Async Kernel Worker (`runner.rs`) & Telemetry Channel Bridging - Severity: 🟡 Major
* **Reviewer Lens:** `egm-implementation-readiness`
* **Affected Scope:** `docs/designs/nvos_simulator.md#L385-L387`, `L422-L430`, `L434-L440`
* **Discrepancy Description:**
  The core simulation loop (`crates/nvos-kernel/src/engine.rs`) is a synchronous, discrete two-phase commit state machine. However, the system architecture dictates that the simulator runs inside an asynchronous background Tokio task, responding to play/pause/step commands and emitting snapshots over channels to `nvos-tui` and `nvos-web`.
  Section 7 enumerates no file responsible for this runtime boundary:
  - `nvos-kernel` contains only `lib.rs`, `arena.rs`, and `engine.rs`.
  - `nvos-tui` and `nvos-web` have no common entry point or coordinator to instantiate, spawn, and drive the engine task.
* **Rationale & Recommendation:**
  Add `./crates/nvos-kernel/src/runner.rs` in Phase 1:
  - Implement `SimulationRunner` which wraps `KernelEngine`, runs the tick loop inside `tokio::spawn`, manages step/pause/speed commands via `tokio::sync::mpsc::Receiver<EngineCommand>`, and publishes engine status via `tokio::sync::watch` and batch state transitions via bounded channels or ring buffer pulls.

---

### 6. Test Suite Deficits: Absence of Property-Based and Mutation Testing Harnesses - Severity: 🟡 Major
* **Reviewer Lens:** `egm-implementation-readiness`
* **Affected Scope:** `docs/designs/nvos_simulator.md#L367`, `L374`, `L383`, `L397-L398`, `L455`
* **Discrepancy Description:**
  Section 7 enumerates basic unit and benchmark tests (`pcb_tests.rs`, `scheduler_tests.rs`, `paging_tests.rs`, `heap_tests.rs`, `io_tests.rs`, `e2e_reproducibility_test.rs`).
  However, per Stage 5 and Stage 8 mandates:
  1. Discrete-event OS invariant testing requires **Property-Based Testing** (e.g. via `proptest`) to prove invariants across randomized burst tapes and interleaved arrivals (e.g. MLFQ starvation boost guarantees, conservation of total physical frames: `free_frames + allocated_frames == total_frames`).
  2. For automated evaluation and grading of student algorithms, **Mutation Testing** (via `cargo-mutants`) is required to guarantee test suites catch deliberate policy mutations.
  Neither `proptest` in `Cargo.toml` nor any property test files or mutation configs (`mutants.toml`) are enumerated in Section 7.
* **Rationale & Recommendation:**
  1. Add `proptest` to workspace dev-dependencies in `./Cargo.toml`.
  2. Enumerate explicit property test targets:
     - `./crates/nvos-cpu/tests/scheduler_proptests.rs`: Generates arbitrary arrival and burst distributions to prove zero starvation and quantum bound compliance.
     - `./crates/nvos-mem/tests/allocator_proptests.rs`: Generates arbitrary allocate/free sequences to verify no block overlaps and 100% frame conservation.
  3. Enumerate `./mutants.toml` in Phase 6: Configures `cargo-mutants` filters to exclude mock boilerplate and target core scheduling and page replacement algorithms.

---

### 7. Multi-Tick Token Resumption Tracking & Omission of Hardware Chaos Tape Tokens - Severity: 🟡 Major
* **Reviewer Lens:** `egm-implementation-readiness` / `egm-style`
* **Affected Scope:** `docs/designs/nvos_simulator.md#L371-L373`, `L411-L413`
* **Discrepancy Description:**
  1. `ProcessControlBlock` specification in Phase 0 (`process.rs`) does not enumerate `current_token_progress: u32`. Without this field, when a process executing a multi-tick instruction (e.g., `InstructionToken::Cpu(5)`) is preempted after 2 ticks, the kernel cannot resume the remaining 3 ticks upon rescheduling.
  2. Line 371 specifies "`ProcessState` enum (7 lifecycle states)", but the 7 states are not explicitly enumerated. As noted in Stage 7, if `InstructionToken::Sleep(u32)` is supported, `ProcessState::Sleeping(Tick)` or `BlockedSleep` must be enumerated, alongside an explicit `Terminated(ExitReason)` state.
  3. Phase 0 (`tape.rs`) and Phase 3 (`disk_controller.rs`) omit deterministic fault-injection primitives (e.g., `InstructionToken::FaultInject` or simulated controller media errors) required for chaos testing.
* **Rationale & Recommendation:**
  - In `crates/nvos-core/src/process.rs`, explicitly document `ProcessControlBlock` fields: `pub current_token_progress: u32` and `pub quantum_remaining: u32`.
  - Explicitly enumerate the 7 states: `New`, `Ready`, `Running`, `BlockedIo`, `BlockedPageFault`, `Sleeping`, `Terminated(ExitReason)`.
  - In `crates/nvos-core/src/tape.rs`, add `InstructionToken::FaultInject { target: FaultTarget, error_code: u32 }`.

---

### 8. Missing Web Frontend Scaffolding Files (`index.html`, Trunk/Dioxus Config) - Severity: 🟢 Nit
* **Reviewer Lens:** `egm-style` / `egm-implementation-readiness`
* **Affected Scope:** `docs/designs/nvos_simulator.md#L431-L440`
* **Discrepancy Description:**
  Phase 5 enumerates Rust source files for `nvos-web`, but completely omits the client web scaffolding: `./crates/nvos-web/index.html` (HTML shell, font links, canvas container), `./crates/nvos-web/Dioxus.toml` (or `Trunk.toml`), and static CSS styling for the Gantt chart and memory grid.
* **Rationale & Recommendation:**
  Add `./crates/nvos-web/index.html`, `./crates/nvos-web/style.css`, and `./crates/nvos-web/Dioxus.toml` to the enumerated file list of Phase 5.
