//! 集成测试：验证 ArceOS wrapper 层的功能
//!
//! 这个测试模拟 ArceOS 环境，测试 wrapper 层的各项功能

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

/// 模拟 ArceOS 的 SystemHal
struct TestHal;

impl lwext4_core::SystemHal for TestHal {
    fn now() -> Option<std::time::Duration> {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
    }
}

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

// 不需要额外的 wrapper 模块，直接使用 lwext4_core

#[test]
fn test_wrapper_mount_and_stat() {
    println!("\n=== 测试 1: 挂载文件系统并获取统计信息 ===");

    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    let stats = fs.stats().expect("Failed to get stats");
    println!("块大小: {} bytes", stats.block_size);
    println!("总块数: {}", stats.blocks_total);
    println!("空闲块数: {}", stats.blocks_free);
    println!("总 inode 数: {}", stats.inodes_total);
    println!("空闲 inode 数: {}", stats.inodes_free);

    assert!(stats.block_size > 0);
    assert!(stats.blocks_total > 0);
}

#[test]
fn test_wrapper_read_root_dir() {
    println!("\n=== 测试 2: 读取根目录 ===");

    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    let entries = fs.read_dir_from_inode(2)
        .expect("Failed to read root directory");

    println!("根目录条目数: {}", entries.len());
    for entry in &entries {
        println!("  - {} (inode: {})", entry.name, entry.inode);
    }

    assert!(entries.len() >= 2); // 至少有 . 和 ..
}

#[test]
fn test_wrapper_create_and_write_file() {
    println!("\n=== 测试 3: 创建文件并写入数据 ===");

    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    // 创建文件
    use lwext4_core::dir::write::EXT4_DE_REG_FILE;
    let file_ino = fs.create_in_dir(2, "test_wrapper.txt", EXT4_DE_REG_FILE, 0o644)
        .expect("Failed to create file");

    println!("创建文件 test_wrapper.txt，inode: {}", file_ino);

    // 写入数据
    let test_data = b"Hello from ArceOS wrapper test!\n";
    let written = fs.write_at_inode(file_ino, test_data, 0)
        .expect("Failed to write data");

    println!("写入 {} 字节", written);
    assert_eq!(written, test_data.len());

    // 读取验证
    let mut read_buf = vec![0u8; test_data.len()];
    let read_size = fs.read_at_inode(file_ino, &mut read_buf, 0)
        .expect("Failed to read data");

    println!("读取 {} 字节", read_size);
    assert_eq!(read_size, test_data.len());
    assert_eq!(&read_buf[..], test_data);

    println!("✓ 数据验证成功");

    // 清理
    fs.unlink_from_dir(2, "test_wrapper.txt")
        .expect("Failed to unlink file");
    println!("✓ 文件已删除");
}

#[test]
fn test_wrapper_get_attr() {
    println!("\n=== 测试 4: 获取文件属性 ===");

    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    // 获取根目录属性
    let attr = fs.get_inode_attr(2)
        .expect("Failed to get root inode attributes");

    println!("根目录属性:");
    println!("  文件类型: {:?}", attr.file_type);
    println!("  权限: {:o}", attr.permissions);
    println!("  大小: {} bytes", attr.size);
    println!("  链接数: {}", attr.links_count);
    println!("  UID: {}, GID: {}", attr.uid, attr.gid);

    assert!(matches!(attr.file_type, lwext4_core::FileType::Directory));
}

#[test]
fn test_wrapper_lookup() {
    println!("\n=== 测试 5: 查找目录项 ===");

    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    // 创建测试文件
    use lwext4_core::dir::write::EXT4_DE_REG_FILE;
    let file_ino = fs.create_in_dir(2, "lookup_test.txt", EXT4_DE_REG_FILE, 0o644)
        .expect("Failed to create file");

    println!("创建测试文件，inode: {}", file_ino);

    // 查找
    let found_ino = fs.lookup_in_dir(2, "lookup_test.txt")
        .expect("Failed to lookup file");

    println!("查找到文件，inode: {}", found_ino);
    assert_eq!(found_ino, file_ino);

    // 清理
    fs.unlink_from_dir(2, "lookup_test.txt")
        .expect("Failed to unlink file");
    println!("✓ 测试通过");
}

