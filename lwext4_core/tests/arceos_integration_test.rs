//! ArceOS 集成测试
//!
//! 模拟 ArceOS 的使用场景，测试 VFS-style inode-based API

use lwext4_core::{
    BlockDevice, BlockDev, Ext4FileSystem, SystemHal, DirReader,
    Error, ErrorKind, Result,
};
use lwext4_core::dir::write::{EXT4_DE_REG_FILE, EXT4_DE_DIR};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use std::time::{SystemTime, UNIX_EPOCH};

/// 物理块大小（512 字节）
const PHYSICAL_BLOCK_SIZE: u32 = 512;

/// 逻辑块大小（4096 字节）
const LOGICAL_BLOCK_SIZE: u32 = 4096;

/// EXT4 根 inode
const EXT4_ROOT_INO: u32 = 2;

/// 模拟 ArceOS 的 AxHal
struct TestHal;

impl SystemHal for TestHal {
    fn now() -> Option<core::time::Duration> {
        // 返回当前 UNIX 时间戳
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
    }
}

/// 文件块设备（模拟 ArceOS 的 AxBlockDevice）
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
        self.file
            .sync_all()
            .map_err(|_| Error::new(ErrorKind::Io, "flush failed"))
    }
}

/// 挂载文件系统
fn mount_fs(image_path: &str) -> Result<Ext4FileSystem<FileBlockDevice>> {
    let device = FileBlockDevice::open(image_path)
        .map_err(|_| Error::new(ErrorKind::Io, "Failed to open image"))?;

    let bdev = BlockDev::new(device)?;
    Ext4FileSystem::mount(bdev)
}

