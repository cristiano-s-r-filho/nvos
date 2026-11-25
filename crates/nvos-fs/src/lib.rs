//! `nvos-fs` — mock Unix inode-based filesystem for `nvos`.
//!
//! Separates synchronous metadata resolution (`resolve_read_blocks`)
//! from timed physical DMA disk transfers (handled by `nvos-io`).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::all)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::cast_possible_truncation,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    clippy::single_match_else
)]

use std::collections::BTreeMap;
use std::fmt;
use nvos_core::process::{FileDescriptor, InodeId, Pid};

pub mod block_device;
pub mod file_table;
pub mod inode;
pub mod superblock;

pub use block_device::MockBlockDevice;
pub use file_table::FileTable;
pub use inode::{Inode, DIRECT_BLOCKS_COUNT};
pub use superblock::{Superblock, BLOCK_SIZE, DEFAULT_TOTAL_BLOCKS, DEFAULT_TOTAL_INODES};

/// Errors produced by filesystem operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    /// Specified path was not found.
    NotFound(String),
    /// File descriptor does not exist or has been closed.
    InvalidFd(FileDescriptor),
    /// No free sectors available on disk.
    DiskFull,
    /// Inode table capacity exhausted.
    NoFreeInodes,
    /// Operation not permitted (e.g. write to read-only file).
    PermissionDenied,
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "file not found: {path}"),
            Self::InvalidFd(fd) => write!(f, "invalid file descriptor {fd:?}"),
            Self::DiskFull => write!(f, "disk full: no free blocks available"),
            Self::NoFreeInodes => write!(f, "inode table exhausted"),
            Self::PermissionDenied => write!(f, "operation not permitted"),
        }
    }
}

impl std::error::Error for FsError {}

/// FileSystem abstraction separating metadata resolution from timed I/O.
pub trait FileSystem: Send + Sync {
    /// Create a file in the directory hierarchy.
    fn create_file(&mut self, path: &str, permissions: u16) -> Result<InodeId, FsError>;

    /// Open a file and allocate a process file descriptor.
    fn open(&mut self, pid: Pid, path: &str, writable: bool) -> Result<FileDescriptor, FsError>;

    /// Close an open file descriptor.
    fn close(&mut self, pid: Pid, fd: FileDescriptor) -> Result<(), FsError>;

    /// Resolve an open file descriptor to a list of disk sector block IDs for timed read.
    /// Operates in zero simulation ticks (instant metadata resolution).
    fn resolve_read_blocks(&self, fd: FileDescriptor, offset: usize, len: usize) -> Result<Vec<u32>, FsError>;

    /// Allocate disk blocks and write data into in-memory storage, returning affected sector IDs.
    fn write_blocks(&mut self, fd: FileDescriptor, offset: usize, data: &[u8]) -> Result<Vec<u32>, FsError>;
}

/// Standard in-memory mock Unix-style inode filesystem.
#[derive(Debug)]
pub struct InodeFileSystem {
    superblock: Superblock,
    device: MockBlockDevice,
    inodes: BTreeMap<InodeId, Inode>,
    path_to_inode: BTreeMap<String, InodeId>,
    file_table: FileTable,
    free_block_bitmap: Vec<bool>, // false = free, true = allocated
    next_inode_id: u32,
}

impl InodeFileSystem {
    /// Create a new initialized mock inode filesystem.
    #[must_use]
    pub fn new() -> Self {
        let sb = Superblock::default();
        let device = MockBlockDevice::new(sb.total_blocks);
        let mut bitmap = vec![false; sb.total_blocks as usize];

        // Reserve superblock and inode table blocks
        for i in 0..sb.data_blocks_start_block as usize {
            bitmap[i] = true;
        }

        let mut fs = Self {
            superblock: sb,
            device,
            inodes: BTreeMap::new(),
            path_to_inode: BTreeMap::new(),
            file_table: FileTable::new(),
            free_block_bitmap: bitmap,
            next_inode_id: 1,
        };

        // Create root directory "/"
        let root_id = InodeId(0);
        fs.inodes.insert(root_id, Inode::new_directory(root_id, 0o755));
        fs.path_to_inode.insert("/".to_string(), root_id);
        fs
    }

    fn allocate_data_block(&mut self) -> Result<u32, FsError> {
        let start = self.superblock.data_blocks_start_block as usize;
        let free_idx = self.free_block_bitmap[start..]
            .iter()
            .position(|&used| !used)
            .map(|pos| start + pos)
            .ok_or(FsError::DiskFull)?;

        self.free_block_bitmap[free_idx] = true;
        self.superblock.free_blocks = self.superblock.free_blocks.saturating_sub(1);
        Ok(free_idx as u32)
    }

    /// Direct reference to underlying mock block device (for DMA reads).
    #[must_use]
    pub fn block_device(&self) -> &MockBlockDevice {
        &self.device
    }

