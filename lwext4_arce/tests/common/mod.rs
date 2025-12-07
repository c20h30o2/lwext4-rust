use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use lwext4_arce::{BlockDevice, Ext4Result, Ext4Error};

/// 物理块大小（512 字节）
const PHYSICAL_BLOCK_SIZE: u32 = 512;

/// 逻辑块大小（4096 字节，典型的 ext4 块大小）
const LOGICAL_BLOCK_SIZE: u32 = 4096;

pub struct FileBlockDevice {
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

        // 检查缓冲区大小
        if buf.len() < size {
            return Err(Ext4Error::new(libc::EINVAL, "buffer too small"));
        }

        self.file.seek(SeekFrom::Start(offset))
            .map_err(|_| Ext4Error::new(libc::EIO, "seek failed"))?;

        self.file.read_exact(&mut buf[..size])
            .map_err(|_| Ext4Error::new(libc::EIO, "read failed"))?;

        Ok(size)
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Ext4Result<usize> {
        let offset = lba * self.physical_block_size() as u64;
        let size = count as usize * self.physical_block_size() as usize;

        // 检查缓冲区大小
        if buf.len() < size {
            return Err(Ext4Error::new(libc::EINVAL, "buffer too small"));
        }

        self.file.seek(SeekFrom::Start(offset))
            .map_err(|_| Ext4Error::new(libc::EIO, "seek failed"))?;

        self.file.write_all(&buf[..size])
            .map_err(|_| Ext4Error::new(libc::EIO, "write failed"))?;

        Ok(size)
    }

    fn flush(&mut self) -> Ext4Result<()> {
        self.file.sync_all()
            .map_err(|_| Ext4Error::new(libc::EIO, "flush failed"))
    }
}