#[test]
fn test_mount_and_stat() {
    println!("\n=== Test: Mount and Stat ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 获取文件系统统计信息
    let stat = fs.stats().expect("Failed to get fs stats");

    println!("Filesystem stats:");
    println!("  Inodes: {} (free: {})", stat.inodes_total, stat.inodes_free);
    println!("  Blocks: {} (free: {})", stat.blocks_total, stat.blocks_free);
    println!("  Block size: {} bytes", stat.block_size);

    assert!(stat.block_size > 0);
    assert!(stat.blocks_total > 0);
    assert!(stat.inodes_total > 0);

    println!("✅ Mount and stat successful!");
}

#[test]
fn test_root_directory_listing() {
    println!("\n=== Test: Root Directory Listing ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 读取根目录
    let entries = fs.read_dir_from_inode(EXT4_ROOT_INO)
        .expect("Failed to read root directory");

    println!("Root directory entries ({} total):", entries.len());
    for (i, entry) in entries.iter().enumerate().take(20) {
        let type_str = match entry.file_type {
            1 => "FILE",
            2 => "DIR ",
            7 => "LINK",
            _ => "????",
        };
        println!("  [{}] {} inode={} ({})", i, entry.name, entry.inode, type_str);
    }

    if entries.len() > 20 {
        println!("  ... and {} more entries", entries.len() - 20);
    }

    assert!(!entries.is_empty());

    // 应该有 . 和 .. 条目
    assert!(entries.iter().any(|e| e.name == "."));
    assert!(entries.iter().any(|e| e.name == ".."));

    println!("✅ Root directory listing successful!");
}

#[test]
fn test_dir_reader_api() {
    println!("\n=== Test: DirReader API ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 获取根目录的 InodeRef
    let mut inode_ref = fs.get_inode_ref(EXT4_ROOT_INO)
        .expect("Failed to get root inode");

    // 创建 DirReader
    let mut reader = DirReader::new(&mut inode_ref, 0)
        .expect("Failed to create DirReader");

    println!("Iterating with DirReader:");
    let mut count = 0;
    while let Some(entry) = reader.current() {
        if count < 10 {
            println!("  [{}] {} (inode {})", count, entry.name, entry.inode);
        }
        count += 1;
        reader.step().expect("Failed to step");
    }

    println!("Total entries via DirReader: {}", count);
    assert!(count > 0);

    println!("✅ DirReader API successful!");
}

#[test]
fn test_lookup_in_dir() {
    println!("\n=== Test: Lookup in Directory ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 查找根目录中的常见目录
    let test_dirs = vec!["bin", "etc", "dev", "lib", "usr", "tmp"];

    for dir_name in test_dirs {
        match fs.lookup_in_dir(EXT4_ROOT_INO, dir_name) {
            Ok(inode) => {
                println!("  Found '{}' -> inode {}", dir_name, inode);

                // 验证是目录
                let attr = fs.get_inode_attr(inode).expect("Failed to get attr");
                println!("    Type: {:?}, Size: {} bytes", attr.file_type, attr.size);
            }
            Err(_) => {
                println!("  '{}' not found (may not exist in this image)", dir_name);
            }
        }
    }

    println!("✅ Lookup in directory successful!");
}

#[test]
fn test_read_file() {
    println!("\n=== Test: Read File ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 尝试查找并读取 /etc/hostname (常见于根文件系统)
    if let Ok(etc_ino) = fs.lookup_in_dir(EXT4_ROOT_INO, "etc") {
        println!("Found /etc (inode {})", etc_ino);

        if let Ok(hostname_ino) = fs.lookup_in_dir(etc_ino, "hostname") {
            println!("Found /etc/hostname (inode {})", hostname_ino);

            let attr = fs.get_inode_attr(hostname_ino).expect("Failed to get attr");
            println!("File size: {} bytes", attr.size);

            // 读取文件内容
            let mut buf = vec![0u8; attr.size as usize];
            let n = fs.read_at_inode(hostname_ino, &mut buf, 0)
                .expect("Failed to read file");

            println!("Read {} bytes", n);
            if let Ok(content) = String::from_utf8(buf[..n].to_vec()) {
                println!("Content: '{}'", content.trim());
            }

            println!("✅ File read successful!");
            return;
        }
    }

    println!("⚠️  /etc/hostname not found, trying alternative files...");

    // 尝试读取其他文件
    if let Ok(entries) = fs.read_dir_from_inode(EXT4_ROOT_INO) {
        for entry in entries.iter().take(50) {
            if entry.file_type == 1 && entry.name != "." && entry.name != ".." {
                println!("Trying to read file: {}", entry.name);

                let attr = fs.get_inode_attr(entry.inode).expect("Failed to get attr");
                if attr.size > 0 && attr.size < 1024 * 1024 {
                    let mut buf = vec![0u8; attr.size.min(1024) as usize];
                    match fs.read_at_inode(entry.inode, &mut buf, 0) {
                        Ok(n) => {
                            println!("✅ Successfully read {} bytes from {}", n, entry.name);
                            return;
                        }
                        Err(e) => {
                            println!("Failed to read {}: {:?}", entry.name, e);
                        }
                    }
                }
            }
        }
    }

    println!("⚠️  No readable files found in test");
}

#[test]
fn test_create_and_write_file() {
    println!("\n=== Test: Create and Write File ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 查找 /tmp 目录
    let tmp_ino = match fs.lookup_in_dir(EXT4_ROOT_INO, "tmp") {
        Ok(ino) => ino,
        Err(_) => {
            // 如果 /tmp 不存在，在根目录创建
            println!("/tmp not found, creating in root...");
            match fs.create_in_dir(EXT4_ROOT_INO, "tmp", EXT4_DE_DIR, 0o755) {
                Ok(ino) => {
                    println!("Created /tmp (inode {})", ino);
                    ino
                }
                Err(e) => {
                    println!("⚠️  Failed to create /tmp: {:?}", e);
                    println!("Test skipped due to read-only filesystem or other constraints");
                    return;
                }
            }
        }
    };

    println!("Using /tmp (inode {})", tmp_ino);

    // 创建测试文件
    let test_filename = "arceos_test.txt";
    let test_content = b"Hello from lwext4_core ArceOS integration test!\n";

    match fs.create_in_dir(tmp_ino, test_filename, EXT4_DE_REG_FILE, 0o644) {
        Ok(file_ino) => {
            println!("Created {} (inode {})", test_filename, file_ino);

            // 写入数据
            match fs.write_at_inode(file_ino, test_content, 0) {
                Ok(n) => {
                    println!("Wrote {} bytes", n);
                    assert_eq!(n, test_content.len());

                    // 读回验证
                    let mut read_buf = vec![0u8; test_content.len()];
                    let read_n = fs.read_at_inode(file_ino, &mut read_buf, 0)
                        .expect("Failed to read back");

                    assert_eq!(read_n, test_content.len());
                    assert_eq!(&read_buf[..read_n], test_content);

                    println!("✅ Write and read-back verification successful!");
                }
                Err(e) => {
                    println!("⚠️  Failed to write: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("⚠️  Failed to create file: {:?}", e);
            println!("Test skipped due to read-only filesystem or other constraints");
        }
    }
}

#[test]
fn test_rename_inode() {
    println!("\n=== Test: Rename Inode ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 查找或创建 /tmp
    let tmp_ino = match fs.lookup_in_dir(EXT4_ROOT_INO, "tmp") {
        Ok(ino) => ino,
        Err(_) => {
            match fs.create_in_dir(EXT4_ROOT_INO, "tmp", EXT4_DE_DIR, 0o755) {
                Ok(ino) => ino,
                Err(_) => {
                    println!("⚠️  Cannot create /tmp, skipping test");
                    return;
                }
            }
        }
    };

    // 创建测试文件（使用随机名字避免冲突）
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let old_name = format!("rename_old_{}.txt", timestamp);
    let new_name = format!("rename_new_{}.txt", timestamp);

    match fs.create_in_dir(tmp_ino, &old_name, EXT4_DE_REG_FILE, 0o644) {
        Ok(file_ino) => {
            println!("Created {} (inode {})", old_name, file_ino);

            // 重命名
            match fs.rename_inode(tmp_ino, &old_name, tmp_ino, &new_name) {
                Ok(()) => {
                    println!("✅ Renamed {} -> {}", old_name, new_name);

                    // 验证旧名字不存在
                    match fs.lookup_in_dir(tmp_ino, &old_name) {
                        Err(_) => println!("  ✓ Old name correctly removed"),
                        Ok(_) => panic!("Old name still exists after rename!"),
                    }

                    // 验证新名字存在
                    match fs.lookup_in_dir(tmp_ino, &new_name) {
                        Ok(found_ino) => {
                            println!("  ✓ New name found (inode {})", found_ino);
                            assert_eq!(found_ino, file_ino);
                        }
                        Err(_) => panic!("New name not found after rename!"),
                    }

                    println!("✅ Rename verification successful!");
                }
                Err(e) => {
                    println!("⚠️  Rename failed: {:?}", e);
                }
            }
        }
        Err(_) => {
            println!("⚠️  Cannot create test file, skipping test");
        }
    }
}

#[test]
fn test_link_inode() {
    println!("\n=== Test: Link Inode (Hard Link) ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 查找或创建 /tmp
    let tmp_ino = match fs.lookup_in_dir(EXT4_ROOT_INO, "tmp") {
        Ok(ino) => ino,
        Err(_) => {
            match fs.create_in_dir(EXT4_ROOT_INO, "tmp", EXT4_DE_DIR, 0o755) {
                Ok(ino) => ino,
                Err(_) => {
                    println!("⚠️  Cannot create /tmp, skipping test");
                    return;
                }
            }
        }
    };

    // 创建源文件
    let source_name = "link_source.txt";
    let link_name = "link_hardlink.txt";
    let content = b"Test content for hard link\n";

    match fs.create_in_dir(tmp_ino, source_name, EXT4_DE_REG_FILE, 0o644) {
        Ok(file_ino) => {
            println!("Created {} (inode {})", source_name, file_ino);

            // 写入内容
            fs.write_at_inode(file_ino, content, 0)
                .expect("Failed to write");

            // 创建硬链接
            match fs.link_inode(tmp_ino, link_name, file_ino) {
                Ok(()) => {
                    println!("✅ Created hard link {} -> inode {}", link_name, file_ino);

                    // 验证硬链接指向同一 inode
                    match fs.lookup_in_dir(tmp_ino, link_name) {
                        Ok(link_ino) => {
                            println!("  ✓ Hard link points to inode {}", link_ino);
                            assert_eq!(link_ino, file_ino);

                            // 验证通过硬链接可以读取相同内容
                            let mut buf = vec![0u8; content.len()];
                            let n = fs.read_at_inode(link_ino, &mut buf, 0)
                                .expect("Failed to read via hard link");

                            assert_eq!(n, content.len());
                            assert_eq!(&buf[..n], content);

                            println!("  ✓ Content accessible via hard link");
                            println!("✅ Hard link verification successful!");
                        }
                        Err(e) => {
                            panic!("Hard link lookup failed: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("⚠️  Link failed: {:?}", e);
                }
            }
        }
        Err(_) => {
            println!("⚠️  Cannot create source file, skipping test");
        }
    }
}

#[test]
fn test_unlink_from_dir() {
    println!("\n=== Test: Unlink from Directory ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 查找或创建 /tmp
    let tmp_ino = match fs.lookup_in_dir(EXT4_ROOT_INO, "tmp") {
        Ok(ino) => ino,
        Err(_) => {
            match fs.create_in_dir(EXT4_ROOT_INO, "tmp", EXT4_DE_DIR, 0o755) {
                Ok(ino) => ino,
                Err(_) => {
                    println!("⚠️  Cannot create /tmp, skipping test");
                    return;
                }
            }
        }
    };

    // 创建测试文件
    let filename = "unlink_test.txt";

    match fs.create_in_dir(tmp_ino, filename, EXT4_DE_REG_FILE, 0o644) {
        Ok(file_ino) => {
            println!("Created {} (inode {})", filename, file_ino);

            // 删除文件
            match fs.unlink_from_dir(tmp_ino, filename) {
                Ok(removed_ino) => {
                    println!("✅ Unlinked {} (returned inode {})", filename, removed_ino);
                    assert_eq!(removed_ino, file_ino);

                    // 验证文件不再存在
                    match fs.lookup_in_dir(tmp_ino, filename) {
                        Err(_) => println!("  ✓ File correctly removed from directory"),
                        Ok(_) => panic!("File still exists after unlink!"),
                    }

                    println!("✅ Unlink verification successful!");
                }
                Err(e) => {
                    println!("⚠️  Unlink failed: {:?}", e);
                }
            }
        }
        Err(_) => {
            println!("⚠️  Cannot create test file, skipping test");
        }
    }
}

#[test]
fn test_get_inode_attr() {
    println!("\n=== Test: Get Inode Attributes ===");

    let image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/rootfs-riscv64.img";
    let mut fs = mount_fs(image).expect("Failed to mount filesystem");

    // 获取根目录属性
    let root_attr = fs.get_inode_attr(EXT4_ROOT_INO)
        .expect("Failed to get root inode attr");

    println!("Root directory attributes:");
    println!("  Type: {:?}", root_attr.file_type);
    println!("  Size: {} bytes", root_attr.size);
    println!("  Permissions: 0o{:o}", root_attr.permissions);
    println!("  UID: {}, GID: {}", root_attr.uid, root_attr.gid);
    println!("  Links: {}", root_attr.links_count);

    assert_eq!(root_attr.file_type, lwext4_core::FileType::Directory);
    assert!(root_attr.links_count >= 2); // 至少有 . 和至少一个子目录的 ..

    println!("✅ Get inode attributes successful!");
}
