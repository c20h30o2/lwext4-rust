//! Superblock 操作模块

use crate::{Ext4Result, Ext4Error, Ext4Superblock, BlockDevice};
use crate::consts::*;

/// 读取并解析 superblock
pub fn read_superblock<D: BlockDevice>(dev: &mut D) -> Ext4Result<Ext4Superblock> {
    let mut sb_buf = [0u8; EXT4_SUPERBLOCK_SIZE];

    // 读取 superblock（从偏移 1024 开始）
    // 计算需要读取的块数
    let ph_bsize = dev.physical_block_size() as u64;
    let start_block = EXT4_SUPERBLOCK_OFFSET / ph_bsize;
    let block_count = ((EXT4_SUPERBLOCK_SIZE as u64 + ph_bsize - 1) / ph_bsize) as u32;

    dev.read_blocks(start_block, block_count, &mut sb_buf)?;

    // 解析 superblock（暂时简化，直接转换）
    let sb = unsafe {
        core::ptr::read_unaligned(sb_buf.as_ptr() as *const Ext4Superblock)
    };

    // 验证魔数
    if u16::from_le(sb.magic) != EXT4_SUPERBLOCK_MAGIC {
        return Err(Ext4Error::new(EINVAL, "Invalid ext4 magic number"));
    }

    Ok(sb)
}

/// 获取块大小
pub fn get_block_size(sb: &Ext4Superblock) -> u32 {
    1024 << u32::from_le(sb.log_block_size)
}

/// 获取 inode 大小
pub fn get_inode_size(sb: &Ext4Superblock) -> u16 {
    let size = u16::from_le(sb.inode_size);
    if size == 0 {
        128  // 默认值
    } else {
        size
    }
}

/// 计算块组数量
pub fn get_block_group_count(sb: &Ext4Superblock) -> u32 {
    let blocks_count = u32::from_le(sb.blocks_count_lo);
    let blocks_per_group = u32::from_le(sb.blocks_per_group);

    (blocks_count + blocks_per_group - 1) / blocks_per_group
}
