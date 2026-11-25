//! Process arena storing Process Control Blocks (`ProcessControlBlock`).
//!
//! To keep `nvos-core` completely free of the `slotmap` dependency, this arena
//! maintains an internal `ProcessKey` slotmap and a bidirectional mapping
//! between `Pid` and `ProcessKey`.

use std::collections::BTreeMap;
use nvos_core::process::{Pid, ProcessControlBlock};
use slotmap::{new_key_type, DenseSlotMap};

new_key_type! {
    /// Internal generational key used by `slotmap::DenseSlotMap`.
    pub struct ProcessKey;
}

/// Generational arena managing allocated `ProcessControlBlock` entries.
#[derive(Debug, Default)]
pub struct ProcessArena {
    slots: DenseSlotMap<ProcessKey, ProcessControlBlock>,
    pid_to_key: BTreeMap<Pid, ProcessKey>,
    key_to_pid: BTreeMap<ProcessKey, Pid>,
}

impl ProcessArena {
    /// Create a new, empty process arena.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a PCB into the arena. Returns the assigned `Pid`.
    pub fn insert(&mut self, pcb: ProcessControlBlock) -> Pid {
        let pid = pcb.pid;
        let key = self.slots.insert(pcb);
        self.pid_to_key.insert(pid, key);
        self.key_to_pid.insert(key, pid);
        pid
    }

    /// Retrieve an immutable reference to a PCB by its `Pid`.
    #[must_use]
    pub fn get(&self, pid: Pid) -> Option<&ProcessControlBlock> {
        let key = self.pid_to_key.get(&pid)?;
        self.slots.get(*key)
    }

    /// Retrieve a mutable reference to a PCB by its `Pid`.
    pub fn get_mut(&mut self, pid: Pid) -> Option<&mut ProcessControlBlock> {
        let key = self.pid_to_key.get(&pid)?;
        self.slots.get_mut(*key)
    }

    /// Remove a PCB from the arena by its `Pid`.
    pub fn remove(&mut self, pid: Pid) -> Option<ProcessControlBlock> {
        let key = self.pid_to_key.remove(&pid)?;
        self.key_to_pid.remove(&key);
        self.slots.remove(key)
    }

    /// Check if a PID exists in the arena.
    #[must_use]
    pub fn contains(&self, pid: Pid) -> bool {
        self.pid_to_key.contains_key(&pid)
    }

    /// Number of live processes in the arena.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the arena is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Iterate over all live process control blocks.
    pub fn iter(&self) -> impl Iterator<Item = &ProcessControlBlock> {
        self.slots.values()
    }

    /// Iterate mutably over all live process control blocks.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ProcessControlBlock> {
        self.slots.values_mut()
    }

    /// Iterate over all PIDs tracked in the arena.
    pub fn pids(&self) -> impl Iterator<Item = Pid> + '_ {
        self.pid_to_key.keys().copied()
    }
}
