use nvos_core::error::{GuestTrap, KernelEngineError, SimulationError, TrapReason};
use nvos_core::events::EngineStatus;
use nvos_core::process::{
    BlockReason, ExitReason, MemoryLimits, Pid, PriorityLevel, ProcessControlBlock, ProcessState,
};
use nvos_core::tape::InstructionToken;
use nvos_core::tick::{DurationTicks, Tick};

#[test]
fn test_tick_arithmetic_and_saturation() {
    let t0 = Tick::ZERO;
    assert_eq!(t0.0, 0);

    let t1 = t0.next();
    assert_eq!(t1.0, 1);

    let d5 = DurationTicks(5);
    let t6 = t1 + d5;
    assert_eq!(t6.0, 6);

    let mut t_mut = t6;
    t_mut += DurationTicks(4);
    assert_eq!(t_mut.0, 10);

    let diff = t_mut - t6;
    assert_eq!(diff.0, 4);

    let elapsed = t_mut.elapsed_since(t6);
    assert_eq!(elapsed.0, 4);

    let elapsed_neg = t6.elapsed_since(t_mut);
    assert_eq!(elapsed_neg.0, 0);
}

#[test]
fn test_tick_and_duration_display() {
    let t = Tick(42);
    assert_eq!(format!("{t}"), "T42");

    let d = DurationTicks(10);
    assert_eq!(format!("{d}"), "10Δt");
}

#[test]
fn test_pcb_new_default_values() {
    let tape = vec![InstructionToken::Cpu(5), InstructionToken::Exit(0)];
    let pcb = ProcessControlBlock::new(
        Pid(1),
        None,
        "test_proc",
        PriorityLevel(10),
        Tick(0),
        tape,
    );

    assert_eq!(pcb.pid, Pid(1));
    assert_eq!(pcb.ppid, None);
    assert_eq!(pcb.name, "test_proc");
    assert_eq!(pcb.state, ProcessState::New);
    assert_eq!(pcb.priority, PriorityLevel(10));
    assert_eq!(pcb.base_priority, PriorityLevel(10));
    assert_eq!(pcb.quantum_remaining, 0);
    assert_eq!(pcb.program_counter, 0);
    assert_eq!(pcb.current_token_progress, 0);
    assert_eq!(pcb.cpu_time_consumed, 0);
    assert_eq!(pcb.arrival_tick, Tick(0));
    assert_eq!(pcb.completion_tick, None);
    assert_eq!(pcb.page_table_root, None);
    assert!(pcb.allocated_heap_blocks.is_empty());
    assert!(pcb.file_descriptors.is_empty());
    assert_eq!(pcb.execution_tape.len(), 2);
}

#[test]
fn test_token_progress_reset_on_retire() {
    let tape = vec![InstructionToken::Cpu(5), InstructionToken::Exit(0)];
    let mut pcb = ProcessControlBlock::new(
        Pid(1),
        None,
        "test_proc",
        PriorityLevel(0),
        Tick(0),
        tape,
    );

    pcb.current_token_progress = 3;
    assert_eq!(pcb.program_counter, 0);

    let retired = pcb.retire_current_token();
    assert!(retired);
    assert_eq!(pcb.program_counter, 1);
    assert_eq!(pcb.current_token_progress, 0);
}

#[test]
fn test_tape_exhaustion_detection() {
    let tape = vec![InstructionToken::Cpu(1)];
    let mut pcb = ProcessControlBlock::new(
        Pid(1),
        None,
        "test_proc",
        PriorityLevel(0),
        Tick(0),
        tape,
    );

    assert!(!pcb.tape_exhausted());
    assert_eq!(pcb.current_token(), Some(&InstructionToken::Cpu(1)));

    let retired = pcb.retire_current_token();
    assert!(retired);
    assert!(pcb.tape_exhausted());
    assert_eq!(pcb.current_token(), None);
}

#[test]
fn test_retire_on_empty_tape_does_not_panic() {
    let mut pcb = ProcessControlBlock::new(
        Pid(1),
        None,
        "test_proc",
        PriorityLevel(0),
        Tick(0),
        vec![],
    );

    assert!(pcb.tape_exhausted());
    assert_eq!(pcb.current_token(), None);

    let retired = pcb.retire_current_token();
    assert!(!retired);
}

#[test]
fn test_process_state_queries() {
    let new_state = ProcessState::New;
    assert!(!new_state.is_ready());
    assert!(!new_state.is_terminated());

    let ready_state = ProcessState::Ready;
    assert!(ready_state.is_ready());
    assert!(!ready_state.is_terminated());

    let running_state = ProcessState::Running;
    assert!(!running_state.is_ready());
    assert!(!running_state.is_terminated());

    let blocked_state = ProcessState::Blocked(BlockReason::Io { device_id: 1 });
    assert!(!blocked_state.is_ready());
    assert!(!blocked_state.is_terminated());

    let sleeping_state = ProcessState::Sleeping(Tick(10));
    assert!(!sleeping_state.is_ready());
    assert!(!sleeping_state.is_terminated());

    let term_state = ProcessState::Terminated(ExitReason::Normal(0));
    assert!(!term_state.is_ready());
    assert!(term_state.is_terminated());
}

#[test]
fn test_priority_level_ordering() {
    let high = PriorityLevel(0);
    let low = PriorityLevel(255);
    assert!(high < low);
}

#[test]
fn test_memory_limits_defaults() {
    let limits = MemoryLimits::default();
    assert_eq!(limits.virtual_address_space, 64 * 1024 * 1024);
    assert_eq!(limits.max_resident_pages, 256);
    assert_eq!(limits.max_heap_bytes, 4 * 1024 * 1024);
}

#[test]
fn test_reasons_formatting() {
    let blk = BlockReason::PageFault { virtual_addr: 0x1000 };
    assert_eq!(format!("{blk:?}"), "PageFault { virtual_addr: 4096 }");

    let exit = ExitReason::SegmentationFault;
    assert_eq!(format!("{exit:?}"), "SegmentationFault");
}

#[test]
fn test_error_displays() {
    let trap = GuestTrap {
        pid: 1,
        reason: TrapReason::SegmentationFault { virtual_addr: 0xDEAD_BEEF },
    };
    let trap_str = format!("{trap}");
    assert!(trap_str.contains("pid=1"));
    assert!(trap_str.contains("SegFault @ vaddr=0xdeadbeef"));

    let eng_err = KernelEngineError {
        subsystem: "nvos-cpu",
        kind: nvos_core::error::EngineErrorKind::TickOverflow,
    };
    let eng_str = format!("{eng_err}");
    assert_eq!(eng_str, "[nvos-cpu] tick counter overflow");
}

#[test]
fn test_simulation_error_conversions() {
    let trap = GuestTrap {
        pid: 2,
        reason: TrapReason::BadFileDescriptor { fd: 5 },
    };
    let sim_err: SimulationError = trap.into();
    assert!(matches!(sim_err, SimulationError::Guest(_)));

    let eng_err = KernelEngineError {
        subsystem: "nvos-mem",
        kind: nvos_core::error::EngineErrorKind::ArenaCorruption {
            detail: "corrupted slotmap".into(),
        },
    };
    let sim_err2: SimulationError = eng_err.into();
    assert!(matches!(sim_err2, SimulationError::Engine(_)));
}

#[test]
fn test_engine_status_defaults() {
    let status = EngineStatus::default();
    assert!(!status.is_running);
    assert_eq!(status.current_tick, Tick(0));
    assert_eq!(status.active_process_count, 0);
}
