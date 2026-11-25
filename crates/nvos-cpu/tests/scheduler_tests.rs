use nvos_core::process::{BlockReason, Pid, PriorityLevel};
use nvos_core::tick::Tick;
use nvos_cpu::{
    FcfsScheduler, MlfqScheduler, RoundRobinScheduler, Scheduler, SchedulerError,
    SchedulingAction, SjfScheduler,
};

#[test]
fn test_fcfs_order_and_non_preemption() {
    let mut sched = FcfsScheduler::new();
    assert_eq!(sched.name(), "FCFS");

    sched.add_process(Pid(1), PriorityLevel(0)).unwrap();
    sched.add_process(Pid(2), PriorityLevel(0)).unwrap();
    sched.add_process(Pid(3), PriorityLevel(0)).unwrap();

    // First scheduled is PID 1
    let p1 = sched.schedule_next(Tick(0)).unwrap();
    assert_eq!(p1, Some(Pid(1)));

    // Tick always returns Continue (non-preemptive)
    let action = sched.tick(p1).unwrap();
    assert_eq!(action, SchedulingAction::Continue);

    // Termination of PID 1
    sched.on_process_terminated(Pid(1)).unwrap();

    // Next scheduled is PID 2
    let p2 = sched.schedule_next(Tick(1)).unwrap();
    assert_eq!(p2, Some(Pid(2)));
}

#[test]
fn test_round_robin_preemption_and_tail_cycle() {
    let mut sched = RoundRobinScheduler::new(2); // quantum = 2 ticks
    assert_eq!(sched.name(), "RoundRobin");

    sched.add_process(Pid(10), PriorityLevel(0)).unwrap();
    sched.add_process(Pid(20), PriorityLevel(0)).unwrap();

    let p = sched.schedule_next(Tick(0)).unwrap();
    assert_eq!(p, Some(Pid(10)));

    // Tick 1: 1 tick consumed -> Continue
    let a1 = sched.tick(p).unwrap();
    assert_eq!(a1, SchedulingAction::Continue);

    // Tick 2: 2 ticks consumed -> Preempt (quantum reached)
    let a2 = sched.tick(p).unwrap();
    assert_eq!(a2, SchedulingAction::Preempt);

    // Next dispatch picks PID 20
    let next = sched.schedule_next(Tick(2)).unwrap();
    assert_eq!(next, Some(Pid(20)));

    // After PID 20's quantum, PID 10 runs again
    sched.tick(next).unwrap();
    let a3 = sched.tick(next).unwrap();
    assert_eq!(a3, SchedulingAction::Preempt);

    let next2 = sched.schedule_next(Tick(4)).unwrap();
    assert_eq!(next2, Some(Pid(10)));
}

#[test]
fn test_sjf_and_srtf_scheduling() {
    // Non-preemptive SJF
    let mut sjf = SjfScheduler::new(false);
    sjf.set_estimated_burst(Pid(1), 10);
    sjf.set_estimated_burst(Pid(2), 3);
    sjf.set_estimated_burst(Pid(3), 7);

    sjf.add_process(Pid(1), PriorityLevel(0)).unwrap();
    sjf.add_process(Pid(2), PriorityLevel(0)).unwrap();
    sjf.add_process(Pid(3), PriorityLevel(0)).unwrap();

    // Shortest job PID 2 runs first
    let p = sjf.schedule_next(Tick(0)).unwrap();
    assert_eq!(p, Some(Pid(2)));

    // Preemptive SRTF
    let mut srtf = SjfScheduler::new(true);
    srtf.set_estimated_burst(Pid(10), 10);
    srtf.add_process(Pid(10), PriorityLevel(0)).unwrap();

    let p10 = srtf.schedule_next(Tick(0)).unwrap();
    assert_eq!(p10, Some(Pid(10)));

    srtf.tick(p10).unwrap(); // remaining becomes 9

    // Shorter job arrives (burst = 2)
    srtf.set_estimated_burst(Pid(20), 2);
    srtf.add_process(Pid(20), PriorityLevel(0)).unwrap();

    // Next tick detects shorter job and preempts
    let action = srtf.tick(p10).unwrap();
    assert_eq!(action, SchedulingAction::Preempt);

    let next = srtf.schedule_next(Tick(2)).unwrap();
    assert_eq!(next, Some(Pid(20)));
}

#[test]
fn test_mlfq_demotion_and_starvation_boost() {
    let mut mlfq = MlfqScheduler::new(3, 2, 10); // 3 queues, Q0 quantum = 2, boost every 10 ticks

    sched_flow(&mut mlfq);
}

fn sched_flow(mlfq: &mut MlfqScheduler) {
    mlfq.add_process(Pid(100), PriorityLevel(0)).unwrap();
    assert_eq!(mlfq.process_level(Pid(100)), Some(0));

    let p = mlfq.schedule_next(Tick(0)).unwrap();
    assert_eq!(p, Some(Pid(100)));

    // Tick 1: allotment = 1 -> Continue
    let a1 = mlfq.tick(p).unwrap();
    assert_eq!(a1, SchedulingAction::Continue);

    // Tick 2: allotment = 2 >= Q0 quantum -> Preempt & demote to Q1
    let a2 = mlfq.tick(p).unwrap();
    assert_eq!(a2, SchedulingAction::Preempt);
    assert_eq!(mlfq.process_level(Pid(100)), Some(1));

    // Fast-forward to tick 10 to trigger Starvation Boost
    let _ = mlfq.schedule_next(Tick(10)).unwrap();
    assert_eq!(mlfq.process_level(Pid(100)), Some(0));
}

#[test]
fn test_duplicate_and_not_found_errors() {
    let mut sched = FcfsScheduler::new();
    sched.add_process(Pid(1), PriorityLevel(0)).unwrap();

    // Adding duplicate returns error
    let err = sched.add_process(Pid(1), PriorityLevel(0)).unwrap_err();
    assert_eq!(err, SchedulerError::DuplicateProcess(Pid(1)));

    // Blocking non-existent PID returns error
    let err_block = sched.on_process_blocked(Pid(99), BlockReason::PageFault { virtual_addr: 0 }).unwrap_err();
    assert_eq!(err_block, SchedulerError::ProcessNotFound(Pid(99)));
}
