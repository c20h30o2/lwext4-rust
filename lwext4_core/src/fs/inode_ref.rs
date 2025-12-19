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
    /// Superblock 引用（可变，以支持块分配等写操作）
    sb: &'a mut Superblock,
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
        sb: &'a mut Superblock,
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

    /// 获取可变 Superblock 引用
    ///
    /// 注意：此方法仅供内部 API 使用，用于解决某些遗留 API 的借用冲突
    pub(crate) fn superblock_mut(&mut self) -> &mut Superblock {
        self.sb
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
        // 先提取需要的 superblock 信息
        let has_huge_file = self.sb.has_ro_compat_feature(EXT4_FEATURE_RO_COMPAT_HUGE_FILE);
        let block_size = self.sb.block_size();

        self.with_inode(|inode| {
            // 读取 32 位低位
            let mut cnt = u32::from_le(inode.blocks_count_lo) as u64;

            // 检查是否启用了 HUGE_FILE 特性
            if has_huge_file {
                // 扩展到 48 位
                cnt |= (u16::from_le(inode.blocks_high) as u64) << 32;

                // 检查 inode 是否使用了 HUGE_FILE 标志
                let flags = u32::from_le(inode.flags);
                if flags & EXT4_INODE_FLAG_HUGE_FILE != 0 {
                    // 进行比例换算：从文件系统块单位转换为 512 字节单位
                    let block_bits = inode_block_bits_count(block_size);
                    return cnt << (block_bits - 9);
                }
            }

            cnt
        })
    }

    /// 设置 blocks 计数（512 字节单位）
    pub fn set_blocks_count(&mut self, count: u64) -> Result<()> {
        // 先提取需要的 superblock 信息
        let block_size = self.sb.block_size();

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

    /// 设置文件权限（Unix 权限位）
    ///
    /// # 参数
    ///
    /// * `mode` - 权限位（0o000 - 0o7777）
    ///
    /// # 注意
    ///
    /// 只修改权限位（低 12 位），不修改文件类型位
    pub fn set_mode(&mut self, mode: u16) -> Result<()> {
        self.with_inode_mut(|inode| {
            let current_mode = u16::from_le(inode.mode);
            // 保留文件类型位（高 4 位），只修改权限位（低 12 位）
            let new_mode = (current_mode & 0xF000) | (mode & 0x0FFF);
            inode.mode = new_mode.to_le();
        })
    }

    /// 设置文件所有者
    ///
    /// # 参数
    ///
    /// * `uid` - 用户 ID
    /// * `gid` - 组 ID
    pub fn set_owner(&mut self, uid: u32, gid: u32) -> Result<()> {
        self.with_inode_mut(|inode| {
            // uid 存储在 uid 和 uid_high 字段
            inode.uid = (uid as u16).to_le();
            inode.uid_high = ((uid >> 16) as u16).to_le();

            // gid 存储在 gid 和 gid_high 字段
            inode.gid = (gid as u16).to_le();
            inode.gid_high = ((gid >> 16) as u16).to_le();
        })
    }

    /// 设置访问时间
    ///
    /// # 参数
    ///
    /// * `atime` - Unix 时间戳（秒）
    pub fn set_atime(&mut self, atime: u32) -> Result<()> {
        self.with_inode_mut(|inode| {
            inode.atime = atime.to_le();
        })
    }

    /// 设置修改时间
    ///
    /// # 参数
    ///
    /// * `mtime` - Unix 时间戳（秒）
    pub fn set_mtime(&mut self, mtime: u32) -> Result<()> {
        self.with_inode_mut(|inode| {
            inode.mtime = mtime.to_le();
        })
    }

    /// 设置变更时间
    ///
    /// # 参数
    ///
    /// * `ctime` - Unix 时间戳（秒）
    pub fn set_ctime(&mut self, ctime: u32) -> Result<()> {
        self.with_inode_mut(|inode| {
            inode.ctime = ctime.to_le();
        })
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
        create: bool,
    ) -> Result<u64> {
        use crate::{balloc::BlockAllocator, extent::get_blocks};

        // 检查是否使用 extents
        if !self.has_extents()? {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "Non-extent block mapping not yet supported",
            ));
        }

        if !create {
            // 只读模式：使用 ExtentTree 查找
            let inode_copy = self.get_inode_copy()?;
            let temp_inode = crate::inode::Inode::from_raw(inode_copy, self.inode_num);
            let mut extent_tree = ExtentTree::new(self.bdev, self.sb.block_size());

            match extent_tree.map_block(&temp_inode, logical_block)? {
                Some(physical_block) => Ok(physical_block),
                None => Err(Error::new(
                    ErrorKind::NotFound,
                    "Logical block not found in extent tree",
                )),
            }
        } else {
            // 写入模式：使用 get_blocks 进行分配
            // 安全性说明：
            // - get_blocks 需要 &mut Superblock 但 self 已持有 &mut sb
            // - 使用 unsafe 指针绕过借用检查器
            // - get_blocks 会修改 superblock 的空闲块计数，但不会与 InodeRef 冲突
            let sb_ptr = self.superblock_mut() as *mut Superblock;
            let sb_ref = unsafe { &mut *sb_ptr };

            let mut allocator = BlockAllocator::new();

            let (physical_block, _allocated_count) =
                get_blocks(self, sb_ref, &mut allocator, logical_block, 1, true)?;

            if physical_block == 0 {
                Err(Error::new(
                    ErrorKind::NoSpace,
                    "Failed to allocate block",
                ))
            } else {
                Ok(physical_block)
            }
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

    // ========================================================================
    // xattr 支持方法
    // ========================================================================

    /// 获取 Inode 的只读引用（用于 xattr 等操作）
    ///
    /// 注意：返回的 Inode 不能修改，只能查询
    pub fn get_inode(&mut self) -> Result<crate::inode::Inode> {
        let inode_copy = self.get_inode_copy()?;
        Ok(crate::inode::Inode::from_raw(inode_copy, self.inode_num))
    }

    /// 获取完整的 inode 块数据（用于 xattr）
    ///
    /// 返回包含 inode 的完整块数据
    pub fn get_inode_data(&mut self) -> Result<alloc::vec::Vec<u8>> {
        // 直接从块设备读取 inode 所在的块
        let mut buf = alloc::vec![0u8; self.sb.block_size() as usize];
        self.bdev.read_block(self.inode_block_addr, &mut buf)?;
        Ok(buf)
    }

    /// 获取可修改的 inode 块数据（用于 xattr 写操作）
    ///
    /// 返回包含 inode 的完整块数据（可修改）
    ///
    /// 注意：调用者需要确保修改后调用 write_inode_data() 写回
    pub fn get_inode_data_mut(&mut self) -> Result<alloc::vec::Vec<u8>> {
        // 和 get_inode_data 相同，返回数据副本
        // 调用者负责写回
        self.get_inode_data()
    }

    /// 写回 inode 块数据
    ///
    /// 将修改后的 inode 块数据写回磁盘
    ///
    /// # 参数
    ///
    /// * `data` - 修改后的 inode 块数据
    ///
    /// # 注意
    ///
    /// 这个方法用于 xattr 等需要修改整个 inode 块的操作
    pub fn write_inode_data(&mut self, data: &[u8]) -> Result<()> {
        // 写回整个块
        self.bdev.write_block(self.inode_block_addr, data)?;
        // 标记为 dirty（虽然已经写回，但保持一致性）
        self.dirty = true;
        Ok(())
    }

    /// 读取 xattr block（如果存在）
    ///
    /// 检查 inode.file_acl 字段，如果非零则读取对应的块
    pub fn read_xattr_block(&mut self) -> Result<Option<alloc::vec::Vec<u8>>> {
        let has_64bit = self.sb.has_incompat_feature(EXT4_FEATURE_INCOMPAT_64BIT);
        let file_acl = self.with_inode(|inode| {
            // file_acl 在 32 位字段
            let mut acl = u32::from_le(inode.file_acl_lo) as u64;

            // 检查是否有高 32 位（64 位模式）
            if has_64bit {
                acl |= (u16::from_le(inode.file_acl_high) as u64) << 32;
            }

            acl
        })?;

        if file_acl == 0 {
            return Ok(None);
        }

        // 读取 xattr block
        let mut buf = alloc::vec![0u8; self.sb.block_size() as usize];
        self.bdev.read_block(file_acl, &mut buf)?;
        Ok(Some(buf))
    }

    /// 读取可修改的 xattr block（如果存在）
    ///
    /// 注意：调用者需要确保修改后写回
    pub fn read_xattr_block_mut(&mut self) -> Result<Option<alloc::vec::Vec<u8>>> {
        // 和 read_xattr_block 相同
        self.read_xattr_block()
    }

    // ========================================================================
    // 文件大小和块操作（写操作）
    // ========================================================================

    // 注意：truncate 方法已移到 Ext4FileSystem 层实现
    // 请使用 fs.truncate_file(inode_num, new_size)

    /// 获取 inode 当前文件末尾的逻辑块号
    ///
    /// 用于计算下一个要追加的块位置
    ///
    /// # 返回
    ///
    /// 文件末尾的逻辑块号（下一个块的位置）
    pub fn get_next_logical_block(&mut self) -> Result<u32> {
        let file_size = self.size()?;
        let block_size = self.sb.block_size();

        // 计算当前文件占用的块数（向上取整）
        let blocks = ((file_size + block_size as u64 - 1) / block_size as u64) as u32;

        Ok(blocks)
    }

    /// 计算块分配的目标位置（hint）
    ///
    /// 对应 lwext4 的 `ext4_fs_inode_to_goal_block()`
    ///
    /// # 返回
    ///
    /// 建议的物理块组 ID
    pub fn get_alloc_goal(&self) -> u32 {
        self.inode_num / self.sb.inodes_per_group()
    }
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
