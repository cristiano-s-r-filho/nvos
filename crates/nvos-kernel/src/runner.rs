//! Async Tokio simulation worker bridging synchronous [`KernelEngine`]
//! to presentation consumers (TUI, Web, Exporters).
//!
//! Subscribes to commands on an `mpsc` channel and publishes:
//! - Coarse status (`EngineStatus`) on `tokio::sync::watch`
//! - Per-process execution spans (`SpanEvent`) on `tokio::sync::broadcast`

use nvos_core::error::KernelEngineError;
use nvos_core::events::{EngineStatus, SpanEvent};
use tokio::sync::{broadcast, mpsc, watch};
use crate::engine::KernelEngine;

/// Commands accepted by [`SimulationRunner`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineCommand {
    /// Resume running ticks continuously at the configured interval.
    Play,
    /// Pause tick advancement.
    Pause,
    /// Advance the simulation by exactly one tick.
    Step,
    /// Reset the simulation engine to tick zero.
    Reset,
}

/// Tokio async worker managing the discrete-event simulation lifecycle.
pub struct SimulationRunner {
    engine: KernelEngine,
    cmd_receiver: mpsc::Receiver<EngineCommand>,
    status_sender: watch::Sender<EngineStatus>,
    span_sender: broadcast::Sender<SpanEvent>,
    is_playing: bool,
    tick_delay_ms: u64,
}

impl SimulationRunner {
    /// Create a new runner around an engine instance.
    #[must_use]
    pub fn new(
        engine: KernelEngine,
        cmd_receiver: mpsc::Receiver<EngineCommand>,
        status_sender: watch::Sender<EngineStatus>,
        span_sender: broadcast::Sender<SpanEvent>,
    ) -> Self {
        Self {
            engine,
            cmd_receiver,
            status_sender,
            span_sender,
            is_playing: false,
            tick_delay_ms: 10,
        }
    }

    /// Set the wall-clock sleep duration between ticks when playing (in milliseconds).
    pub fn set_tick_delay_ms(&mut self, delay: u64) {
        self.tick_delay_ms = delay;
    }

    /// Run the simulation task event loop until the command channel closes.
    pub async fn run(&mut self) -> Result<(), KernelEngineError> {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(self.tick_delay_ms.max(1)));

        loop {
            tokio::select! {
                cmd_opt = self.cmd_receiver.recv() => {
                    match cmd_opt {
                        Some(EngineCommand::Play) => {
                            self.is_playing = true;
                            self.publish_status();
                        }
                        Some(EngineCommand::Pause | EngineCommand::Reset) => {
                            self.is_playing = false;
                            self.publish_status();
                        }
                        Some(EngineCommand::Step) => {
                            self.step_one_tick()?;
                        }
                        None => {
                            // Command channel closed, terminate runner
                            break;
                        }
                    }
                }
                _ = interval.tick(), if self.is_playing => {
                    self.step_one_tick()?;
                }
            }
        }

        Ok(())
    }

    fn step_one_tick(&mut self) -> Result<(), KernelEngineError> {
        self.engine.tick()?;

        // Broadcast newly emitted Gantt spans
        for span in self.engine.drain_span_events() {
            let _ = self.span_sender.send(span);
        }

        // Publish coarse engine status to watch channel
        self.publish_status();
        Ok(())
    }

    fn publish_status(&self) {
        let mut status = self.engine.engine_status();
        status.is_running = self.is_playing;
        let _ = self.status_sender.send(status);
    }
}
