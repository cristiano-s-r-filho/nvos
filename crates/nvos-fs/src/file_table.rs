//! System-wide open file table and per-process FileDescriptor mapping.

use std::collections::BTreeMap;
use nvos_core::process::{FileDescriptor, InodeId, Pid};

/// An open file description entry in the system-wide table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFileEntry {
    pub inode_id: InodeId,
    pub offset: usize,
    pub writable: bool,
    pub ref_count: usize,
}

/// Global open file table and per-process file descriptor allocator.
#[derive(Debug, Default)]
pub struct FileTable {
    next_fd: u32,
    entries: BTreeMap<FileDescriptor, OpenFileEntry>,
    process_fds: BTreeMap<Pid, Vec<FileDescriptor>>,
}

impl FileTable {
    /// Create a new file table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_fd: 1, // 0 is reserved / stdin
            entries: BTreeMap::new(),
            process_fds: BTreeMap::new(),
        }
    }

    /// Allocate a new file descriptor for an opened inode.
    pub fn open(&mut self, pid: Pid, inode_id: InodeId, writable: bool) -> FileDescriptor {
        let fd = FileDescriptor(self.next_fd);
        self.next_fd = self.next_fd.saturating_add(1);

        self.entries.insert(
            fd,
            OpenFileEntry {
                inode_id,
                offset: 0,
                writable,
                ref_count: 1,
            },
        );

        self.process_fds.entry(pid).or_default().push(fd);
        fd
    }

    /// Close a file descriptor for a given process.
    pub fn close(&mut self, pid: Pid, fd: FileDescriptor) -> bool {
        if let Some(fds) = self.process_fds.get_mut(&pid) {
            if let Some(pos) = fds.iter().position(|&f| f == fd) {
                fds.remove(pos);
                self.entries.remove(&fd);
                return true;
            }
        }
        false
    }

    /// Lookup open file entry for an FD.
    #[must_use]
    pub fn get(&self, fd: FileDescriptor) -> Option<&OpenFileEntry> {
        self.entries.get(&fd)
    }

    /// Lookup mutable open file entry for an FD (e.g. to advance seek offset).
    pub fn get_mut(&mut self, fd: FileDescriptor) -> Option<&mut OpenFileEntry> {
        self.entries.get_mut(&fd)
    }

    /// Close all file descriptors owned by a terminated process.
    pub fn close_all_for_process(&mut self, pid: Pid) {
        if let Some(fds) = self.process_fds.remove(&pid) {
            for fd in fds {
                self.entries.remove(&fd);
            }
        }
    }
}
