//! Inode data structure: metadata, direct block pointers, single and double indirect pointers.

use nvos_core::process::InodeId;

/// Number of direct block pointers in an inode.
pub const DIRECT_BLOCKS_COUNT: usize = 12;

/// Unix-style Inode structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inode {
    pub id: InodeId,
    pub is_directory: bool,
    pub file_size_bytes: u64,
    pub permissions: u16,
    pub links_count: u16,
    pub direct_blocks: [Option<u32>; DIRECT_BLOCKS_COUNT],
    pub single_indirect: Option<u32>,
    pub double_indirect: Option<u32>,
}

impl Inode {
    /// Create a new file inode with zero size and no blocks allocated.
    #[must_use]
    pub fn new_file(id: InodeId, permissions: u16) -> Self {
        Self {
            id,
            is_directory: false,
            file_size_bytes: 0,
            permissions,
            links_count: 1,
            direct_blocks: [None; DIRECT_BLOCKS_COUNT],
            single_indirect: None,
            double_indirect: None,
        }
    }

    /// Create a new directory inode.
    #[must_use]
    pub fn new_directory(id: InodeId, permissions: u16) -> Self {
        Self {
            id,
            is_directory: true,
            file_size_bytes: 0,
            permissions,
            links_count: 2, // '.' and parent
            direct_blocks: [None; DIRECT_BLOCKS_COUNT],
            single_indirect: None,
            double_indirect: None,
        }
    }
}
