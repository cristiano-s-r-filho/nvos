# Design Documentation Sitemap: `nvos`

This sitemap registers the complete modular design architecture for the `nvos` deterministic discrete-event operating system simulator.

---

## Master Architecture Document
* [nvos_simulator.md](nvos_simulator.md): Master design document defining the problem statement, high-level architecture, Tanenbaum PCB model, workspace decomposition, alternatives considered, and sequenced implementation plan.

---

## Modular Subsystem Specifications

| Document | Target Modules | Primary Architectural Scope |
| :--- | :--- | :--- |
| [01_kernel_and_pcb.md](subsystems/01_kernel_and_pcb.md) | `nvos-core`, `nvos-kernel` | Two-Phase Commit tick cycle, Tanenbaum PCB layout, instruction tape with `current_token_progress`, two-tier error taxonomy, and safe panic boundary (`catch_unwind`). |
| [02_cpu_and_scheduling.md](subsystems/02_cpu_and_scheduling.md) | `nvos-cpu` | `Scheduler` trait with `Result<SchedulingAction, SchedulerError>`, FCFS, Round Robin, SJF/SRTF, MLFQ with starvation boost, property-based tests (`proptest`), and mutation testing (`cargo-mutants`). |
| [03_memory_management.md](subsystems/03_memory_management.md) | `nvos-mem` | Dual-layer memory model: Two-level paging with software TLB and page replacement (FIFO/LRU/Clock), contiguous physical heap allocators (First-Fit, Best-Fit, Buddy), and frame conservation fuzzing. |
| [04_filesystem_and_io.md](subsystems/04_filesystem_and_io.md) | `nvos-fs`, `nvos-io` | Mock Unix inode storage (superblocks, direct/indirect blocks), separation of instant metadata resolution from timed disk I/O, virtual disk controller DMA, and interrupt bus. |
| [05_telemetry_and_ui.md](subsystems/05_telemetry_and_ui.md) | `nvos-trace`, `nvos-tui`, `nvos-web` | Bounded sliding ring buffer ($N=1000$), Ratatui 60 FPS terminal dashboard, pure-Rust Dioxus web dashboard with SVG Gantt chart, and dual-target Cargo compilation. |

---

## Review & Audit Reports

* [zoom_out_report.md](../review/zoom_out_report.md): Stage 5 Initial Zoom Out review findings (two-tier error model, panic boundary).
* [comprehension_report.md](../review/comprehension_report.md): Stage 6 Comprehension test audit and terminology self-containment.
* [critic_report.md](../review/critic_report.md): Stage 7 Critic review findings (scheduler race resolution, PCB type decoupling).
* [intermediate_zoom_out_report.md](../review/intermediate_zoom_out_report.md): Stage 8 Intermediate Zoom Out findings (YAGNI pruning of reverse mutations, multi-tick token progress).
* [implementation_readiness_report.md](../review/implementation_readiness_report.md): Stage 9 Implementation Readiness report (file enumeration, target compilation split, testing harness).
* [design_draft_original.md](../review/design_draft_original.md): Original pre-review monolithic design draft (archived).
