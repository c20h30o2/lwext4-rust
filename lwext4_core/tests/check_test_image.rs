//! 检查测试镜像内容

use lwext4_core::{BlockDevice, BlockDev, Ext4FileSystem, Result};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};

const PHYSICAL_BLOCK_SIZE: u32 = 512;
const LOGICAL_BLOCK_SIZE: u32 = 4096;
const TEST_IMAGE: &str = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/test.ext4";

struct FileBlockDevice {
    file: File,
    total_size: u64,
}

impl FileBlockDevice {
    pub fn open(path: &str) -> std::io::Result<Self> {
        let file = File::options().read(true).write(true).open(path)?;
        let total_size = file.metadata()?.len();
        Ok(Self { file, total_size })
    }
}

impl BlockDevice for FileBlockDevice {
    fn block_size(&self) -> u32 {
        LOGICAL_BLOCK_SIZE
    }

    fn sector_size(&self) -> u32 {
        PHYSICAL_BLOCK_SIZE
    }

    fn total_blocks(&self) -> u64 {
        self.total_size / self.block_size() as u64
    }

    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<usize> {
        use lwext4_core::{Error, ErrorKind};
        let offset = lba * self.sector_size() as u64;
        let size = count as usize * self.sector_size() as usize;

        if buf.len() < size {
            return Err(Error::new(ErrorKind::InvalidInput, "buffer too small"));
        }

        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| Error::new(ErrorKind::Io, "seek failed"))?;

        self.file
            .read_exact(&mut buf[..size])
            .map_err(|_| Error::new(ErrorKind::Io, "read failed"))?;

        Ok(size)
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<usize> {
        use lwext4_core::{Error, ErrorKind};
        let offset = lba * self.sector_size() as u64;
        let size = count as usize * self.sector_size() as usize;

        if buf.len() < size {
            return Err(Error::new(ErrorKind::InvalidInput, "buffer too small"));
        }

        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| Error::new(ErrorKind::Io, "seek failed"))?;

        self.file
            .write_all(&buf[..size])
            .map_err(|_| Error::new(ErrorKind::Io, "write failed"))?;

        Ok(size)
    }

    fn flush(&mut self) -> Result<()> {
        use lwext4_core::Error;
        use lwext4_core::ErrorKind;
        self.file
            .sync_all()
            .map_err(|_| Error::new(ErrorKind::Io, "flush failed"))
    }
}

#[test]
fn check_root_directory() {
    let device = FileBlockDevice::open(TEST_IMAGE).expect("Failed to open test image");
    let bdev = BlockDev::new(device).expect("Failed to create BlockDev");
    let mut fs = Ext4FileSystem::mount(bdev).expect("Failed to mount filesystem");

    println!("=== 检查根目录内容 ===");

    match fs.read_dir("/") {
        Ok(entries) => {
            println!("根目录包含 {} 个条目:", entries.len());
            for entry in entries {
                println!("  - {} (inode: {}, type: {:?})",
                         entry.name, entry.inode, entry.file_type);
            }
        }
        Err(e) => {
            println!("读取根目录失败: {:?}", e);
        }
    }

    // 尝试创建 /tmp 目录
    println!("\n=== 尝试创建 /tmp 目录 ===");
    match fs.create_dir("/", "tmp", 0o755) {
        Ok(inode) => {
            println!("成功创建 /tmp 目录，inode: {}", inode);
        }
        Err(e) => {
            println!("创建 /tmp 目录失败: {:?}", e);
            println!("（可能已存在）");
        }
    }

    // 再次检查
    println!("\n=== 再次检查根目录 ===");
    match fs.read_dir("/") {
        Ok(entries) => {
            println!("根目录包含 {} 个条目:", entries.len());
            for entry in entries {
                println!("  - {} (inode: {}, type: {:?})",
                         entry.name, entry.inode, entry.file_type);
            }
        }
        Err(e) => {
            println!("读取根目录失败: {:?}", e);
        }
    }
}
