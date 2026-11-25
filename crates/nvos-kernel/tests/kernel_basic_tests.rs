use nvos_core::process::{PriorityLevel, ProcessState};
use nvos_core::tape::InstructionToken;
use nvos_core::tick::Tick;
use nvos_cpu::{FcfsScheduler, RoundRobinScheduler};
use nvos_kernel::{safe_subsystem_dispatch, EngineCommand, KernelEngine, SimulationRunner};
use tokio::sync::{broadcast, mpsc, watch};

#[test]
fn test_kernel_fcfs_basic_execution() {
    let mut engine = KernelEngine::new(Box::new(FcfsScheduler::new()));

    // Spawn 2 processes with distinct CPU bursts
    let p1 = engine.spawn_process(
        "proc_1",
        PriorityLevel(0),
        vec![InstructionToken::Cpu(2), InstructionToken::Exit(0)],
    );
    let p2 = engine.spawn_process(
        "proc_2",
        PriorityLevel(0),
        vec![InstructionToken::Cpu(1), InstructionToken::Exit(0)],
    );

    assert_eq!(engine.current_tick(), Tick(0));

    // Tick 0: p1 & p2 admitted to Ready queue; p1 dispatched to Running
    engine.tick().unwrap();
    assert_eq!(engine.running_pid(), Some(p1));
    assert_eq!(engine.arena().get(p1).unwrap().state, ProcessState::Running);
    assert_eq!(engine.arena().get(p2).unwrap().state, ProcessState::Ready);

    // Tick 1: p1 consumes first tick of Cpu(2)
    engine.tick().unwrap();
    assert_eq!(engine.running_pid(), Some(p1));
    assert_eq!(engine.arena().get(p1).unwrap().state, ProcessState::Running);
    assert_eq!(engine.arena().get(p1).unwrap().current_token_progress, 1);

    // Tick 2: p1 completes second CPU burst and terminates
    engine.tick().unwrap();
    assert_eq!(
        engine.arena().get(p1).unwrap().state,
        ProcessState::Terminated(nvos_core::process::ExitReason::Normal(0))
    );

    // Tick 3: p2 dispatched to Running, completes burst and terminates
    engine.tick().unwrap();
    assert_eq!(
        engine.arena().get(p2).unwrap().state,
        ProcessState::Terminated(nvos_core::process::ExitReason::Normal(0))
    );
}

#[test]
fn test_kernel_round_robin_preemption_and_interleaving() {
    // Quantum = 1 tick
    let mut engine = KernelEngine::new(Box::new(RoundRobinScheduler::new(1)));

    let p1 = engine.spawn_process(
        "rr_p1",
        PriorityLevel(0),
        vec![InstructionToken::Cpu(2), InstructionToken::Exit(0)],
    );
    let p2 = engine.spawn_process(
        "rr_p2",
        PriorityLevel(0),
        vec![InstructionToken::Cpu(2), InstructionToken::Exit(0)],
    );

    // Tick 0: Admission & dispatch p1
    engine.tick().unwrap();
    assert_eq!(engine.running_pid(), Some(p1));

    // Tick 1: p1 quantum expires -> preempted, p2 dispatched
    engine.tick().unwrap();
    assert_eq!(engine.running_pid(), Some(p2));
    assert_eq!(engine.arena().get(p1).unwrap().state, ProcessState::Ready);
    assert_eq!(engine.arena().get(p1).unwrap().current_token_progress, 1);

    // Tick 2: p2 quantum expires -> preempted, p1 dispatched
    engine.tick().unwrap();
    assert_eq!(engine.running_pid(), Some(p1));

    // Tick 3: p1 finishes remaining burst and terminates
    engine.tick().unwrap();
    assert!(engine.arena().get(p1).unwrap().state.is_terminated());
}

#[test]
fn test_panic_boundary_catches_unwind() {
    let result: Result<(), _> = safe_subsystem_dispatch("student-scheduler", || {
        panic!("simulated student algorithmic panic!");
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.subsystem, "student-scheduler");
    assert!(err.to_string().contains("simulated student algorithmic panic!"));
}

#[tokio::test]
async fn test_async_simulation_runner_step() {
    let engine = KernelEngine::new(Box::new(FcfsScheduler::new()));
    let (cmd_tx, cmd_rx) = mpsc::channel(8);
    let (status_tx, mut status_rx) = watch::channel(engine.engine_status());
    let (span_tx, span_rx) = broadcast::channel(16);

    let mut runner = SimulationRunner::new(engine, cmd_rx, status_tx, span_tx);

    let handle = tokio::spawn(async move {
        runner.run().await.unwrap();
    });

    // Send Step command
    cmd_tx.send(EngineCommand::Step).await.unwrap();

    // Verify status tick incremented
    status_rx.changed().await.unwrap();
    let status = status_rx.borrow().clone();
    assert_eq!(status.current_tick, Tick(1));

    // Clean shutdown
    drop(cmd_tx);
    handle.await.unwrap();
    assert!(span_rx.is_empty());
}
