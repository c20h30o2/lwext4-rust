//! Journal (JBD2) 集成测试

use lwext4_core::journal::types::*;
use lwext4_core::journal::checksum;

#[test]
fn test_journal_superblock_structure() {
    // 验证 journal superblock 结构大小
    assert_eq!(
        core::mem::size_of::<jbd_sb>(),
        1024,
        "Journal superblock must be exactly 1024 bytes"
    );
}

#[test]
fn test_block_header_creation() {
    // 测试块头创建
    let header = jbd_bhdr::new(JBD_DESCRIPTOR_BLOCK, 100);

    assert!(header.verify_magic());
    assert_eq!(header.get_blocktype(), JBD_DESCRIPTOR_BLOCK);
    assert_eq!(header.get_sequence(), 100);
}

#[test]
fn test_journal_superblock_validation() {
    // 创建有效的 journal superblock
    let mut sb = jbd_sb::default();
    sb.header.magic = JBD_MAGIC_NUMBER.to_be();
    sb.header.blocktype = JBD_SUPERBLOCK_V2.to_be();
    sb.blocksize = 4096u32.to_be();
    sb.maxlen = 1024u32.to_be();
    sb.first = 1u32.to_be();
    sb.sequence = 1u32.to_be();
    sb.start = 1u32.to_be();

    assert!(sb.is_valid(), "Valid journal superblock should pass validation");
}

#[test]
fn test_journal_superblock_features() {
    let mut sb = jbd_sb::default();
    sb.header.magic = JBD_MAGIC_NUMBER.to_be();
    sb.header.blocktype = JBD_SUPERBLOCK_V2.to_be();
    sb.feature_incompat = JBD_FEATURE_INCOMPAT_CSUM_V3.to_be();

    assert!(sb.has_incompat_feature(JBD_FEATURE_INCOMPAT_CSUM_V3));
    assert!(!sb.has_incompat_feature(JBD_FEATURE_INCOMPAT_64BIT));
    assert_eq!(sb.checksum_version(), 3);
}

#[test]
fn test_journal_superblock_checksum() {
    let mut sb = jbd_sb::default();
    sb.header.magic = JBD_MAGIC_NUMBER.to_be();
    sb.header.blocktype = JBD_SUPERBLOCK_V2.to_be();
    sb.blocksize = 4096u32.to_be();
    sb.feature_incompat = JBD_FEATURE_INCOMPAT_CSUM_V3.to_be();

    // 计算校验和
    checksum::calculate_superblock_csum(&mut sb);
    assert_ne!(sb.checksum, 0, "Checksum should be non-zero");

    // 验证校验和
    assert!(
        checksum::verify_superblock_csum(&sb),
        "Checksum verification should pass"
    );

    // 修改数据应该导致校验和失败
    let mut sb_modified = sb;
    sb_modified.blocksize = 8192u32.to_be();
    assert!(
        !checksum::verify_superblock_csum(&sb_modified),
        "Modified superblock should fail checksum verification"
    );
}

#[test]
fn test_block_tag_structure() {
    // 验证 block tag 结构大小
    assert_eq!(
        core::mem::size_of::<jbd_block_tag>(),
        12,
        "jbd_block_tag must be 12 bytes"
    );
    assert_eq!(
        core::mem::size_of::<jbd_block_tag3>(),
        16,
        "jbd_block_tag3 must be 16 bytes"
    );
}

#[test]
fn test_commit_header_structure() {
    // 验证 commit header 结构大小
    assert_eq!(
        core::mem::size_of::<jbd_commit_header>(),
        60,
        "jbd_commit_header must be 60 bytes"
    );
}

#[test]
fn test_tail_structures() {
    // 验证 tail 结构大小
    assert_eq!(
        core::mem::size_of::<jbd_block_tail>(),
        4,
        "jbd_block_tail must be 4 bytes"
    );
    assert_eq!(
        core::mem::size_of::<jbd_revoke_tail>(),
        4,
        "jbd_revoke_tail must be 4 bytes"
    );
}

#[test]
fn test_checksum_functions() {
    let uuid = [0u8; 16];
    let data = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let sequence = 100;

    // 测试基本校验和计算
    let csum = checksum::block_csum(&uuid, &data, sequence);
    assert_ne!(csum, 0, "Checksum should be non-zero");

    // 相同输入应该产生相同输出
    let csum2 = checksum::block_csum(&uuid, &data, sequence);
    assert_eq!(csum, csum2, "Checksum should be deterministic");

    // 不同输入应该产生不同输出
    let different_data = [9u8, 10, 11, 12, 13, 14, 15, 16];
    let csum3 = checksum::block_csum(&uuid, &different_data, sequence);
    assert_ne!(csum, csum3, "Different data should produce different checksum");
}

