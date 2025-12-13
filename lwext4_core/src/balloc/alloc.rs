//! 块分配功能
//!
//! 对应 lwext4 的 `ext4_balloc_alloc_block()` 和 `ext4_balloc_try_alloc_block()`

use crate::{
    bitmap::{self, *},
    block::{Block, BlockDev, BlockDevice},
    block_group::BlockGroup,
    error::{Error, ErrorKind, Result},
    fs::BlockGroupRef,
    superblock::Superblock,
};

use super::{checksum::*, helpers::*};

/// 块分配器状态
///
/// 用于跟踪上次分配的块组，优化分配性能
pub struct BlockAllocator {
    last_block_bg_id: u32,
}

impl BlockAllocator {
    /// 创建新的块分配器
    pub fn new() -> Self {
        Self {
            last_block_bg_id: 0,
        }
    }

    /// 分配一个块（带目标块提示）
    ///
    /// 对应 lwext4 的 `ext4_balloc_alloc_block()`
    ///
    /// # 参数
    ///
    /// * `bdev` - 块设备引用
    /// * `sb` - superblock 可变引用
    /// * `goal` - 目标块地址（提示）
    ///
    /// # 返回
    ///
    /// 成功返回分配的块地址
    ///
    /// # 注意
    ///
    /// 此版本不更新 inode 的 blocks 计数，调用者需要自己处理
    pub fn alloc_block<D: BlockDevice>(
        &mut self,
        bdev: &mut BlockDev<D>,
        sb: &mut Superblock,
        goal: u64,
    ) -> Result<u64> {
        // 计算目标块组
        let bg_id = get_bgid_of_block(sb, goal);
        let idx_in_bg = addr_to_idx_bg(sb, goal);

        // 检查目标块组是否有空闲块
        let free_blocks = {
            let mut bg_ref = BlockGroupRef::get(bdev, sb, bg_id)?;
            bg_ref.free_blocks_count()?
        };

        // 尝试在目标块组中分配
        if free_blocks > 0 {
            if let Some(alloc) = self.try_alloc_in_group(bdev, sb, bg_id, idx_in_bg)? {
                self.last_block_bg_id = bg_id;
                return Ok(alloc);
            }
        }

        // 目标块组失败，尝试其他块组
        let block_group_count = sb.block_group_count();
        let mut bgid = (bg_id + 1) % block_group_count;
        let mut count = block_group_count - 1; // 已经尝试过一个了

        while count > 0 {
            // 检查此块组是否有空闲块
            let free_blocks = {
                let mut bg_ref = BlockGroupRef::get(bdev, sb, bgid)?;
                bg_ref.free_blocks_count()?
            };

            if free_blocks > 0 {
                // 计算此块组的起始索引
                let first_in_bg = get_block_of_bgid(sb, bgid);
                let idx_in_bg = addr_to_idx_bg(sb, first_in_bg);

                if let Some(alloc) = self.try_alloc_in_group(bdev, sb, bgid, idx_in_bg)? {
                    self.last_block_bg_id = bgid;
                    return Ok(alloc);
                }
            }

            bgid = (bgid + 1) % block_group_count;
            count -= 1;
        }

        Err(Error::new(ErrorKind::NoSpace, "No free blocks available"))
    }

    /// 在指定块组中尝试分配块
    fn try_alloc_in_group<D: BlockDevice>(
        &self,
        bdev: &mut BlockDev<D>,
        sb: &mut Superblock,
        bgid: u32,
        mut idx_in_bg: u32,
    ) -> Result<Option<u64>> {
        // 获取此块组的块数
        let blk_in_bg = sb.blocks_in_group_cnt(bgid);

        // 计算此块组的第一个有效索引
        let first_in_bg = get_block_of_bgid(sb, bgid);
        let first_in_bg_index = addr_to_idx_bg(sb, first_in_bg);

        if idx_in_bg < first_in_bg_index {
            idx_in_bg = first_in_bg_index;
        }

        // 第一步：获取位图地址和块组描述符副本
        let (bmp_blk_addr, bg_copy) = {
            let mut bg_ref = BlockGroupRef::get(bdev, sb, bgid)?;
            let bitmap_addr = bg_ref.block_bitmap()?;
            let bg_data = bg_ref.get_block_group_copy()?;
            (bitmap_addr, bg_data)
        };

        // 第二步：操作位图
        let alloc_opt = {
            let mut bitmap_block = Block::get(bdev, bmp_blk_addr)?;

            bitmap_block.with_data_mut(|bitmap_data| {
                // 验证位图校验和
                if !verify_bitmap_csum(sb, &bg_copy, bitmap_data) {
                    // 记录警告但继续
                }

                // 1. 检查目标位置是否空闲
                if !bitmap::test_bit(bitmap_data, idx_in_bg) {
                    set_bit(bitmap_data, idx_in_bg)?;
                    let mut bg_for_csum = bg_copy;
                    set_bitmap_csum(sb, &mut bg_for_csum, bitmap_data);
                    return Ok(Some(idx_in_bg));
                }

                // 2. 在目标附近查找（+63 范围内）
                let mut end_idx = (idx_in_bg + 63) & !63;
                if end_idx > blk_in_bg {
                    end_idx = blk_in_bg;
                }

                for tmp_idx in (idx_in_bg + 1)..end_idx {
                    if !bitmap::test_bit(bitmap_data, tmp_idx) {
                        set_bit(bitmap_data, tmp_idx)?;
                        let mut bg_for_csum = bg_copy;
                        set_bitmap_csum(sb, &mut bg_for_csum, bitmap_data);
                        return Ok(Some(tmp_idx));
                    }
                }

                // 3. 在整个块组中查找
                if let Some(rel_blk_idx) = find_first_zero(bitmap_data, idx_in_bg, blk_in_bg) {
                    set_bit(bitmap_data, rel_blk_idx)?;
                    let mut bg_for_csum = bg_copy;
                    set_bitmap_csum(sb, &mut bg_for_csum, bitmap_data);
                    return Ok(Some(rel_blk_idx));
                }

                Ok(None)
            })??
        };

        if let Some(idx) = alloc_opt {
            // 计算绝对地址
            let alloc = bg_idx_to_addr(sb, idx, bgid);

            // 第三步：更新块组描述符
            {
                let mut bg_ref = BlockGroupRef::get(bdev, sb, bgid)?;
                bg_ref.dec_free_blocks(1)?;
                // bg_ref 在此处自动释放并写回
            }

            // 更新 superblock 空闲块计数
            let mut sb_free_blocks = sb.free_blocks_count();
            if sb_free_blocks > 0 {
                sb_free_blocks -= 1;
            }
            sb.set_free_blocks_count(sb_free_blocks);
            sb.write(bdev)?;

            return Ok(Some(alloc));
        }

        Ok(None)
    }

