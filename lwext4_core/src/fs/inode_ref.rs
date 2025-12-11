//! Inode 引用结构
//!
//! 对应 lwext4 的 `ext4_inode_ref`，提供 RAII 风格的 inode 操作

use crate::{
    block::{BlockDev, BlockDevice},
    error::Result,
    extent::ExtentTree,
    inode::Inode,
    superblock::Superblock,
};

/// Inode 引用
///
/// 类似 lwext4 的 `ext4_inode_ref`，自动管理 inode 的加载和写回
///
/// # 生命周期
///
/// - 创建时从磁盘加载 inode
/// - 修改时标记为 dirty
/// - Drop 时自动写回（如果 dirty）
///
/// # 示例
///
/// ```rust,ignore
/// let mut inode_ref = InodeRef::get(&mut bdev, &sb, inode_num)?;
/// inode_ref.set_size(1024);
/// inode_ref.mark_dirty();
/// // Drop 时自动写回
/// ```
pub struct InodeRef<'a, D: BlockDevice> {
    pub(crate) bdev: &'a mut BlockDev<D>,
    pub(crate) sb: &'a mut Superblock,
    inode_num: u32,
    inode: Inode,
    dirty: bool,
}

impl<'a, D: BlockDevice> InodeRef<'a, D> {
    /// 获取 inode 引用（自动加载）
    ///
    /// # 参数
    ///
    /// * `bdev` - 块设备引用
    /// * `sb` - superblock 引用
    /// * `inode_num` - inode 编号
    ///
    /// # 返回
    ///
    /// 成功返回 InodeRef
    pub fn get(
        bdev: &'a mut BlockDev<D>,
        sb: &'a mut Superblock,
        inode_num: u32,
    ) -> Result<Self> {
        let inode = Inode::load(bdev, sb, inode_num)?;

        Ok(Self {
            bdev,
            sb,
            inode_num,
            inode,
            dirty: false,
        })
    }

    /// 获取 inode 编号
    pub fn inode_num(&self) -> u32 {
        self.inode_num
    }

    /// 获取 inode 的不可变引用
    pub fn inode(&self) -> &Inode {
        &self.inode
    }

    /// 获取 inode 的可变引用
    ///
    /// 注意：获取可变引用不会自动标记为 dirty，需要手动调用 `mark_dirty()`
    pub fn inode_mut(&mut self) -> &mut Inode {
        &mut self.inode
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
            self.inode.write(self.bdev, self.sb)?;
            self.dirty = false;
        }
        Ok(())
    }

    // ===== 便捷方法 =====

    /// 获取文件大小
    pub fn size(&self) -> u64 {
        self.inode.file_size()
    }

    /// 设置文件大小
    pub fn set_size(&mut self, size: u64) {
        self.inode.set_size(size);
        self.dirty = true;
    }

    /// 获取 blocks 计数（512 字节单位）
    pub fn blocks_count(&self) -> u64 {
        self.inode.blocks_count_with_sb(self.sb)
    }

    /// 设置 blocks 计数（512 字节单位）
    pub fn set_blocks_count(&mut self, count: u64) {
        let _ = self.inode.set_blocks_count(self.sb, count);
        self.dirty = true;
    }

    /// 增加 blocks 计数
    ///
    /// # 参数
    ///
    /// * `blocks` - 要增加的块数（文件系统块大小）
    pub fn add_blocks(&mut self, blocks: u32) {
        let block_size = self.sb.block_size();
        let blocks_512 = blocks as u64 * (block_size as u64 / 512);
        let current = self.blocks_count();
        self.set_blocks_count(current + blocks_512);
    }

    /// 减少 blocks 计数
    ///
    /// # 参数
    ///
    /// * `blocks` - 要减少的块数（文件系统块大小）
    pub fn sub_blocks(&mut self, blocks: u32) {
        let block_size = self.sb.block_size();
        let blocks_512 = blocks as u64 * (block_size as u64 / 512);
        let current = self.blocks_count();
        if current >= blocks_512 {
            self.set_blocks_count(current - blocks_512);
        } else {
            self.set_blocks_count(0);
        }
    }

    /// 检查是否是目录
    pub fn is_dir(&self) -> bool {
        self.inode.is_dir()
    }

    /// 检查是否是普通文件
    pub fn is_file(&self) -> bool {
        self.inode.is_file()
    }

    /// 检查是否使用 extents
    pub fn has_extents(&self) -> bool {
        self.inode.has_extents()
    }

    /// 读取文件内容
    ///
    /// # 参数
    ///
    /// * `offset` - 文件内偏移（字节）
    /// * `buf` - 输出缓冲区
    ///
    /// # 返回
    ///
    /// 实际读取的字节数
    pub fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let mut extent_tree = ExtentTree::new(self.bdev, self.sb.block_size());
        extent_tree.read_file(&self.inode, offset, buf)
    }

    // 注意：write 方法需要更复杂的实现，涉及块分配等，暂时不实现
}

impl<'a, D: BlockDevice> Drop for InodeRef<'a, D> {
    fn drop(&mut self) {
        if self.dirty {
            // 忽略错误，因为 Drop 不能返回 Result
            let _ = self.inode.write(self.bdev, self.sb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_ref_api() {
        // 这些测试需要实际的块设备和 ext4 文件系统
        // 主要是验证 API 的设计和编译
    }
}
