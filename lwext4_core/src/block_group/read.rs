//! 块组描述符读取和查询操作

use crate::{
    block::{BlockDev, BlockDevice},
    consts::*,
    error::{Error, ErrorKind, Result},
    superblock::Superblock,
    types::ext4_group_desc,
};
use alloc::vec;

/// 读取块组描述符
///
/// # 参数
///
/// * `bdev` - 块设备引用
/// * `sb` - superblock 引用
/// * `group_num` - 块组编号
///
/// # 返回
///
/// 成功返回块组描述符
pub fn read_block_group_desc<D: BlockDevice>(
    bdev: &mut BlockDev<D>,
    sb: &Superblock,
    group_num: u32,
) -> Result<ext4_group_desc> {
    let block_size = sb.block_size() as u64;
    let desc_size = sb.group_desc_size() as u64;

    // 块组描述符表在第一个数据块之后
    let first_data_block = sb.first_data_block() as u64;
    let gdt_block = first_data_block + 1;

    // 计算描述符的偏移
    let desc_offset = gdt_block * block_size + (group_num as u64) * desc_size;

    // 读取块组描述符
    let mut desc_buf = vec![0u8; core::mem::size_of::<ext4_group_desc>()];
    bdev.read_bytes(desc_offset, &mut desc_buf)?;

    let desc = unsafe {
        core::ptr::read_unaligned(desc_buf.as_ptr() as *const ext4_group_desc)
    };

    Ok(desc)
}

/// BlockGroup 包装器，提供高级操作
pub struct BlockGroup {
    pub(super) inner: ext4_group_desc,
    pub(super) group_num: u32,
}

impl BlockGroup {
    /// 从块设备加载块组描述符
    ///
    /// # 参数
    ///
    /// * `bdev` - 块设备引用
    /// * `sb` - superblock 引用
    /// * `group_num` - 块组编号
    pub fn load<D: BlockDevice>(
        bdev: &mut BlockDev<D>,
        sb: &Superblock,
        group_num: u32,
    ) -> Result<Self> {
        let inner = read_block_group_desc(bdev, sb, group_num)?;
        Ok(Self { inner, group_num })
    }

    /// 获取块组编号
    pub fn group_num(&self) -> u32 {
        self.group_num
    }

    /// 获取内部块组描述符结构的引用
    pub fn inner(&self) -> &ext4_group_desc {
        &self.inner
    }

    /// 获取块位图块号
    ///
    /// 对应 lwext4 的 `ext4_bg_get_block_bitmap()`
    ///
    /// # 参数
    ///
    /// * `sb` - superblock 引用
    pub fn get_block_bitmap(&self, sb: &Superblock) -> u64 {
        let mut v = u32::from_le(self.inner.block_bitmap_lo) as u64;

        if sb.group_desc_size() > EXT4_MIN_BLOCK_GROUP_DESCRIPTOR_SIZE as usize {
            v |= (u32::from_le(self.inner.block_bitmap_hi) as u64) << 32;
        }

        v
    }

    /// 获取 inode 位图块号
    ///
    /// 对应 lwext4 的 `ext4_bg_get_inode_bitmap()`
    ///
    /// # 参数
    ///
    /// * `sb` - superblock 引用
    pub fn get_inode_bitmap(&self, sb: &Superblock) -> u64 {
        let mut v = u32::from_le(self.inner.inode_bitmap_lo) as u64;

        if sb.group_desc_size() > EXT4_MIN_BLOCK_GROUP_DESCRIPTOR_SIZE as usize {
            v |= (u32::from_le(self.inner.inode_bitmap_hi) as u64) << 32;
        }

        v
    }

    /// 获取 inode 表起始块号
    ///
    /// 对应 lwext4 的 `ext4_bg_get_inode_table_first_block()`
    ///
    /// # 参数
    ///
    /// * `sb` - superblock 引用
    pub fn get_inode_table_first_block(&self, sb: &Superblock) -> u64 {
        let mut v = u32::from_le(self.inner.inode_table_lo) as u64;

        if sb.group_desc_size() > EXT4_MIN_BLOCK_GROUP_DESCRIPTOR_SIZE as usize {
            v |= (u32::from_le(self.inner.inode_table_hi) as u64) << 32;
        }

        v
    }

