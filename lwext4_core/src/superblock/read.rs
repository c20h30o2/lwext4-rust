//! Superblock 读取和验证

use crate::{
    block::{BlockDev, BlockDevice},
    consts::*,
    error::{Error, ErrorKind, Result},
    types::ext4_sblock,
};
use alloc::vec;

/// 从块设备读取 superblock
///
/// # 参数
///
/// * `bdev` - 块设备引用
///
/// # 返回
///
/// 成功返回 superblock 结构
pub fn read_superblock<D: BlockDevice>(bdev: &mut BlockDev<D>) -> Result<ext4_sblock> {
    let mut sb_buf = vec![0u8; EXT4_SUPERBLOCK_SIZE];

    // 读取 superblock（从偏移 1024 开始）
    bdev.read_bytes(EXT4_SUPERBLOCK_OFFSET, &mut sb_buf)?;

    // 解析 superblock
    let sb = unsafe {
        core::ptr::read_unaligned(sb_buf.as_ptr() as *const ext4_sblock)
    };

    // 验证魔数
    if !sb.is_valid() {
        return Err(Error::new(
            ErrorKind::Corrupted,
            "Invalid ext4 superblock magic number",
        ));
    }

    Ok(sb)
}

/// Superblock 包装器，提供高级操作
pub struct Superblock {
    inner: ext4_sblock,
}

impl Superblock {
    /// 从块设备加载 superblock
    pub fn load<D: BlockDevice>(bdev: &mut BlockDev<D>) -> Result<Self> {
        let inner = read_superblock(bdev)?;
        Ok(Self { inner })
    }

    /// 获取内部 superblock 结构的引用
    pub fn inner(&self) -> &ext4_sblock {
        &self.inner
    }

    /// 获取块大小
    pub fn block_size(&self) -> u32 {
        self.inner.block_size()
    }

    /// 获取 inode 大小
    pub fn inode_size(&self) -> u16 {
        self.inner.inode_size()
    }

    /// 获取总块数
    pub fn blocks_count(&self) -> u64 {
        self.inner.blocks_count()
    }

    /// 获取空闲块数
    pub fn free_blocks_count(&self) -> u64 {
        self.inner.free_blocks_count()
    }

    /// 获取总 inode 数
    pub fn inodes_count(&self) -> u32 {
        u32::from_le(self.inner.inodes_count)
    }

    /// 获取空闲 inode 数
    pub fn free_inodes_count(&self) -> u32 {
        u32::from_le(self.inner.free_inodes_count)
    }

    /// 获取每组块数
    pub fn blocks_per_group(&self) -> u32 {
        u32::from_le(self.inner.blocks_per_group)
    }

    /// 获取每组 inode 数
    pub fn inodes_per_group(&self) -> u32 {
        u32::from_le(self.inner.inodes_per_group)
    }

    /// 获取块组数量
    pub fn block_group_count(&self) -> u32 {
        self.inner.block_group_count()
    }

    /// 获取第一个数据块
    pub fn first_data_block(&self) -> u32 {
        u32::from_le(self.inner.first_data_block)
    }

    /// 检查是否支持某个兼容特性
    pub fn has_compat_feature(&self, feature: u32) -> bool {
        (u32::from_le(self.inner.feature_compat) & feature) != 0
    }

    /// 检查是否支持某个不兼容特性
    pub fn has_incompat_feature(&self, feature: u32) -> bool {
        (u32::from_le(self.inner.feature_incompat) & feature) != 0
    }

    /// 检查是否支持某个只读兼容特性
    pub fn has_ro_compat_feature(&self, feature: u32) -> bool {
        (u32::from_le(self.inner.feature_ro_compat) & feature) != 0
    }

    /// 检查是否使用 extent
    pub fn has_extents(&self) -> bool {
        self.has_incompat_feature(EXT4_FEATURE_INCOMPAT_EXTENTS)
    }

    /// 检查是否是 64 位文件系统
    pub fn is_64bit(&self) -> bool {
        self.has_incompat_feature(EXT4_FEATURE_INCOMPAT_64BIT)
    }

    /// 获取块组描述符大小
    pub fn group_desc_size(&self) -> usize {
        if self.is_64bit() {
            let size = u16::from_le(self.inner.desc_size) as usize;
            if size > 0 {
                size
            } else {
                EXT4_GROUP_DESC_SIZE_64
            }
        } else {
            EXT4_GROUP_DESC_SIZE
        }
    }

    /// 获取卷名称（UTF-8 字符串）
    pub fn volume_name(&self) -> Option<&str> {
        // 找到第一个 null 字节
        let len = self.inner.volume_name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.inner.volume_name.len());

        core::str::from_utf8(&self.inner.volume_name[..len]).ok()
    }

    /// 验证文件系统状态
    pub fn is_clean(&self) -> bool {
        const EXT4_VALID_FS: u16 = 0x0001;
        (u16::from_le(self.inner.state) & EXT4_VALID_FS) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_superblock_validation() {
        let mut sb = ext4_sblock::default();

        // 应该无效（魔数为 0）
        assert!(!sb.is_valid());

        // 设置正确的魔数
        sb.magic = EXT4_SUPERBLOCK_MAGIC.to_le();
        assert!(sb.is_valid());
    }

    #[test]
    fn test_superblock_helpers() {
        let mut sb = ext4_sblock::default();
        sb.magic = EXT4_SUPERBLOCK_MAGIC.to_le();
        sb.log_block_size = 2u32.to_le(); // 4096 = 1024 << 2
        sb.blocks_count_lo = 1000u32.to_le();
        sb.blocks_per_group = 100u32.to_le();

        assert_eq!(sb.block_size(), 4096);
        assert_eq!(sb.blocks_count(), 1000);
        assert_eq!(sb.block_group_count(), 10);
    }
}
