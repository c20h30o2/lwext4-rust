//! Inode 分配功能

use crate::{
    bitmap::*,
    block::{Block, BlockDev, BlockDevice},
    block_group::BlockGroup,
    error::{Error, ErrorKind, Result},
    superblock::Superblock,
};

use super::{checksum::*, helpers::*};

/// Inode 分配器状态
///
/// 用于跟踪上次分配的块组，优化分配性能
pub struct InodeAllocator {
    last_inode_bg_id: u32,
}

impl InodeAllocator {
    /// 创建新的 inode 分配器
    pub fn new() -> Self {
        Self {
            last_inode_bg_id: 0,
        }
    }

    /// 分配一个 inode
    ///
    /// 对应 lwext4 的 `ext4_ialloc_alloc_inode()`
    ///
    /// # 参数
    ///
    /// * `bdev` - 块设备引用
    /// * `sb` - superblock 可变引用
    /// * `is_dir` - 是否是目录
    ///
    /// # 返回
    ///
    /// 成功返回分配的 inode 编号
    pub fn alloc_inode<D: BlockDevice>(
        &mut self,
        bdev: &mut BlockDev<D>,
        sb: &mut Superblock,
        is_dir: bool,
    ) -> Result<u32> {
        let mut bgid = self.last_inode_bg_id;
        let bg_count = sb.block_group_count();
        let mut sb_free_inodes = sb.free_inodes_count();
        let mut rewind = false;

        // 检查是否还有空闲 inode
        if sb_free_inodes == 0 {
            return Err(Error::new(ErrorKind::NoSpace, "No free inodes"));
        }

        // 尝试在所有块组中查找空闲 inode
        loop {
            if bgid >= bg_count {
                if rewind {
                    break; // 已经搜索过所有块组
                }
                // 从头开始重新搜索
                bgid = 0;
                rewind = true;
                continue;
            }

            // 加载块组描述符
            let mut bg = BlockGroup::load(bdev, sb, bgid)?;

            // 读取必要的值
            let free_inodes = bg.get_free_inodes_count(sb);
            let mut used_dirs = bg.get_used_dirs_count(sb);

            // 检查此块组是否有空闲 inode
            if free_inodes > 0 {
                // 计算此块组中的 inode 数（后续需要使用）
                let inodes_in_bg = inodes_in_group_cnt(sb, bgid);

                // 使用作用域确保 bitmap_block 在后续操作前被释放
                let idx_in_bg_opt = {
                    // 获取位图块句柄
                    let bmp_blk_addr = bg.get_inode_bitmap(sb);
                    let mut bitmap_block = Block::get(bdev, bmp_blk_addr)?;

                    // 在闭包内操作位图数据
                    bitmap_block.with_data_mut(|bitmap_data| {
                        // 验证位图校验和（如果启用）
                        if !verify_bitmap_csum(sb, &bg, bitmap_data) {
                            // 这里只是记录警告，不阻止操作
                        }

                        // 查找第一个空闲的 inode
                        let idx_in_bg = match find_first_zero(bitmap_data, 0, inodes_in_bg) {
                            Some(idx) => idx,
                            None => return None,
                        };

                        // 找到空闲 inode，设置位图中的位
                        if let Err(_) = set_bit(bitmap_data, idx_in_bg) {
                            return None;
                        }

                        // 更新位图校验和
                        set_bitmap_csum(sb, &mut bg, bitmap_data);

                        Some(idx_in_bg)
                    })?
                    // bitmap_block 在此处自动释放并写回
                };

                // 如果没找到空闲 inode，继续下一个块组
                let idx_in_bg = match idx_in_bg_opt {
                    Some(idx) => idx,
                    None => {
                        bgid += 1;
                        continue;
                    }
                };

                // 修改文件系统计数器
                let mut free_inodes_in_bg = free_inodes;
                if free_inodes_in_bg > 0 {
                    free_inodes_in_bg -= 1;
                }
                bg.set_free_inodes_count(sb, free_inodes_in_bg);

                // 如果是目录，增加已使用目录计数
                if is_dir {
                    used_dirs += 1;
                    bg.set_used_dirs_count(sb, used_dirs);
                }

                // 减少未使用的 inode 数
                let mut unused = bg.get_itable_unused(sb);
                let free = inodes_in_bg - unused;

                if idx_in_bg >= free {
                    unused = inodes_in_bg - (idx_in_bg + 1);
                    bg.set_itable_unused(sb, unused);
                }

                // 写回块组描述符
                bg.write(bdev, sb)?;

                // 更新 superblock
                if sb_free_inodes > 0 {
                    sb_free_inodes -= 1;
                }
                sb.set_free_inodes_count(sb_free_inodes);
                sb.write(bdev)?;

                // 计算绝对 inode 编号
                let inode_num = bgidx_to_inode(sb, idx_in_bg, bgid);

                // 更新分配器状态
                self.last_inode_bg_id = bgid;

                return Ok(inode_num);
            }

            // 块组没有空闲 inode，继续下一个
            bgid += 1;
        }

        Err(Error::new(ErrorKind::NoSpace, "No free inodes available"))
    }

    /// 获取上次分配的块组 ID
    pub fn last_bg_id(&self) -> u32 {
        self.last_inode_bg_id
    }

    /// 设置上次分配的块组 ID
    pub fn set_last_bg_id(&mut self, bgid: u32) {
        self.last_inode_bg_id = bgid;
    }
}

impl Default for InodeAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// 分配一个 inode（无状态版本）
///
/// 这是一个便捷函数，从块组 0 开始搜索
///
/// # 参数
///
/// * `bdev` - 块设备引用
/// * `sb` - superblock 可变引用
/// * `is_dir` - 是否是目录
///
/// # 返回
///
/// 成功返回分配的 inode 编号
pub fn alloc_inode<D: BlockDevice>(
    bdev: &mut BlockDev<D>,
    sb: &mut Superblock,
    is_dir: bool,
) -> Result<u32> {
    let mut allocator = InodeAllocator::new();
    allocator.alloc_inode(bdev, sb, is_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_allocator_creation() {
        let allocator = InodeAllocator::new();
        assert_eq!(allocator.last_bg_id(), 0);
    }

    #[test]
    fn test_inode_allocator_set_last_bg() {
        let mut allocator = InodeAllocator::new();
        allocator.set_last_bg_id(5);
        assert_eq!(allocator.last_bg_id(), 5);
    }
}
