//! Inode 引用结构
//!
//! 对应 lwext4 的 `ext4_inode_ref`，提供 RAII 风格的 inode 操作

use crate::{
    block::{Block, BlockDev, BlockDevice},
    consts::*,
    error::{Error, ErrorKind, Result},
    extent::ExtentTree,
    superblock::Superblock,
    types::ext4_inode,
};

/// Inode 引用
///
/// 类似 lwext4 的 `ext4_inode_ref`，自动管理 inode 的加载和写回
///
/// # 设计说明
///
/// 与 lwext4 C 版本一致，InodeRef 持有一个 Block 句柄，
/// 直接操作 cache 中的 inode 数据，而不是持有数据副本。
/// 这保证了：
/// 1. **一致性**: 所有对同一 inode 的访问都操作同一份 cache 数据
/// 2. **性能**: 避免不必要的数据复制
/// 3. **正确语义**: 修改直接作用于 cache，自动标记为脏
///
/// # 生命周期
///
/// - 创建时获取包含 inode 的 block 句柄
/// - 通过 block 句柄访问和修改 inode 数据
/// - Drop 时自动释放 block 句柄
///
/// # 示例
///
/// ```rust,ignore
/// let mut inode_ref = InodeRef::get(&mut bdev, &sb, inode_num)?;
/// inode_ref.set_size(1024)?;
/// inode_ref.mark_dirty()?;
/// // Drop 时自动写回 inode
/// ```
pub struct InodeRef<'a, D: BlockDevice> {
    /// 块设备引用
    bdev: &'a mut BlockDev<D>,
    /// Superblock 引用
    sb: &'a Superblock,
    /// Inode 编号
    inode_num: u32,
    /// Inode 所在的块地址
    inode_block_addr: u64,
    /// Inode 在块内的偏移（字节）
    offset_in_block: usize,
    /// 是否已标记为脏
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
    ///
    /// # 实现说明
    ///
    /// 对应 lwext4 的 `ext4_fs_get_inode_ref()`
    pub fn get(
        bdev: &'a mut BlockDev<D>,
        sb: &'a Superblock,
        inode_num: u32,
    ) -> Result<Self> {
        if inode_num == 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid inode number (0)",
            ));
        }

        // 计算 inode 所在的块组和索引
        let inodes_per_group = sb.inodes_per_group();
        let block_group = (inode_num - 1) / inodes_per_group;
        let index_in_group = (inode_num - 1) % inodes_per_group;

        // 读取块组描述符以获取 inode 表位置
        // 注意：这里我们需要临时读取块组描述符，不需要持有 BlockGroupRef
        let inode_table_block = {
            use crate::block_group::BlockGroup;
            let bg = BlockGroup::load(bdev, sb, block_group)?;
            bg.get_inode_table_first_block(sb)
        };

        // 计算 inode 在 inode 表中的位置
        let block_size = sb.block_size() as u64;
        let inode_size = sb.inode_size() as u64;
        let inodes_per_block = block_size / inode_size;

        // 计算 inode 所在的块号和块内偏移
        let block_index = index_in_group as u64 / inodes_per_block;
        let offset_in_block = ((index_in_group as u64 % inodes_per_block) * inode_size) as usize;
        let inode_block_addr = inode_table_block + block_index;

        Ok(Self {
            bdev,
            sb,
            inode_num,
            inode_block_addr,
            offset_in_block,
            dirty: false,
        })
    }

    /// 获取 inode 编号
    pub fn inode_num(&self) -> u32 {
        self.inode_num
    }

    /// 访问 inode 数据（只读）
    ///
    /// 通过闭包访问 inode 数据，避免生命周期问题
    pub fn with_inode<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce(&ext4_inode) -> R,
    {
        let mut block = Block::get(self.bdev, self.inode_block_addr)?;
        block.with_data(|data| {
            let inode = unsafe {
                &*(data.as_ptr().add(self.offset_in_block) as *const ext4_inode)
            };
            f(inode)
        })
    }

    /// 访问 inode 数据（可写）
    ///
    /// 通过闭包修改 inode 数据，自动标记 block 为脏
    pub fn with_inode_mut<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce(&mut ext4_inode) -> R,
    {
        let mut block = Block::get(self.bdev, self.inode_block_addr)?;
        let result = block.with_data_mut(|data| {
            let inode = unsafe {
                &mut *(data.as_mut_ptr().add(self.offset_in_block) as *mut ext4_inode)
            };
            f(inode)
        })?;
        self.dirty = true;
        Ok(result)
    }

    /// 标记为脏（需要写回）
    ///
    /// 注意：修改 inode 时会自动标记为脏，通常不需要手动调用
    pub fn mark_dirty(&mut self) -> Result<()> {
        if !self.dirty {
            // 标记 block 为脏 - 获取块并立即标记为脏
            let mut block = Block::get(self.bdev, self.inode_block_addr)?;
            block.with_data_mut(|_| {})?;
            self.dirty = true;
        }
        Ok(())
    }

    /// 检查是否为脏
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 手动写回
    ///
    /// 通常不需要手动调用，Drop 时 Block 会自动写回脏数据
    pub fn flush(&mut self) -> Result<()> {
        // Block 的 Drop 会自动处理写回
        // 这里只需要清除 dirty 标志
        if self.dirty {
            self.dirty = false;
        }
        Ok(())
    }

    // ===== 便捷方法 =====

    /// 获取文件大小
    pub fn size(&mut self) -> Result<u64> {
        self.with_inode(|inode| inode.file_size())
    }

    /// 设置文件大小
    pub fn set_size(&mut self, size: u64) -> Result<()> {
        self.with_inode_mut(|inode| {
            // 直接修改 inode 字段
            inode.size_lo = ((size << 32) >> 32).to_le() as u32;
            inode.size_hi = (size >> 32).to_le() as u32;
        })
    }

    /// 获取 blocks 计数（512 字节单位）
    pub fn blocks_count(&mut self) -> Result<u64> {
        let sb = self.sb;
        self.with_inode(|inode| {
            // 读取 32 位低位
            let mut cnt = u32::from_le(inode.blocks_count_lo) as u64;

            // 检查是否启用了 HUGE_FILE 特性
            if sb.has_ro_compat_feature(EXT4_FEATURE_RO_COMPAT_HUGE_FILE) {
                // 扩展到 48 位
                cnt |= (u16::from_le(inode.blocks_high) as u64) << 32;

                // 检查 inode 是否使用了 HUGE_FILE 标志
                let flags = u32::from_le(inode.flags);
                if flags & EXT4_INODE_FLAG_HUGE_FILE != 0 {
                    // 进行比例换算：从文件系统块单位转换为 512 字节单位
                    let block_size = sb.block_size();
                    let block_bits = inode_block_bits_count(block_size);
                    return cnt << (block_bits - 9);
                }
            }

            cnt
        })
    }

    /// 设置 blocks 计数（512 字节单位）
    pub fn set_blocks_count(&mut self, count: u64) -> Result<()> {
        let sb = self.sb;
        self.with_inode_mut(|inode| {
            // 32 位最大值
            let max_32bit: u64 = 0xFFFFFFFF;

            if count <= max_32bit {
                // 可以用 32 位表示
                inode.blocks_count_lo = (count as u32).to_le();
                inode.blocks_high = 0;
                let flags = u32::from_le(inode.flags);
                inode.flags = (flags & !EXT4_INODE_FLAG_HUGE_FILE).to_le();
                return;
            }

            // 48 位最大值
            let max_48bit: u64 = 0xFFFFFFFFFFFF;

            if count <= max_48bit {
                // 可以用 48 位表示（不需要比例换算）
                inode.blocks_count_lo = (count as u32).to_le();
                inode.blocks_high = ((count >> 32) as u16).to_le();
                let flags = u32::from_le(inode.flags);
                inode.flags = (flags & !EXT4_INODE_FLAG_HUGE_FILE).to_le();
            } else {
                // 需要使用 HUGE_FILE 标志和比例换算
                let block_size = sb.block_size();
                let block_bits = inode_block_bits_count(block_size);

                let flags = u32::from_le(inode.flags);
                inode.flags = (flags | EXT4_INODE_FLAG_HUGE_FILE).to_le();

                // 从 512 字节单位转换为文件系统块单位
                let scaled_count = count >> (block_bits - 9);
                inode.blocks_count_lo = (scaled_count as u32).to_le();
                inode.blocks_high = ((scaled_count >> 32) as u16).to_le();
            }
        })
    }

    /// 增加 blocks 计数
    ///
    /// # 参数
    ///
    /// * `blocks` - 要增加的块数（文件系统块大小）
    pub fn add_blocks(&mut self, blocks: u32) -> Result<()> {
        let block_size = self.sb.block_size();
        let blocks_512 = blocks as u64 * (block_size as u64 / 512);
        let current = self.blocks_count()?;
        self.set_blocks_count(current + blocks_512)
    }

    /// 减少 blocks 计数
    ///
    /// # 参数
    ///
    /// * `blocks` - 要减少的块数（文件系统块大小）
    pub fn sub_blocks(&mut self, blocks: u32) -> Result<()> {
        let block_size = self.sb.block_size();
        let blocks_512 = blocks as u64 * (block_size as u64 / 512);
        let current = self.blocks_count()?;
        if current >= blocks_512 {
            self.set_blocks_count(current - blocks_512)
        } else {
            self.set_blocks_count(0)
        }
    }

    /// 检查是否是目录
    pub fn is_dir(&mut self) -> Result<bool> {
        self.with_inode(|inode| inode.is_dir())
    }

    /// 检查是否是普通文件
    pub fn is_file(&mut self) -> Result<bool> {
        self.with_inode(|inode| inode.is_file())
    }

    /// 检查是否使用 extents
    pub fn has_extents(&mut self) -> Result<bool> {
        self.with_inode(|inode| {
            let flags = u32::from_le(inode.flags);
            (flags & EXT4_INODE_FLAG_EXTENTS) != 0
        })
    }

    /// 获取 inode 数据的拷贝（用于需要长期持有的场景）
    ///
    /// 注意：返回的是数据副本，修改不会反映到磁盘
    pub fn get_inode_copy(&mut self) -> Result<ext4_inode> {
        self.with_inode(|inode| *inode)
    }

    /// 获取 inode 的 generation（用于校验和等）
    pub fn generation(&mut self) -> Result<u32> {
        self.with_inode(|inode| u32::from_le(inode.generation))
    }

    /// 获取 inode 编号（便捷方法）
    pub fn index(&self) -> u32 {
        self.inode_num
    }

    /// 获取 superblock 引用
    pub fn sb(&self) -> &Superblock {
        self.sb
    }

    /// 获取 BlockDev 的可变引用
    ///
    /// 用于需要访问块设备的操作（如读取目录块）
    pub fn bdev(&mut self) -> &mut BlockDev<D> {
        self.bdev
    }

    /// 将逻辑块号映射到物理块号
    ///
    /// 对应 lwext4 的 `ext4_fs_get_inode_dblk_idx()`
    ///
    /// # 参数
    ///
    /// * `logical_block` - 逻辑块号（文件内的块索引）
    /// * `create` - 是否在不存在时创建（暂不支持）
    ///
    /// # 返回
    ///
    /// 物理块号
    pub fn get_inode_dblk_idx(
        &mut self,
        logical_block: u32,
        _create: bool,
    ) -> Result<u64> {
        // 检查是否使用 extents
        if !self.has_extents()? {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "Non-extent block mapping not yet supported",
            ));
        }

        // 获取 inode 数据副本（包含 extent 根节点）
        let inode_copy = self.get_inode_copy()?;

        // 创建临时的 Inode 封装（用于 ExtentTree）
        let temp_inode = crate::inode::Inode::from_raw(inode_copy, self.inode_num);

        // 使用 ExtentTree 进行映射
        let mut extent_tree = ExtentTree::new(self.bdev, self.sb.block_size());

        match extent_tree.map_block(&temp_inode, logical_block)? {
            Some(physical_block) => Ok(physical_block),
            None => Err(Error::new(
                ErrorKind::NotFound,
                "Logical block not found in extent tree",
            )),
        }
    }

    // ========================================================================
    // 块分配集成说明
    // ========================================================================
    //
    // InodeRef 的块分配功能通过 `balloc::fs_integration` 模块提供。
    //
    // 使用示例：
    // ```rust,ignore
    // use lwext4_core::balloc::fs_integration;
    //
    // // 分配块
    // let baddr = fs_integration::alloc_block_with_inode(
    //     &mut allocator, bdev, &mut sb, &mut inode_ref, goal
    // )?;
    //
    // // 释放块
    // fs_integration::free_block_with_inode(
    //     bdev, &mut sb, &mut inode_ref, baddr
    // )?;
    // ```
    //
    // 这些函数会自动更新 inode 的 blocks 计数和 superblock 的空闲块计数。

    // 注意：read/write 方法需要更复杂的实现，涉及 extent tree 等，暂时不实现
}

impl<'a, D: BlockDevice> Drop for InodeRef<'a, D> {
    fn drop(&mut self) {
        // Block 的 Drop 会自动处理写回
        // 这里不需要额外操作
    }
}

/// 计算块大小的位数
///
/// 对应 lwext4 的 `ext4_inode_block_bits_count()`
///
/// # 参数
///
/// * `block_size` - 块大小（字节）
///
/// # 返回
///
/// 块大小的位数（用于地址计算）
fn inode_block_bits_count(block_size: u32) -> u32 {
    let mut bits = 8;
    let mut size = block_size;

    while size > 256 {
        bits += 1;
        size >>= 1;
    }

    bits
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