    /// 获取空闲块数
    ///
    /// 对应 lwext4 的 `ext4_bg_get_free_blocks_count()`
    ///
    /// # 参数
    ///
    /// * `sb` - superblock 引用
    pub fn get_free_blocks_count(&self, sb: &Superblock) -> u32 {
        let mut v = u16::from_le(self.inner.free_blocks_count_lo) as u32;

        if sb.group_desc_size() > EXT4_MIN_BLOCK_GROUP_DESCRIPTOR_SIZE as usize {
            v |= (u16::from_le(self.inner.free_blocks_count_hi) as u32) << 16;
        }

        v
    }

    /// 获取空闲 inode 数
    ///
    /// 对应 lwext4 的 `ext4_bg_get_free_inodes_count()`
    ///
    /// # 参数
    ///
    /// * `sb` - superblock 引用
    pub fn get_free_inodes_count(&self, sb: &Superblock) -> u32 {
        let mut v = u16::from_le(self.inner.free_inodes_count_lo) as u32;

        if sb.group_desc_size() > EXT4_MIN_BLOCK_GROUP_DESCRIPTOR_SIZE as usize {
            v |= (u16::from_le(self.inner.free_inodes_count_hi) as u32) << 16;
        }

        v
    }

    /// 获取已使用的目录数
    ///
    /// 对应 lwext4 的 `ext4_bg_get_used_dirs_count()`
    ///
    /// # 参数
    ///
    /// * `sb` - superblock 引用
    pub fn get_used_dirs_count(&self, sb: &Superblock) -> u32 {
        let mut v = u16::from_le(self.inner.used_dirs_count_lo) as u32;

        if sb.group_desc_size() > EXT4_MIN_BLOCK_GROUP_DESCRIPTOR_SIZE as usize {
            v |= (u16::from_le(self.inner.used_dirs_count_hi) as u32) << 16;
        }

        v
    }

    /// 获取未使用的 inode 数
    ///
    /// 对应 lwext4 的 `ext4_bg_get_itable_unused()`
    ///
    /// # 参数
    ///
    /// * `sb` - superblock 引用
    pub fn get_itable_unused(&self, sb: &Superblock) -> u32 {
        let mut v = u16::from_le(self.inner.itable_unused_lo) as u32;

        if sb.group_desc_size() > EXT4_MIN_BLOCK_GROUP_DESCRIPTOR_SIZE as usize {
            v |= (u16::from_le(self.inner.itable_unused_hi) as u32) << 16;
        }

        v
    }

    /// 检查块组是否有指定标志
    ///
    /// 对应 lwext4 的 `ext4_bg_has_flag()`
    ///
    /// # 参数
    ///
    /// * `flag` - 要检查的标志
    pub fn has_flag(&self, flag: u16) -> bool {
        (u16::from_le(self.inner.flags) & flag) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_group_getters() {
        let mut desc = ext4_group_desc::default();

        // 设置测试数据
        desc.block_bitmap_lo = 100u32.to_le();
        desc.inode_bitmap_lo = 200u32.to_le();
        desc.inode_table_lo = 300u32.to_le();
        desc.free_blocks_count_lo = 1000u16.to_le();
        desc.free_inodes_count_lo = 2000u16.to_le();
        desc.used_dirs_count_lo = 50u16.to_le();
        desc.itable_unused_lo = 500u16.to_le();

        let bg = BlockGroup {
            inner: desc,
            group_num: 0,
        };

        // 创建一个测试用的 superblock（使用最小描述符大小）
        let mut sb_inner = crate::types::ext4_sblock::default();
        sb_inner.desc_size = (EXT4_MIN_BLOCK_GROUP_DESCRIPTOR_SIZE as u16).to_le();
        let sb = Superblock::new(sb_inner);

        assert_eq!(bg.get_block_bitmap(&sb), 100);
        assert_eq!(bg.get_inode_bitmap(&sb), 200);
        assert_eq!(bg.get_inode_table_first_block(&sb), 300);
        assert_eq!(bg.get_free_blocks_count(&sb), 1000);
        assert_eq!(bg.get_free_inodes_count(&sb), 2000);
        assert_eq!(bg.get_used_dirs_count(&sb), 50);
        assert_eq!(bg.get_itable_unused(&sb), 500);
    }

    #[test]
    fn test_block_group_flags() {
        let mut desc = ext4_group_desc::default();
        desc.flags = 0x0005u16.to_le(); // 设置bit 0和bit 2

        let bg = BlockGroup {
            inner: desc,
            group_num: 0,
        };

        assert!(bg.has_flag(0x0001));
        assert!(!bg.has_flag(0x0002));
        assert!(bg.has_flag(0x0004));
        assert!(!bg.has_flag(0x0008));
    }
}
