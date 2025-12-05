//! 工具函数模块，提供超级块相关的辅助计算。

use crate::ffi::ext4_sblock;

/// 计算文件系统的块大小
/// 块大小 = 1024 << log_block_size（超级块中存储的是对数形式）
pub fn get_block_size(sb: &ext4_sblock) -> u32 {
    1024u32 << u32::from_le(sb.log_block_size)
}

/// 获取文件系统的版本号（主版本 + 次版本）
pub fn revision_tuple(sb: &ext4_sblock) -> (u32, u16) {
    (u32::from_le(sb.rev_level), u16::from_le(sb.minor_rev_level))
}