//! Superblock layout, inode counts, block sizes, and filesystem configuration.

pub const DEFAULT_TOTAL_BLOCKS: u32 = 4096; // 4096 * 512 bytes = 2 MiB storage
pub const DEFAULT_TOTAL_INODES: u32 = 256;
pub const BLOCK_SIZE: usize = 512;

/// Unix-style Filesystem Superblock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Superblock {
    pub magic: u32,
    pub block_size: u32,
    pub total_blocks: u32,
    pub free_blocks: u32,
    pub total_inodes: u32,
    pub free_inodes: u32,
    pub inode_table_start_block: u32,
    pub data_blocks_start_block: u32,
}

impl Default for Superblock {
    fn default() -> Self {
        Self {
            magic: 0x4E56_4F53, // "NVOS"
            block_size: BLOCK_SIZE as u32,
            total_blocks: DEFAULT_TOTAL_BLOCKS,
            free_blocks: DEFAULT_TOTAL_BLOCKS.saturating_sub(64),
            total_inodes: DEFAULT_TOTAL_INODES,
            free_inodes: DEFAULT_TOTAL_INODES.saturating_sub(1), // root directory inode reserved
            inode_table_start_block: 2,
            data_blocks_start_block: 64,
        }
    }
}
