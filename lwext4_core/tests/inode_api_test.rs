//! 测试基于 inode 编号的 VFS-style API
//!
//! 这些测试验证新实现的 inode-based API 是否正常工作

use lwext4_core::{BlockDevice, BlockDev, Ext4FileSystem, Error, Result};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};

/// 物理块大小（512 字节）
const PHYSICAL_BLOCK_SIZE: u32 = 512;

/// 逻辑块大小（4096 字节，典型的 ext4 块大小）
const LOGICAL_BLOCK_SIZE: u32 = 4096;

/// 测试镜像路径
const TEST_IMAGE: &str = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/test.ext4";

/// 用于测试的文件块设备
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
        let offset = lba * self.block_size() as u64;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| Error::new(lwext4_core::ErrorKind::Io, "seek failed"))?;

        self.file
            .read_exact(buf)
            .map_err(|e| Error::new(lwext4_core::ErrorKind::Io, "read failed"))?;

        Ok(buf.len())
    }

    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<usize> {
        let offset = lba * self.block_size() as u64;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| Error::new(lwext4_core::ErrorKind::Io, "seek failed"))?;

        self.file
            .write_all(buf)
            .map_err(|e| Error::new(lwext4_core::ErrorKind::Io, "write failed"))?;

        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.file
            .sync_all()
            .map_err(|e| Error::new(lwext4_core::ErrorKind::Io, "flush failed"))
    }
}

fn mount_test_fs() -> Result<Ext4FileSystem<FileBlockDevice>> {
    let device = FileBlockDevice::open(TEST_IMAGE)
        .expect("Failed to open test image");

    let bdev = BlockDev::new(device)?;
    Ext4FileSystem::mount(bdev)
}

#[test]
fn test_mount_and_stats() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    let stats = fs.stats().expect("Failed to get stats");
    println!("Free inodes: {}", stats.inodes_free);
    println!("Total inodes: {}", stats.inodes_total);

    assert!(stats.inodes_free > 10, "Should have free inodes");

    // Try creating a file
    println!("Attempting to create file...");
    let result = fs.create_file("/", "debug_test.txt", 0o644);
    println!("Create result: {:?}", result);

    if let Ok(inode) = result {
        println!("Successfully created file with inode {}", inode);
        fs.remove_file("/", "debug_test.txt").expect("Failed to clean up");
    }
}

#[test]
fn test_with_inode_ref() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    // Debug: Check stats before creating
    let stats = fs.stats().expect("Failed to get stats");
    println!("Before create - Free inodes: {}", stats.inodes_free);

    // 创建一个测试文件
    let inode_num = fs.create_file("/", "test_with_inode_ref.txt", 0o644)
        .expect("Failed to create file");

    // 使用 with_inode_ref 访问 inode
    let size = fs.with_inode_ref(inode_num, |inode_ref| {
        inode_ref.size()
    }).expect("Failed to access inode");

    assert_eq!(size, 0, "New file should have size 0");

    // 清理
    fs.remove_file("/", "test_with_inode_ref.txt").expect("Failed to remove file");
}

#[test]
fn test_read_write_at_inode() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    // 创建测试文件
    let inode_num = fs.create_file("/", "test_rw_inode.txt", 0o644)
        .expect("Failed to create file");

    // 写入数据
    let test_data = b"Hello from inode API!";
    let mut written = 0;
    while written < test_data.len() {
        let n = fs.write_at_inode(inode_num, &test_data[written..], written as u64)
            .expect("Failed to write");
        written += n;
    }

    // 读取数据
    let mut read_buf = vec![0u8; test_data.len()];
    let n = fs.read_at_inode(inode_num, &mut read_buf, 0)
        .expect("Failed to read");

    assert_eq!(n, test_data.len(), "Should read all data");
    assert_eq!(&read_buf[..], test_data, "Data should match");

    // 测试偏移读取
    let mut partial_buf = vec![0u8; 5];
    let n = fs.read_at_inode(inode_num, &mut partial_buf, 6)
        .expect("Failed to read with offset");

    assert_eq!(n, 5);
    assert_eq!(&partial_buf[..], b"from ", "Partial read should work");

    // 清理
    fs.remove_file("/", "test_rw_inode.txt").expect("Failed to remove file");
}

#[test]
fn test_get_inode_attr() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    // 创建测试文件
    let inode_num = fs.create_file("/", "test_attr.txt", 0o644)
        .expect("Failed to create file");

    // 获取属性
    let attr = fs.get_inode_attr(inode_num)
        .expect("Failed to get inode attributes");

    assert_eq!(attr.inode_num, inode_num);
    assert!(attr.file_type.is_file(), "Should be a file");
    assert_eq!(attr.size, 0, "New file should be empty");
    assert_eq!(attr.links_count, 1, "Should have 1 link");
    assert_eq!(attr.permissions & 0o777, 0o644, "Permissions should match");

    // 清理
    fs.remove_file("/", "test_attr.txt").expect("Failed to remove file");
}

#[test]
fn test_lookup_in_dir() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    // 创建测试文件
    let created_inode = fs.create_file("/", "test_lookup.txt", 0o644)
        .expect("Failed to create file");

    // 在根目录中查找
    let found_inode = fs.lookup_in_dir(2, "test_lookup.txt")
        .expect("Failed to lookup in directory");

    assert_eq!(found_inode, created_inode, "Found inode should match created inode");

    // 测试查找不存在的文件
    let result = fs.lookup_in_dir(2, "nonexistent.txt");
    assert!(result.is_err(), "Should fail to find nonexistent file");

    // 清理
    fs.remove_file("/", "test_lookup.txt").expect("Failed to remove file");
}

