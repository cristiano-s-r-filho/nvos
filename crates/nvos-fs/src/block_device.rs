//! Contiguous in-memory mock block storage device with sector bounds checking.

use crate::superblock::BLOCK_SIZE;

/// Simulated block storage device partitioned into 512-byte sectors.
#[derive(Debug)]
pub struct MockBlockDevice {
    total_sectors: u32,
    storage: Vec<u8>,
}

impl MockBlockDevice {
    /// Create a new block device with `total_sectors` sectors.
    #[must_use]
    pub fn new(total_sectors: u32) -> Self {
        let total_bytes = (total_sectors as usize).saturating_mul(BLOCK_SIZE);
        Self {
            total_sectors,
            storage: vec![0; total_bytes],
        }
    }

    /// Total sectors on the block device.
    #[must_use]
    pub fn total_sectors(&self) -> u32 {
        self.total_sectors
    }

    /// Read an entire sector (512 bytes) into a buffer.
    pub fn read_sector(&self, sector_id: u32, buf: &mut [u8; BLOCK_SIZE]) -> bool {
        if sector_id >= self.total_sectors {
            return false;
        }
        let offset = (sector_id as usize).saturating_mul(BLOCK_SIZE);
        buf.copy_from_slice(&self.storage[offset..offset + BLOCK_SIZE]);
        true
    }

    /// Write an entire sector (512 bytes) from a buffer.
    pub fn write_sector(&mut self, sector_id: u32, buf: &[u8; BLOCK_SIZE]) -> bool {
        if sector_id >= self.total_sectors {
            return false;
        }
        let offset = (sector_id as usize).saturating_mul(BLOCK_SIZE);
        self.storage[offset..offset + BLOCK_SIZE].copy_from_slice(buf);
        true
    }

    /// Read raw slice directly from a sector offset.
    #[must_use]
    pub fn read_bytes(&self, sector_id: u32, offset: usize, len: usize) -> Option<&[u8]> {
        if sector_id >= self.total_sectors || offset + len > BLOCK_SIZE {
            return None;
        }
        let base = (sector_id as usize).saturating_mul(BLOCK_SIZE) + offset;
        Some(&self.storage[base..base + len])
    }

    /// Write raw slice directly to a sector offset.
    pub fn write_bytes(&mut self, sector_id: u32, offset: usize, data: &[u8]) -> bool {
        if sector_id >= self.total_sectors || offset + data.len() > BLOCK_SIZE {
            return false;
        }
        let base = (sector_id as usize).saturating_mul(BLOCK_SIZE) + offset;
        self.storage[base..base + data.len()].copy_from_slice(data);
        true
    }
}