    /// Mutable reference to underlying mock block device.
    pub fn block_device_mut(&mut self) -> &mut MockBlockDevice {
        &mut self.device
    }
}

impl Default for InodeFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for InodeFileSystem {
    fn create_file(&mut self, path: &str, permissions: u16) -> Result<InodeId, FsError> {
        if self.path_to_inode.contains_key(path) {
            return Ok(self.path_to_inode[path]);
        }

        if self.inodes.len() >= self.superblock.total_inodes as usize {
            return Err(FsError::NoFreeInodes);
        }

        let id = InodeId(self.next_inode_id);
        self.next_inode_id = self.next_inode_id.saturating_add(1);

        let inode = Inode::new_file(id, permissions);
        self.inodes.insert(id, inode);
        self.path_to_inode.insert(path.to_string(), id);
        self.superblock.free_inodes = self.superblock.free_inodes.saturating_sub(1);

        Ok(id)
    }

    fn open(&mut self, pid: Pid, path: &str, writable: bool) -> Result<FileDescriptor, FsError> {
        let inode_id = self
            .path_to_inode
            .get(path)
            .copied()
            .ok_or_else(|| FsError::NotFound(path.to_string()))?;

        let fd = self.file_table.open(pid, inode_id, writable);
        Ok(fd)
    }

    fn close(&mut self, pid: Pid, fd: FileDescriptor) -> Result<(), FsError> {
        if self.file_table.close(pid, fd) {
            Ok(())
        } else {
            Err(FsError::InvalidFd(fd))
        }
    }

    fn resolve_read_blocks(
        &self,
        fd: FileDescriptor,
        offset: usize,
        len: usize,
    ) -> Result<Vec<u32>, FsError> {
        let entry = self.file_table.get(fd).ok_or(FsError::InvalidFd(fd))?;
        let inode = self.inodes.get(&entry.inode_id).ok_or(FsError::NotFound("inode".into()))?;

        let start_block = offset / BLOCK_SIZE;
        let end_block = (offset + len.max(1) - 1) / BLOCK_SIZE;

        let mut sectors = Vec::new();
        for blk_idx in start_block..=end_block {
            if blk_idx < DIRECT_BLOCKS_COUNT {
                if let Some(sector) = inode.direct_blocks[blk_idx] {
                    sectors.push(sector);
                }
            } else if blk_idx < DIRECT_BLOCKS_COUNT + (BLOCK_SIZE / 4) {
                // Single indirect block pointer
                if let Some(indir_sector) = inode.single_indirect {
                    let entry_offset = (blk_idx - DIRECT_BLOCKS_COUNT) * 4;
                    if let Some(bytes) = self.device.read_bytes(indir_sector, entry_offset, 4) {
                        let sec = u32::from_le_bytes(bytes.try_into().unwrap_or([0; 4]));
                        if sec != 0 {
                            sectors.push(sec);
                        }
                    }
                }
            }
        }

        Ok(sectors)
    }

    fn write_blocks(
        &mut self,
        fd: FileDescriptor,
        offset: usize,
        data: &[u8],
    ) -> Result<Vec<u32>, FsError> {
        let entry = self.file_table.get(fd).ok_or(FsError::InvalidFd(fd))?;
        if !entry.writable {
            return Err(FsError::PermissionDenied);
        }
        let inode_id = entry.inode_id;

        let start_block = offset / BLOCK_SIZE;
        let end_block = (offset + data.len().max(1) - 1) / BLOCK_SIZE;

        let mut affected_sectors = Vec::new();

        for blk_idx in start_block..=end_block {
            if blk_idx < DIRECT_BLOCKS_COUNT {
                let sector = match self.inodes.get(&inode_id).and_then(|inod| inod.direct_blocks[blk_idx]) {
                    Some(s) => s,
                    None => {
                        let new_sec = self.allocate_data_block()?;
                        if let Some(inod) = self.inodes.get_mut(&inode_id) {
                            inod.direct_blocks[blk_idx] = Some(new_sec);
                        }
                        new_sec
                    }
                };

                let blk_offset = if blk_idx == start_block { offset % BLOCK_SIZE } else { 0 };
                let data_start = (blk_idx - start_block) * BLOCK_SIZE;
                let data_slice_end = (data_start + BLOCK_SIZE - blk_offset).min(data.len());

                if data_start < data.len() {
                    self.device.write_bytes(sector, blk_offset, &data[data_start..data_slice_end]);
                }
                affected_sectors.push(sector);
            }
        }

        // Update file size in inode
        if let Some(inod) = self.inodes.get_mut(&inode_id) {
            inod.file_size_bytes = inod.file_size_bytes.max((offset + data.len()) as u64);
        }

        Ok(affected_sectors)
    }
}
