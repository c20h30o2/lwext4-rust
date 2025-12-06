mod common;

use common::FileBlockDevice;
use lwext4_arce::{DummyHal, Ext4Filesystem, FsConfig};

#[test]
fn test_open_filesystem() {
    // 测试能否成功打开文件系统
    let test_image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/test.ext4";
    let device = FileBlockDevice::open(test_image).expect("Failed to open test image");

    let _fs = Ext4Filesystem::<DummyHal, _>::new(device, FsConfig::default())
        .expect("Failed to initialize filesystem");

    println!("✅ Successfully opened filesystem!");
}

// 更多测试可以在这里添加
// #[test]
// fn test_read_superblock() { ... }

#[test]
fn test_new_ext4filesystem() {
    let test_image = "/home/c20h30o2/files/lwext4-rust/lwext4-rust/test-images/test.ext4";
    let device = FileBlockDevice::open(test_image).expect("Failed to open test image");
    
}