#[test]
fn test_wrapper_rename() {
    println!("\n=== 测试 6: 重命名文件 ===");

    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    // 创建文件
    use lwext4_core::dir::write::EXT4_DE_REG_FILE;
    let file_ino = fs.create_in_dir(2, "old_name.txt", EXT4_DE_REG_FILE, 0o644)
        .expect("Failed to create file");

    println!("创建文件 old_name.txt，inode: {}", file_ino);

    // 重命名
    fs.rename_inode(2, "old_name.txt", 2, "new_name.txt")
        .expect("Failed to rename file");

    println!("重命名为 new_name.txt");

    // 验证旧名称不存在
    assert!(fs.lookup_in_dir(2, "old_name.txt").is_err());

    // 验证新名称存在
    let found_ino = fs.lookup_in_dir(2, "new_name.txt")
        .expect("Failed to lookup renamed file");
    assert_eq!(found_ino, file_ino);

    println!("✓ 重命名验证成功");

    // 清理
    fs.unlink_from_dir(2, "new_name.txt")
        .expect("Failed to unlink file");
}

#[test]
fn test_wrapper_truncate() {
    println!("\n=== 测试 7: 截断文件 ===");

    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    // 创建文件并写入数据
    use lwext4_core::dir::write::EXT4_DE_REG_FILE;
    let file_ino = fs.create_in_dir(2, "truncate_test.txt", EXT4_DE_REG_FILE, 0o644)
        .expect("Failed to create file");

    let test_data = b"This is a long test string that will be truncated";
    fs.write_at_inode(file_ino, test_data, 0)
        .expect("Failed to write data");

    println!("写入 {} 字节", test_data.len());

    // 截断到 20 字节
    fs.truncate_file(file_ino, 20)
        .expect("Failed to truncate file");

    println!("截断到 20 字节");

    // 验证大小
    let attr = fs.get_inode_attr(file_ino)
        .expect("Failed to get attributes");
    assert_eq!(attr.size, 20);

    println!("✓ 文件大小验证成功: {} bytes", attr.size);

    // 清理
    fs.unlink_from_dir(2, "truncate_test.txt")
        .expect("Failed to unlink file");
}

#[test]
fn test_wrapper_with_inode_ref() {
    println!("\n=== 测试 8: with_inode_ref 操作 ===");

    let device = FileBlockDevice::new("../test-images/test_clean.ext4")
        .expect("Failed to open test image");

    let bdev = lwext4_core::BlockDev::new(device)
        .expect("Failed to create BlockDev");

    let mut fs = lwext4_core::Ext4FileSystem::mount(bdev)
        .expect("Failed to mount filesystem");

    // 创建文件
    use lwext4_core::dir::write::EXT4_DE_REG_FILE;
    let file_ino = fs.create_in_dir(2, "inode_ref_test.txt", EXT4_DE_REG_FILE, 0o644)
        .expect("Failed to create file");

    // 使用 with_inode_ref 修改权限
    fs.with_inode_ref(file_ino, |inode_ref| {
        inode_ref.set_mode(0o755)?;
        inode_ref.set_owner(1000, 1000)?;
        Ok(())
    }).expect("Failed to modify inode");

    println!("✓ 通过 with_inode_ref 修改了权限和所有者");

    // 验证修改
    let attr = fs.get_inode_attr(file_ino)
        .expect("Failed to get attributes");
    println!("  新权限: {:o}", attr.permissions);
    println!("  新所有者: UID={}, GID={}", attr.uid, attr.gid);

    assert_eq!(attr.permissions, 0o755);
    assert_eq!(attr.uid, 1000);
    assert_eq!(attr.gid, 1000);

    // 清理
    fs.unlink_from_dir(2, "inode_ref_test.txt")
        .expect("Failed to unlink file");
}
