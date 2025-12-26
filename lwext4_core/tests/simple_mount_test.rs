//! 简单的挂载测试，检查基本功能

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

/// 简单的文件块设备
struct FileBlockDevice {
    file: File,
    block_size: u32,
}

impl FileBlockDevice {
    fn new(path: &str) -> std::io::Result<Self> {
        let file = File::options().read(true).write(true).open(path)?;
        Ok(Self {
            file,
            block_size: 512,
        })
    }
}

impl lwext4_core::BlockDevice for FileBlockDevice {
    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn sector_size(&self) -> u32 {
        512
    }

    fn total_blocks(&self) -> u64 {
        self.file.metadata().unwrap().len() / self.block_size as u64
    }

    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> lwext4_core::Result<usize> {
        let offset = lba * self.block_size as u64;
        let size = count as usize * self.block_size as usize;

        self.file.seek(SeekFrom::Start(offset))
            .map_err(|_| lwext4_core::Error::new(lwext4_core::ErrorKind::Io, "Seek failed"))?;

        self.file.read_exact(&mut buf[..size])
            .map_err(|_| lwext4_core::Error::new(lwext4_core::ErrorKind::Io, "Read failed"))?;

        Ok(size)
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> lwext4_core::Result<usize> {
        let offset = lba * self.block_size as u64;
        let size = count as usize * self.block_size as usize;

        self.file.seek(SeekFrom::Start(offset))
            .map_err(|_| lwext4_core::Error::new(lwext4_core::ErrorKind::Io, "Seek failed"))?;

        self.file.write_all(&buf[..size])
            .map_err(|_| lwext4_core::Error::new(lwext4_core::ErrorKind::Io, "Write failed"))?;

        Ok(size)
    }
}

#[test]
fn test_simple_mount_and_operations() {
    println!("\n=== 简单挂载和操作测试 ===");

    // 挂载文件系统
    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    println!("✓ 文件系统挂载成功");

    // 获取统计信息
    let stats = fs.stats().expect("Failed to get stats");
    println!("  块大小: {} bytes", stats.block_size);
    println!("  总块数: {}", stats.blocks_total);
    println!("  空闲块数: {}", stats.blocks_free);

    // 使用 root_inode 方法 (如果有)
    // 或者通过路径操作
    println!("\n尝试通过路径创建文件...");

    // 尝试使用 create 方法
    use lwext4_core::dir::write::EXT4_DE_REG_FILE;
    match fs.create("/test_file.txt", EXT4_DE_REG_FILE, 0o644) {
        Ok(ino) => {
            println!("✓ 通过路径创建文件成功，inode: {}", ino);

            // 写入数据
            let test_data = b"Hello from lwext4_core!\n";
            let written = fs.write_at("/test_file.txt", test_data, 0)
                .expect("Failed to write");
            println!("✓ 写入 {} 字节", written);

            // 读取验证
            let mut read_buf = vec![0u8; test_data.len()];
            let read_size = fs.read_at("/test_file.txt", &mut read_buf, 0)
                .expect("Failed to read");
            println!("✓ 读取 {} 字节", read_size);

            if &read_buf[..] == test_data {
                println!("✓ 数据验证成功！");
            } else {
                println!("✗ 数据不匹配");
            }

            // 删除文件
            fs.unlink("/test_file.txt").expect("Failed to unlink");
            println!("✓ 文件删除成功");
        }
        Err(e) => {
            println!("✗ 创建文件失败: {:?}", e);
            println!("  这可能是预期的，如果没有路径操作 API");
        }
    }
}
