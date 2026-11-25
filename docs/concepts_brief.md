# Deterministic Discrete-Event OS Simulator (nvos)

* **Author:** cristiano-s-r-filho
* **Date:** 2026-11-18

## Overview

`nvos` is a modular Rust workspace that implements a deterministic, discrete-event operating system simulator. It models core operating system abstractions derived from standard literature (notably Andrew S. Tanenbaum's *Modern Operating Systems*): CPU scheduling algorithms, dual-layer memory management (virtual paging and contiguous physical allocation), a mock inode-based filesystem, and interrupt/DMA-driven I/O devices. 

The simulator executes deterministically using a tick-based virtual clock (`Tick(u64)`) and an event queue rather than real-time OS threads, guaranteeing reproducible execution traces. Subsystems are decoupled behind event-driven interfaces and procedural adapters, enabling them to be executed both in isolation for targeted unit testing and composed together in an integrated kernel simulation loop. 

Concurrency is managed via an asynchronous runtime (`tokio`), decoupling the core discrete-event tick engine from the presentation layers. Telemetry is emitted as structured trace events bounded by a configurable sliding window for live inspection. The simulator provides dual primary interfaces:
1. An interactive **Terminal User Interface (TUI)** powered by `ratatui` for fast local terminal inspection and execution stepping.
2. A dedicated **Web/Desktop Interface (`nvos-web`)** built with **Dioxus**, providing a pure-Rust visual dashboard featuring WebSocket state streaming, interactive step/pause controls, live PCB and physical memory views, and an interactive **Gantt Chart** plotting process states and CPU burst execution across discrete tick timestamps.

## Analysis

The multi-lens evaluation identified several architectural constraints, trade-offs, and design resolutions across the system:

### 1. Architectural Boundaries & Determinism
* **Event Dispatch vs. Fixed-Tick Stepping:** A fixed tick-by-tick step loop wastes computation when the system is idle or awaiting long-latency I/O operations (such as simulated disk seek times). Conversely, a pure event-skip scheduler can bypass granular preemptive scheduling points. The engine adopts a hybrid approach: advancing tick-by-tick during active CPU burst slices, while advancing directly to the next scheduled event timestamp when the CPU is idle.
* **Deterministic Guarantees:** Standard Rust structures like `std::collections::HashMap` use non-deterministic SIP hashing, which produces varying iteration order across runs. All internal state storage uses deterministic alternatives (`BTreeMap` or seedable `fxhash`), and simulated latencies (e.g. disk head seek or I/O jitter) rely strictly on seedable pseudo-random generators (`rand_chacha::ChaCha8Rng`).

### 2. Coupling and State Management (Anti-Pattern Prevention)
* **Avoiding Shared Mutable State:** Distributing `Arc<Mutex<PCB>>` references across subsystems creates lock contention, non-deterministic lock acquisition orders, and circular ownership. `nvos` centralizes Process Control Block ownership inside a generational arena (`slotmap` / `generational-arena`) owned solely by the kernel coordinator. Subsystems interact via immutable references, process IDs (`Pid`), and explicit command buffers.
* **Two-Phase Commit Tick Execution:** To prevent cascading circular dependencies within a single discrete tick (e.g., page fault triggering a disk request, which triggers an interrupt, which modifies scheduling), execution follows a strict two-phase commit:
  1. *Collection Phase:* Subsystems evaluate current state and emit requests (`KernelRequest`).
  2. *Arbitration Phase:* The kernel arbitrates requests in canonical order (Clock -> I/O Interrupts -> Memory Faults -> Scheduling -> CPU Burst) and applies state mutations.

### 3. Memory Hierarchy Fidelity
* Physical memory is represented as a contiguous byte frame store (`Box<[u8]>` or frame bitmap).
* Virtual memory incorporates a two-level hierarchical page table and a software-managed Translation Lookaside Buffer (TLB) cache with configurable entries (e.g., 4, 8, 16) to explicitly capture TLB hit/miss penalties before physical frame lookup.

### 4. Telemetry and Presentation Scaling
* **Configurable Sliding Window & Tiered Telemetry:** Emitting unbounded tick events exhausts memory. The simulator implements a configurable ring buffer (e.g., sliding window of $N = 1000$ ticks) for live TUI and Web rendering and time-travel step-back inspection. Comprehensive execution history is periodically flushed as compacted telemetry checkpoints to disk.
* **Async Decoupling:** The simulation engine runs inside an asynchronous background task (`tokio`), communicating with presentation adapters through non-blocking broadcast channels (`tokio::sync::watch`). This ensures both the terminal UI and web WebSocket server operate smoothly without stalling the simulation tick thread.

### 5. Dioxus Frontend & Gantt Visualization Architecture (`nvos-web`)
* **Technology Choice:** `nvos-web` utilizes **Dioxus**, adhering strictly to the pure-Rust paradigm without requiring JavaScript frameworks. Dioxus compiles directly to WebAssembly (`wasm32-unknown-unknown`) for browser deployment or native desktop targets via `dioxus-desktop`.
* **State Synchronization:** The Dioxus UI connects via WebSocket to the `nvos` runner (or embeds the engine via WASM). Signals (`use_signal`) bind directly to tick state frames.
* **Gantt Chart Implementation:** Implemented as a declarative SVG/Canvas Dioxus component. The Gantt chart plots time on the horizontal axis (`Tick(u64)`) against Process IDs (`Pid`) on the vertical lanes. Each slice represents a contiguous execution state (`Running`, `Ready`, `Waiting/IO`, `Blocked/PageFault`, `Terminated`). Interactive zooming and scrubbing synchronize with the simulator's tick position.

## Goals vs. Non-Goals

### In-Scope Goals
* **Deterministic Discrete-Event Engine:** Reproducible tick-based progression with zero external non-determinism.
* **Tanenbaum-Aligned PCB:** Process Control Block tracking PID, PPID, registers, state machine, CPU accounting, priority, memory limits, and file descriptor tables.
* **Multiple Scheduling Algorithms:** Pluggable implementations of FCFS, Round Robin (preemptive), Shortest Job First (SJF/SRTF), and Multi-Level Feedback Queue (MLFQ).
* **Dual-Layer Memory Management:** Paging with page replacement policies (FIFO, LRU, Clock) alongside contiguous physical heap allocation policies (First-Fit, Best-Fit, Buddy Allocator).
* **Mock Inode Filesystem:** In-memory block structure with superblocks, inode allocation tables, direct/indirect block addressing, and file metadata.
* **Interrupt and I/O Device Simulation:** Mock timer, storage controller, and generic I/O devices emitting completion interrupts and DMA transfers with simulated tick latencies.
* **Async Runtime Layer:** `tokio`-based task separation between simulation stepping, TUI event loop, and web server streaming.
* **Sliding Window Telemetry:** Configurable ring buffer for recent tick states, paired with streaming disk persistence.
* **Interactive Ratatui TUI:** Multi-pane terminal dashboard showing CPU state, priority queues, physical frame map, device status, and live event logs.
* **Dioxus Web Dashboard (`nvos-web`):** Pure-Rust web application featuring WebSocket telemetry streaming, simulation stepping controls, memory map inspector, and interactive SVG/Canvas Gantt chart over discrete tick timestamps.
* **Trace Export:** Standardized trace export (Chrome Tracing / Perfetto JSON) for external offline timeline analysis.
* **Declarative Scenario Workloads:** TOML-based workload definitions for repeatable simulation scenarios and automated grading/testing.

### Out-of-Scope Non-Goals
* **Real Hardware Emulation or Virtualization:** No x86/ARM instruction set emulation, no real machine booting, no hardware virtualization (QEMU/KVM).
* **Real Multi-Threaded Execution of Processes:** Simulated processes do not map to OS-level threads; they are simulated state machines executing abstract bursts.
* **POSIX Compliance:** The filesystem and syscall interfaces are conceptual educational models, not a complete POSIX-compliant implementation.
* **Real Host Asynchronous I/O Drivers:** No integration with host operating system async primitives (e.g. `io_uring`, `epoll`).

## Alternatives Weighed

* **JavaScript/TypeScript Frontend vs. Pure-Rust Dioxus:** JavaScript frameworks (React/Vue) rejected in favor of Dioxus to preserve a 100% pure-Rust codebase, unified cargo tooling, shared type definitions (`Tick`, `Pid`, `ProcessState`), and optional direct WASM compilation.
* **Deferring Web Interface to Future Work:** Rejected by author. An interactive Web visualization with a Gantt chart is a mandatory acceptance requirement. Elevated to a first-class workspace crate (`nvos-web`).
* **Real Threads with Channels vs. Discrete-Event Queue:** Rejected by author. Real threads introduce non-deterministic execution order, race conditions, and wall-clock dependency, undermining algorithm verification and automated grading.
* **Synchronous Single-Threaded UI vs. Async Architecture:** Synchronous loop rejected by author in favor of an asynchronous architecture using `tokio`, decoupling UI/web frame pacing from simulation tick progression.
* **Unbounded In-Memory Event Accumulation vs. Sliding Window:** Unbounded logging rejected during brainstorm due to memory exhaustion; resolved via configurable sliding ring-buffer window.
* **Single Monolithic Crate vs. Cargo Workspace:** Rejected by author. Placing all subsystems in a single crate leads to tight coupling, high build times, and inability to test schedulers or allocators in isolation.
* **Distributed `Arc<Mutex<T>>` Shared State:** Rejected during brainstorm. Introduces deadlock risks and obfuscates data ownership; replaced with a centralized generational arena inside the kernel.

## Key Decisions & Staging for Next Steps

1. **Workspace Decomposition:**
   * `nvos-core`: Core primitives (`Tick`, `Pid`, `ProcessState`, `ProcessControlBlock`, event traits).
   * `nvos-cpu`: Scheduling traits and implementations (FCFS, RR, MLFQ, SJF).
   * `nvos-mem`: Physical frame allocator, page tables, TLB, page replacement policies.
   * `nvos-fs`: Inode tables, superblock, simulated disk blocks, file descriptors.
   * `nvos-io`: Virtual device controllers, interrupt broker, DMA channels.
   * `nvos-kernel`: Discrete-event engine, two-phase tick coordinator, syscall/trap mediator, async worker runner.
   * `nvos-trace`: Structured event logging, sliding ring buffer window, Perfetto/Chrome Tracing exporter.
   * `nvos-scenario`: TOML workload parser and deterministic scenario generator.
   * `nvos-tui`: Interactive Ratatui interface with split-pane status, non-blocking snapshot subscriber, and playback controls.
   * `nvos-web`: Pure-Rust Dioxus frontend (WASM/Desktop) and Axum WebSocket server, rendering the interactive Gantt chart.
   * `nvos-cli`: Main CLI runner binary for headless execution, scenario running, TUI launch (`nvos tui`), and web server launch (`nvos web`).

2. **Staged Implementation Milestones:**
   * **Phase 0:** Workspace scaffolding, linting setup, `nvos-core` primitives.
   * **Phase 1:** `nvos-cpu` schedulers and headless `nvos-kernel` discrete-event loop with synthetic burst execution.
   * **Phase 2:** `nvos-mem` frame allocator, page tables, TLB, and page fault handling.
   * **Phase 3:** `nvos-fs` mock inodes and `nvos-io` interrupt-driven device controllers.
   * **Phase 4:** `nvos-tui` live Ratatui interface with time-stepping and replay controls over async watch channels.
   * **Phase 5:** `nvos-web` Dioxus application with live WebSocket state streaming and interactive Gantt chart.
   * **Phase 6:** `nvos-scenario` TOML trace runner and external trace exporter validation.
