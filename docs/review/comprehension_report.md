# Stage 6 Comprehension Test Audit: `docs/designs/nvos_simulator.md`

## Feature Spec Paraphrase

### What this proposes
The document proposes `nvos`, a deterministic, pure-Rust discrete-event operating system simulator designed for educational and research evaluation of core OS concepts without the non-determinism of host OS threads or physical timers. The system executes synthetic instruction tapes against decoupled subsystems—CPU scheduling, dual-layer memory management, mock inode filesystem, and virtual I/O devices—governed by a discrete virtual clock (`Tick(u64)`). To eliminate circular event cascades and race conditions within a single tick, the kernel coordinates execution using a two-phase tick cycle (gathering subsystem requests in Phase 1 and arbitrating mutations in Phase 2). The system exposes execution state through structured telemetry ring buffers, feeding both a high-density terminal UI (`ratatui`) and a pure-Rust web dashboard (`Dioxus`) featuring an interactive Gantt chart.

### How the existing system works, per this document
According to the document, existing educational operating systems (such as xv6 or Pintos) and traditional simulators suffer from tight monolithic entanglement, where testing an isolated scheduler or memory policy requires running an entire kernel stack. Furthermore, existing simulators that rely on host concurrency primitives (threads and wall-clock timers) suffer from non-deterministic scheduling jitter, rendering edge cases and student submissions unreproducible. Finally, observability in current tooling is limited to unstructured stdout text logs or fragmented multi-language toolchains (e.g., C kernels paired with JavaScript/Python frontends) requiring FFI and complex runtime dependencies.

---

## Detailed Implementation Paraphrase

The detailed implementation plan organizes the development of `nvos` into seven progressive milestones within an 11-crate Cargo virtual workspace. Phases 0 through 3 scaffold the core domain models, implement the discrete-event kernel with pluggable CPU schedulers (FCFS, RR, SJF, MLFQ), build dual-layer memory management (paging with TLB/replacement policies and physical heap allocators), and construct an in-memory mock inode filesystem alongside virtual timer and disk controller devices. Phases 4 through 6 complete the user-facing and integration layers by implementing a bounded ring-buffer telemetry engine, terminal (`nvos-tui`) and web (`nvos-web`) dashboards with dynamic Gantt visualizations, declarative TOML workload scenarios, and a unified CLI binary verified by an end-to-end bit-for-bit reproducibility test suite.

---

## Comprehension Audit Findings

### 1. Dual-Target Compilation Ambiguity in `nvos-web` (Client WASM vs. Host Server)
* **Severity**: Major
* **Affected Scope**: Section 5.5 (Line 316) & Section 7 Phase 5 (Lines 432–434)
* **Discrepancy Description & Rationale**:
  In Phase 5, crate `nvos-web` specifies dependencies on both `dioxus-web` (which targets browser WebAssembly via `wasm32-unknown-unknown`) and `axum` / `tokio` / `tower-http` (which target native host platforms) within a single `Cargo.toml`. The crate file tree places `src/server.rs` (Axum server and WebSocket broadcaster) and `src/app.rs` / `src/components/gantt.rs` (client Dioxus UI components) in the exact same crate.
  A naive reader or implementer attempting to build this crate will encounter target architecture conflicts: `axum` and `tokio` cannot compile to `wasm32-unknown-unknown` without specialized configurations, and `dioxus-web` cannot run directly in a native desktop/server binary. The document does not explain how this crate is built—whether it utilizes Dioxus Fullstack server-function features, conditional target compilation (`[target.'cfg(...)'.dependencies]`), or separate binaries/crates for client and server.

### 2. Ambiguity of "Time-Travel Debugging" / "Step-Back" Execution Model
* **Severity**: Major
* **Affected Scope**: Section 2 (Line 25), Section 5.5 (Lines 310–312), Section 6.6 (Line 358), Section 7 Phase 5 (Line 439)
* **Discrepancy Description & Rationale**:
  The document repeatedly references "time-travel debugging" and specifies a "step-back" button on the UI controls communicating back to the simulation worker task. However, the telemetry pipeline only specifies that `nvos-trace` stores recent `Snapshot` records in a sliding ring buffer for rendering and logging.
  The document does not clarify whether "step-back" allows rewinding the live simulation engine state (which would require restoring the full process arena, page tables, disk block states, and scheduler queues from a checkpoint snapshot), or if "time-travel" is strictly a read-only viewer mode that scrubs over past telemetry snapshots while the simulator core remains paused at its highest tick.

### 3. `nvos-core` Zero-Dependency Mandate vs. `slotmap::DenseSlotMap<Pid, PCB>` Key Typing
* **Severity**: Nit
* **Affected Scope**: Section 7 Phase 0 (Lines 368, 371) vs. Phase 1 (Line 386)
* **Discrepancy Description & Rationale**:
  Phase 0 explicitly specifies `nvos-core` as a "foundational zero-dependency" crate defining the `Pid` type. However, Phase 1 specifies that `nvos-kernel` manages processes using `slotmap::DenseSlotMap<Pid, ProcessControlBlock>`.
  In the `slotmap` crate, keys passed to `DenseSlotMap<K, V>` must implement the `slotmap::Key` trait (typically generated via the `new_key_type!` macro). If `nvos-core` has zero external dependencies, `Pid` cannot implement `slotmap::Key` without adding `slotmap` as a dependency to `nvos-core`, or by introducing an internal kernel slotmap key wrapper distinct from `Pid`. This is an unstated type relationship gap.

### 4. Undefined Acronym: DMA
* **Severity**: Nit
* **Affected Scope**: Section 3 (Line 38) & Section 7 Phase 3 (Lines 410, 412)
* **Discrepancy Description & Rationale**:
  The acronym "DMA" is referenced multiple times ("DMA Simulation", "DMA channel contracts", "DMA block transfer completion interrupts") without expansion or definition. For a reader without prior hardware architecture background, expanding this term to "Direct Memory Access (DMA)" and briefly noting how it transfers blocks without CPU intervention is necessary for complete self-containment.

### 5. Delayed Expansion of Acronym: TLB
* **Severity**: Nit
* **Affected Scope**: Section 5.2.2 (Line 145), Section 5.2.3 (Line 162), Section 7 Phase 2 (Line 394)
* **Discrepancy Description & Rationale**:
  The acronym "TLB" appears in the process state transition diagram (`TLB Miss & Page Fault`) and instruction token descriptions (`MemoryRead triggers TLB/Paging lookup`) in Section 5, but is not expanded until line 394 in Section 7 Phase 2 ("software Translation Lookaside Buffer (TLB)"). Acronyms should be defined on their first occurrence in the document.

### 6. Semantic Overload: "Two-Phase Commit"
* **Severity**: Nit
* **Affected Scope**: Section 2 (Line 24), Section 5.1 (Lines 58–93), Section 7 Phase 1 (Line 387)
* **Discrepancy Description & Rationale**:
  The document labels its tick cycle a "Two-Phase Commit" (Phase 1: Request Collection, Phase 2: Canonical Arbitration & State Mutation). In systems literature, Two-Phase Commit (2PC) specifically denotes distributed atomic consensus across autonomous network nodes. In `nvos`, it describes a synchronous gather-scatter tick loop used to eliminate intra-tick reentrancy. Adding a brief parenthetical note clarifying this distinction will avoid misleading readers expecting distributed transaction mechanics.