#[test]
fn test_create_in_dir() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    // 使用 create_in_dir 创建文件 (file_type=1 表示普通文件)
    let inode_num = fs.create_in_dir(2, "test_create_in_dir.txt", 1, 0o600)
        .expect("Failed to create in dir");

    // 验证文件已创建
    let attr = fs.get_inode_attr(inode_num)
        .expect("Failed to get attributes");

    assert!(attr.file_type.is_file(), "Should be a file");
    assert_eq!(attr.permissions & 0o777, 0o600, "Permissions should match");

    // 验证可以查找到
    let found = fs.lookup_in_dir(2, "test_create_in_dir.txt")
        .expect("Should find created file");
    assert_eq!(found, inode_num);

    // 测试创建目录 (file_type=2 表示目录)
    let dir_inode = fs.create_in_dir(2, "test_dir_create", 2, 0o755)
        .expect("Failed to create directory");

    let dir_attr = fs.get_inode_attr(dir_inode)
        .expect("Failed to get dir attributes");
    assert!(dir_attr.file_type.is_dir(), "Should be a directory");

    // 清理
    fs.remove_file("/", "test_create_in_dir.txt").expect("Failed to remove file");
    fs.remove_dir("/", "test_dir_create").expect("Failed to remove dir");
}

#[test]
fn test_read_dir_from_inode() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    // 创建几个测试文件
    fs.create_file("/", "test_dir_read_1.txt", 0o644)
        .expect("Failed to create file 1");
    fs.create_file("/", "test_dir_read_2.txt", 0o644)
        .expect("Failed to create file 2");

    // 读取根目录 (inode 2)
    let entries = fs.read_dir_from_inode(2)
        .expect("Failed to read directory");

    // 应该包含我们创建的文件
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"test_dir_read_1.txt"), "Should contain file 1");
    assert!(names.contains(&"test_dir_read_2.txt"), "Should contain file 2");

    // 清理
    fs.remove_file("/", "test_dir_read_1.txt").expect("Failed to remove file 1");
    fs.remove_file("/", "test_dir_read_2.txt").expect("Failed to remove file 2");
}

#[test]
fn test_unlink_from_dir() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    // 创建测试文件
    let inode_num = fs.create_file("/", "test_unlink.txt", 0o644)
        .expect("Failed to create file");

    // 使用 unlink_from_dir 删除
    let removed_inode = fs.unlink_from_dir(2, "test_unlink.txt")
        .expect("Failed to unlink");

    assert_eq!(removed_inode, inode_num, "Removed inode should match");

    // 验证文件已从目录中删除
    let result = fs.lookup_in_dir(2, "test_unlink.txt");
    assert!(result.is_err(), "File should no longer be in directory");

    // 注意：unlink_from_dir 不会减少链接计数或释放 inode
    // 这需要调用者手动处理
    let attr = fs.get_inode_attr(inode_num)
        .expect("Inode should still exist");
    assert_eq!(attr.links_count, 1, "Link count not automatically decreased");

    // 手动清理：减少链接计数并释放 inode
    fs.with_inode_ref(inode_num, |inode_ref| {
        inode_ref.with_inode_mut(|inode| {
            inode.links_count = 0u16.to_le();
        })?;
        inode_ref.mark_dirty()
    }).expect("Failed to update inode");

    fs.free_inode(inode_num, false).expect("Failed to free inode");
}

#[test]
fn test_complete_inode_workflow() {
    let mut fs = mount_test_fs().expect("Failed to mount filesystem");

    // 创建目录
    let dir_inode = fs.create_in_dir(2, "test_workflow_dir", 2, 0o755)
        .expect("Failed to create directory");

    // 在新目录中创建文件
    let file_inode = fs.create_in_dir(dir_inode, "file.txt", 1, 0o644)
        .expect("Failed to create file in dir");

    // 写入数据
    let data = b"Test workflow data";
    let mut written = 0;
    while written < data.len() {
        let n = fs.write_at_inode(file_inode, &data[written..], written as u64)
            .expect("Failed to write");
        written += n;
    }

    // 读取目录内容
    let entries = fs.read_dir_from_inode(dir_inode)
        .expect("Failed to read dir");

    let file_entry = entries.iter()
        .find(|e| e.name == "file.txt")
        .expect("Should find file.txt");
    assert_eq!(file_entry.inode, file_inode);

    // 读取文件数据
    let mut read_buf = vec![0u8; data.len()];
    let n = fs.read_at_inode(file_inode, &mut read_buf, 0)
        .expect("Failed to read");
    assert_eq!(n, data.len());
    assert_eq!(&read_buf[..], data);

    // 获取文件属性
    let attr = fs.get_inode_attr(file_inode)
        .expect("Failed to get attributes");
    assert_eq!(attr.size, data.len() as u64);

    // 清理 - 使用高级 API
    fs.remove_file(&format!("/test_workflow_dir"), "file.txt")
        .expect("Failed to remove file");
    fs.remove_dir("/", "test_workflow_dir")
        .expect("Failed to remove directory");
}
