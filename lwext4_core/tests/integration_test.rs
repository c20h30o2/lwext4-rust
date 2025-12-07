//! 集成测试 - 测试 lwext4_core 的 Rust API

use lwext4_core::{BlockDevice, Ext4BlockDev, Ext4Error, Ext4Result};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};

/// 物理块大小（512 字节）
const PHYSICAL_BLOCK_SIZE: u32 = 512;

/// 逻辑块大小（4096 字节，典型的 ext4 块大小）
const LOGICAL_BLOCK_SIZE: u32 = 4096;

/// 用于测试的文件块设备
struct FileBlockDevice {
    file: File,
    total_size: u64,
}

impl FileBlockDevice {
    pub fn open(path: &str) -> std::io::Result<Self> {
        let file = File::options().read(true).write(true).open(path)?;
        let total_size = file.metadata()?.len();
        Ok(Self {
            file,
            total_size,
        })
    }
}

impl BlockDevice for FileBlockDevice {
    fn block_size(&self) -> u32 {
        LOGICAL_BLOCK_SIZE
    }

    fn physical_block_size(&self) -> u32 {
        PHYSICAL_BLOCK_SIZE
    }

    fn total_blocks(&self) -> u64 {
        self.total_size / self.block_size() as u64
    }

    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Ext4Result<usize> {
        let offset = lba * self.physical_block_size() as u64;
        let size = count as usize * self.physical_block_size() as usize;

        if buf.len() < size {
            return Err(Ext4Error::new(22, "buffer too small")); // EINVAL
        }

        self.file.seek(SeekFrom::Start(offset))
            .map_err(|_| Ext4Error::new(5, "seek failed"))?; // EIO

        self.file.read_exact(&mut buf[..size])
            .map_err(|_| Ext4Error::new(5, "read failed"))?; // EIO

        Ok(size)
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Ext4Result<usize> {
        let offset = lba * self.physical_block_size() as u64;
        let size = count as usize * self.physical_block_size() as usize;

        if buf.len() < size {
            return Err(Ext4Error::new(22, "buffer too small")); // EINVAL
        }

        self.file.seek(SeekFrom::Start(offset))
            .map_err(|_| Ext4Error::new(5, "seek failed"))?; // EIO

        self.file.write_all(&buf[..size])
            .map_err(|_| Ext4Error::new(5, "write failed"))?; // EIO

        Ok(size)
    }

    fn flush(&mut self) -> Ext4Result<()> {
        self.file.sync_all()
            .map_err(|_| Ext4Error::new(5, "flush failed")) // EIO
    }
}

#[test]
fn test_block_device_creation() {
    let test_image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/test.ext4";
    let device = FileBlockDevice::open(test_image).expect("Failed to open test image");

    let bdev = Ext4BlockDev::new(device);

    assert_eq!(bdev.lg_bsize(), LOGICAL_BLOCK_SIZE);
    assert_eq!(bdev.ph_bsize(), PHYSICAL_BLOCK_SIZE);
    assert!(bdev.lg_bcnt() > 0);

    println!("✅ Block device created successfully!");
}

#[test]
fn test_block_read_write() {
    let test_image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/test.ext4";
    let device = FileBlockDevice::open(test_image).expect("Failed to open test image");

    let mut bdev = Ext4BlockDev::new(device);

    // 测试读取第一个块
    let mut buf = vec![0u8; LOGICAL_BLOCK_SIZE as usize];
    let result = bdev.ext4_blocks_get_direct(0, &mut buf);

    assert!(result.is_ok(), "Failed to read block: {:?}", result.err());

    let bytes_read = result.unwrap();
    assert!(bytes_read > 0);

    println!("✅ Successfully read {} bytes from block 0", bytes_read);
}

#[test]
fn test_byte_level_read() {
    let test_image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/test.ext4";
    let device = FileBlockDevice::open(test_image).expect("Failed to open test image");

    let mut bdev = Ext4BlockDev::new(device);

    // 测试字节级读取
    let mut buf = vec![0u8; 100];
    let result = bdev.ext4_block_readbytes(1024, &mut buf);

    assert!(result.is_ok(), "Failed to read bytes: {:?}", result.err());

    let bytes_read = result.unwrap();
    assert_eq!(bytes_read, 100);

    println!("✅ Successfully read {} bytes from offset 1024", bytes_read);
}

#[test]
fn test_statistics() {
    let test_image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/test.ext4";
    let device = FileBlockDevice::open(test_image).expect("Failed to open test image");

    let mut bdev = Ext4BlockDev::new(device);

    assert_eq!(bdev.bread_ctr(), 0);
    assert_eq!(bdev.bwrite_ctr(), 0);

    // 执行一次读取
    let mut buf = vec![0u8; LOGICAL_BLOCK_SIZE as usize];
    let _ = bdev.ext4_blocks_get_direct(0, &mut buf);

    // 验证计数器增加
    assert_eq!(bdev.bread_ctr(), 1);
    assert_eq!(bdev.bwrite_ctr(), 0);

    println!("✅ Statistics tracking works correctly!");
}
