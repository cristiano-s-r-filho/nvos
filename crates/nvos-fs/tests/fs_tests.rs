use nvos_core::process::Pid;
use nvos_fs::{FileSystem, FsError, InodeFileSystem};

#[test]
fn test_create_open_write_resolve_blocks() {
    let mut fs = InodeFileSystem::new();
    let pid = Pid(1);

    // Create file
    let inode_id = fs.create_file("/etc/passwd", 0o644).unwrap();
    assert_eq!(inode_id.0, 1);

    // Open file
    let fd = fs.open(pid, "/etc/passwd", true).unwrap();
    assert_eq!(fd.0, 1);

    // Write 1000 bytes (spans 2 sectors: 512 + 488)
    let payload = vec![0xAB; 1000];
    let sectors = fs.write_blocks(fd, 0, &payload).unwrap();
    assert_eq!(sectors.len(), 2);

    // Resolve read sectors
    let read_sectors = fs.resolve_read_blocks(fd, 0, 1000).unwrap();
    assert_eq!(read_sectors, sectors);

    // Read back bytes directly from block device to verify data integrity
    let s0_bytes = fs.block_device().read_bytes(sectors[0], 0, 512).unwrap();
    assert_eq!(s0_bytes, &payload[0..512]);

    let s1_bytes = fs.block_device().read_bytes(sectors[1], 0, 488).unwrap();
    assert_eq!(s1_bytes, &payload[512..1000]);

    // Close file
    fs.close(pid, fd).unwrap();
    assert!(matches!(fs.resolve_read_blocks(fd, 0, 100), Err(FsError::InvalidFd(_))));
}

#[test]
fn test_read_only_permission_denied() {
    let mut fs = InodeFileSystem::new();
    let pid = Pid(2);

    fs.create_file("/readonly.txt", 0o444).unwrap();
    let fd = fs.open(pid, "/readonly.txt", false).unwrap(); // open read-only

    let err = fs.write_blocks(fd, 0, b"data").unwrap_err();
    assert_eq!(err, FsError::PermissionDenied);
}

#[test]
fn test_file_not_found() {
    let mut fs = InodeFileSystem::new();
    let err = fs.open(Pid(3), "/nonexistent", true).unwrap_err();
    assert!(matches!(err, FsError::NotFound(_)));
}
