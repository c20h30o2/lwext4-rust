//! 块组引用结构
//!
//! 对应 lwext4 的 `ext4_block_group_ref`，提供 RAII 风格的块组操作

use crate::{
    block::{BlockDev, BlockDevice},
    block_group::BlockGroup,
    error::Result,
    superblock::Superblock,
};

/// 块组引用
///
/// 类似 lwext4 的 `ext4_block_group_ref`，自动管理块组描述符的加载和写回
///
/// # 生命周期
///
/// - 创建时从磁盘加载块组描述符
/// - 修改时标记为 dirty
/// - Drop 时自动写回（如果 dirty）
///
/// # 示例
///
/// ```rust,ignore
/// let mut bg_ref = BlockGroupRef::get(&mut bdev, &sb, bgid)?;
/// bg_ref.set_free_blocks_count(100);
/// bg_ref.mark_dirty();
/// // Drop 时自动写回
/// ```
pub struct BlockGroupRef<'a, D: BlockDevice> {
    bdev: &'a mut BlockDev<D>,
    sb: &'a mut Superblock,
    bgid: u32,
    bg: BlockGroup,
    dirty: bool,
}

impl<'a, D: BlockDevice> BlockGroupRef<'a, D> {
    /// 获取块组引用（自动加载）
    ///
    /// # 参数
    ///
    /// * `bdev` - 块设备引用
    /// * `sb` - superblock 引用
    /// * `bgid` - 块组 ID
    ///
    /// # 返回
    ///
    /// 成功返回 BlockGroupRef
    pub fn get(
        bdev: &'a mut BlockDev<D>,
        sb: &'a mut Superblock,
        bgid: u32,
    ) -> Result<Self> {
        let bg = BlockGroup::load(bdev, sb, bgid)?;

        Ok(Self {
            bdev,
            sb,
            bgid,
            bg,
            dirty: false,
        })
    }

    /// 获取块组 ID
    pub fn bgid(&self) -> u32 {
        self.bgid
    }

    /// 获取块组的不可变引用
    pub fn block_group(&self) -> &BlockGroup {
        &self.bg
    }

    /// 获取块组的可变引用
    ///
    /// 注意：获取可变引用不会自动标记为 dirty，需要手动调用 `mark_dirty()`
    pub fn block_group_mut(&mut self) -> &mut BlockGroup {
        &mut self.bg
    }

    /// 标记为脏（需要写回）
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// 检查是否为脏
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 手动写回
    ///
    /// 通常不需要手动调用，Drop 时会自动写回
    pub fn flush(&mut self) -> Result<()> {
        if self.dirty {
            self.bg.write(self.bdev, self.sb)?;
            self.dirty = false;
        }
        Ok(())
    }

    // ===== 便捷方法 =====

    /// 获取块位图地址
    pub fn block_bitmap(&self) -> u64 {
        self.bg.get_block_bitmap(self.sb)
    }

    /// 获取 inode 位图地址
    pub fn inode_bitmap(&self) -> u64 {
        self.bg.get_inode_bitmap(self.sb)
    }

    /// 获取 inode 表地址
    pub fn inode_table(&self) -> u64 {
        self.bg.get_inode_table_first_block(self.sb)
    }

    /// 获取空闲块数
    pub fn free_blocks_count(&self) -> u32 {
        self.bg.get_free_blocks_count(self.sb)
    }

    /// 设置空闲块数
    pub fn set_free_blocks_count(&mut self, count: u32) {
        self.bg.set_free_blocks_count(self.sb, count);
        self.dirty = true;
    }

    /// 增加空闲块数
    pub fn inc_free_blocks(&mut self, delta: u32) {
        let current = self.free_blocks_count();
        self.set_free_blocks_count(current + delta);
    }

    /// 减少空闲块数
    pub fn dec_free_blocks(&mut self, delta: u32) {
        let current = self.free_blocks_count();
        if current >= delta {
            self.set_free_blocks_count(current - delta);
        } else {
            self.set_free_blocks_count(0);
        }
    }

    /// 获取空闲 inode 数
    pub fn free_inodes_count(&self) -> u32 {
        self.bg.get_free_inodes_count(self.sb)
    }

    /// 设置空闲 inode 数
    pub fn set_free_inodes_count(&mut self, count: u32) {
        self.bg.set_free_inodes_count(self.sb, count);
        self.dirty = true;
    }

    /// 增加空闲 inode 数
    pub fn inc_free_inodes(&mut self, delta: u32) {
        let current = self.free_inodes_count();
        self.set_free_inodes_count(current + delta);
    }

    /// 减少空闲 inode 数
    pub fn dec_free_inodes(&mut self, delta: u32) {
        let current = self.free_inodes_count();
        if current >= delta {
            self.set_free_inodes_count(current - delta);
        } else {
            self.set_free_inodes_count(0);
        }
    }

    /// 获取已使用目录数
    pub fn used_dirs_count(&self) -> u32 {
        self.bg.get_used_dirs_count(self.sb)
    }

    /// 设置已使用目录数
    pub fn set_used_dirs_count(&mut self, count: u32) {
        self.bg.set_used_dirs_count(self.sb, count);
        self.dirty = true;
    }

    /// 增加已使用目录数
    pub fn inc_used_dirs(&mut self) {
        let current = self.used_dirs_count();
        self.set_used_dirs_count(current + 1);
    }

    /// 减少已使用目录数
    pub fn dec_used_dirs(&mut self) {
        let current = self.used_dirs_count();
        if current > 0 {
            self.set_used_dirs_count(current - 1);
        }
    }

    /// 获取未使用 inode 表数
    pub fn itable_unused(&self) -> u32 {
        self.bg.get_itable_unused(self.sb)
    }

    /// 设置未使用 inode 表数
    pub fn set_itable_unused(&mut self, count: u32) {
        self.bg.set_itable_unused(self.sb, count);
        self.dirty = true;
    }
}

impl<'a, D: BlockDevice> Drop for BlockGroupRef<'a, D> {
    fn drop(&mut self) {
        if self.dirty {
            // 忽略错误，因为 Drop 不能返回 Result
            let _ = self.bg.write(self.bdev, self.sb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_group_ref_api() {
        // 这些测试需要实际的块设备和 ext4 文件系统
        // 主要是验证 API 的设计和编译
    }
}