    /// 获取上次分配的块组 ID
    pub fn last_bg_id(&self) -> u32 {
        self.last_block_bg_id
    }

    /// 设置上次分配的块组 ID
    pub fn set_last_bg_id(&mut self, bgid: u32) {
        self.last_block_bg_id = bgid;
    }
}

impl Default for BlockAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// 尝试分配特定的块地址
///
/// 对应 lwext4 的 `ext4_balloc_try_alloc_block()`
///
/// # 参数
///
/// * `bdev` - 块设备引用
/// * `sb` - superblock 可变引用
/// * `baddr` - 要尝试分配的块地址
///
/// # 返回
///
/// 成功返回 true（块已分配），false（块已被占用）
///
/// # 注意
///
/// 此版本不更新 inode 的 blocks 计数，调用者需要自己处理
pub fn try_alloc_block<D: BlockDevice>(
    bdev: &mut BlockDev<D>,
    sb: &mut Superblock,
    baddr: u64,
) -> Result<bool> {
    // 计算块组和索引
    let block_group = get_bgid_of_block(sb, baddr);
    let index_in_group = addr_to_idx_bg(sb, baddr);

    // 第一步：获取位图地址和块组描述符副本
    let (bmp_blk_addr, bg_copy) = {
        let mut bg_ref = BlockGroupRef::get(bdev, sb, block_group)?;
        let bitmap_addr = bg_ref.block_bitmap()?;
        let bg_data = bg_ref.get_block_group_copy()?;
        (bitmap_addr, bg_data)
    };

    // 第二步：操作位图
    let is_free = {
        let mut bitmap_block = Block::get(bdev, bmp_blk_addr)?;

        bitmap_block.with_data_mut(|bitmap_data| {
            // 验证位图校验和
            if !verify_bitmap_csum(sb, &bg_copy, bitmap_data) {
                // 记录警告但继续
            }

            // 检查块是否空闲
            let free = !bitmap::test_bit(bitmap_data, index_in_group);

            // 如果空闲，分配它
            if free {
                set_bit(bitmap_data, index_in_group)?;
                let mut bg_for_csum = bg_copy;
                set_bitmap_csum(sb, &mut bg_for_csum, bitmap_data);
            }

            Ok(free)
        })??
    };

    // 如果块不空闲，直接返回
    if !is_free {
        return Ok(false);
    }

    // 第三步：更新块组描述符
    {
        let mut bg_ref = BlockGroupRef::get(bdev, sb, block_group)?;
        bg_ref.dec_free_blocks(1)?;
        // bg_ref 在此处自动释放并写回
    }

    // 更新 superblock 空闲块计数
    let mut sb_free_blocks = sb.free_blocks_count();
    if sb_free_blocks > 0 {
        sb_free_blocks -= 1;
    }
    sb.set_free_blocks_count(sb_free_blocks);
    sb.write(bdev)?;

    Ok(true)
}

/// 分配一个块（无状态版本）
///
/// 这是一个便捷函数，从块 0 开始作为目标
///
/// # 参数
///
/// * `bdev` - 块设备引用
/// * `sb` - superblock 可变引用
///
/// # 返回
///
/// 成功返回分配的块地址
pub fn alloc_block<D: BlockDevice>(
    bdev: &mut BlockDev<D>,
    sb: &mut Superblock,
) -> Result<u64> {
    let mut allocator = BlockAllocator::new();
    let goal = sb.first_data_block() as u64;
    allocator.alloc_block(bdev, sb, goal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_allocator_creation() {
        let allocator = BlockAllocator::new();
        assert_eq!(allocator.last_bg_id(), 0);
    }

    #[test]
    fn test_block_allocator_set_last_bg() {
        let mut allocator = BlockAllocator::new();
        allocator.set_last_bg_id(5);
        assert_eq!(allocator.last_bg_id(), 5);
    }
}
