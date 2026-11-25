use nvos_core::process::Pid;
use nvos_core::tick::Tick;
use nvos_io::{DeviceInterrupt, DeviceManager, VirtualDiskController, VirtualTimer};

#[test]
fn test_timer_periodic_interrupts() {
    let mut timer = VirtualTimer::new(2); // fires every 2 ticks

    assert!(timer.poll(Tick(0))); // tick 0 fires
    assert!(!timer.poll(Tick(1))); // 1 tick elapsed < 2 -> no fire
    assert!(timer.poll(Tick(2))); // 2 ticks elapsed -> fire
    assert_eq!(timer.total_interrupts(), 2);
}

#[test]
fn test_disk_seek_and_transfer_latency() {
    let mut disk = VirtualDiskController::new(32); // 32 sectors per cylinder

    // Sector 0 on cylinder 0
    let t0 = Tick(0);
    let comp1 = disk.schedule_transfer(Pid(1), vec![0, 1], t0);
    // seek = 0, rot = 3, transfer = 4 => latency = 7
    assert_eq!(comp1, Tick(7));

    // Sector 64 on cylinder 2 (seek distance = 2)
    let comp2 = disk.schedule_transfer(Pid(2), vec![64], t0);
    // seek = 2, rot = 3, transfer = 2 => latency = 7
    assert_eq!(comp2, Tick(7));

    // At Tick 6: neither completed
    let done_early = disk.poll_completions(Tick(6));
    assert!(done_early.is_empty());

    // At Tick 7: both completed
    let done = disk.poll_completions(Tick(7));
    assert_eq!(done.len(), 2);
    assert!(done.contains(&Pid(1)));
    assert!(done.contains(&Pid(2)));
}

#[test]
fn test_device_manager_poll_hardware() {
    let mut dm = DeviceManager::new();
    let pid = Pid(42);

    dm.schedule_disk_transfer(pid, vec![0], Tick(0)); // completes at tick 5

    // Tick 0: timer fires
    let ints0 = dm.poll_hardware(Tick(0));
    assert!(ints0.contains(&DeviceInterrupt::TimerTick));

    // Tick 5: disk completion interrupt fires
    let ints5 = dm.poll_hardware(Tick(5));
    assert!(ints5.contains(&DeviceInterrupt::DiskTransferComplete { pid }));
}