#[test]
fn test_descriptor_block_checksum() {
    let uuid = [0u8; 16];
    let sequence = 50;

    // 创建一个模拟的 descriptor block
    let mut block_data = vec![0u8; 4096];

    // 写入 header
    let header = jbd_bhdr::new(JBD_DESCRIPTOR_BLOCK, sequence);
    unsafe {
        core::ptr::write_unaligned(
            block_data.as_mut_ptr() as *mut jbd_bhdr,
            header,
        );
    }

    // 计算校验和
    let tail_offset = block_data.len() - core::mem::size_of::<jbd_block_tail>();
    let csum = checksum::calculate_descriptor_csum(&uuid, &block_data[..tail_offset]);

    // 写入 tail
    let tail = jbd_block_tail {
        checksum: csum.to_be(),
    };
    unsafe {
        core::ptr::write_unaligned(
            block_data.as_mut_ptr().add(tail_offset) as *mut jbd_block_tail,
            tail,
        );
    }

    // 验证校验和
    assert!(
        checksum::verify_descriptor_block(&uuid, &block_data),
        "Descriptor block checksum should verify correctly"
    );
}

#[test]
fn test_revoke_block_checksum() {
    let uuid = [1u8; 16]; // 使用不同的 UUID
    let sequence = 75;

    // 创建一个模拟的 revoke block
    let mut block_data = vec![0u8; 4096];

    // 写入 header
    let header = jbd_revoke_header {
        header: jbd_bhdr::new(JBD_REVOKE_BLOCK, sequence),
        count: 0u32.to_be(),
    };
    unsafe {
        core::ptr::write_unaligned(
            block_data.as_mut_ptr() as *mut jbd_revoke_header,
            header,
        );
    }

    // 计算校验和
    let tail_offset = block_data.len() - core::mem::size_of::<jbd_revoke_tail>();
    let csum = checksum::calculate_revoke_csum(&uuid, &block_data[..tail_offset]);

    // 写入 tail
    let tail = jbd_revoke_tail {
        checksum: csum.to_be(),
    };
    unsafe {
        core::ptr::write_unaligned(
            block_data.as_mut_ptr().add(tail_offset) as *mut jbd_revoke_tail,
            tail,
        );
    }

    // 验证校验和
    assert!(
        checksum::verify_revoke_block(&uuid, &block_data),
        "Revoke block checksum should verify correctly"
    );
}

#[test]
fn test_known_features() {
    // 验证已知特性标志
    assert_ne!(JBD_KNOWN_INCOMPAT_FEATURES, 0);

    let expected = JBD_FEATURE_INCOMPAT_REVOKE
        | JBD_FEATURE_INCOMPAT_ASYNC_COMMIT
        | JBD_FEATURE_INCOMPAT_64BIT
        | JBD_FEATURE_INCOMPAT_CSUM_V2
        | JBD_FEATURE_INCOMPAT_CSUM_V3;

    assert_eq!(
        JBD_KNOWN_INCOMPAT_FEATURES, expected,
        "Known incompat features should match expected value"
    );
}

#[test]
fn test_magic_number() {
    // 验证 JBD2 magic number
    assert_eq!(JBD_MAGIC_NUMBER, 0xC03B3998);
}

#[test]
fn test_block_types() {
    // 验证块类型常量
    assert_eq!(JBD_DESCRIPTOR_BLOCK, 1);
    assert_eq!(JBD_COMMIT_BLOCK, 2);
    assert_eq!(JBD_SUPERBLOCK_V1, 3);
    assert_eq!(JBD_SUPERBLOCK_V2, 4);
    assert_eq!(JBD_REVOKE_BLOCK, 5);
}

#[test]
fn test_checksum_disabled() {
    // 测试未启用校验和时的行为
    let mut sb = jbd_sb::default();
    sb.header.magic = JBD_MAGIC_NUMBER.to_be();
    sb.header.blocktype = JBD_SUPERBLOCK_V2.to_be();
    sb.feature_incompat = 0; // 未启用任何特性

    // 未启用校验和时，验证应该总是成功
    assert!(
        checksum::verify_superblock_csum(&sb),
        "Verification should succeed when checksum is disabled"
    );
}
