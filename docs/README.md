# Documentation: `nvos` Operating System Simulator

This directory contains the complete design architecture, subsystem technical specifications, original concept brief, and architectural review logs for `nvos`.

---

## Directory Structure

* **[`concepts_brief.md`](concepts_brief.md)**: Original technical and algorithmic conceptual brief.
* **[`designs/`](designs/)**: Active and modular architecture specifications.
  * **[`designs/nvos_simulator.md`](designs/nvos_simulator.md)**: Master technical design document incorporating all 21 multi-lens Goldfish review findings.
  * **[`designs/SITEMAP.md`](designs/SITEMAP.md)**: Master sitemap indexing all subsystem specifications and reviews.
  * **[`designs/subsystems/`](designs/subsystems/)**:
    * [`01_kernel_and_pcb.md`](designs/subsystems/01_kernel_and_pcb.md): Kernel loop, Tanenbaum PCB, tape execution, error model.
    * [`02_cpu_and_scheduling.md`](designs/subsystems/02_cpu_and_scheduling.md): CPU scheduling algorithms and property testing.
    * [`03_memory_management.md`](designs/subsystems/03_memory_management.md): Virtual paging, TLB, page replacement, and heap allocation.
    * [`04_filesystem_and_io.md`](designs/subsystems/04_filesystem_and_io.md): Inode storage engine and virtual DMA I/O controllers.
    * [`05_telemetry_and_ui.md`](designs/subsystems/05_telemetry_and_ui.md): Telemetry ring buffer, Ratatui TUI, and Dioxus Gantt chart UI.
* **[`review/`](review/)**: Multi-lens review and audit reports.
  * [`zoom_out_report.md`](review/zoom_out_report.md): Stage 5 Initial Zoom Out review findings.
  * [`comprehension_report.md`](review/comprehension_report.md): Stage 6 Comprehension test audit.
  * [`critic_report.md`](review/critic_report.md): Stage 7 Hostile Critic findings.
  * [`intermediate_zoom_out_report.md`](review/intermediate_zoom_out_report.md): Stage 8 Intermediate Zoom Out findings.
  * [`implementation_readiness_report.md`](review/implementation_readiness_report.md): Stage 9 Implementation Readiness report.
  * [`design_draft_original.md`](review/design_draft_original.md): Archived original unrevised draft.
