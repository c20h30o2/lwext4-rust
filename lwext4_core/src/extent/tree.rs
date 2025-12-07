//! Extent 树解析和块映射

use crate::{
    block::{BlockDev, BlockDevice},
    error::{Error, ErrorKind, Result},
    inode::Inode,
    types::{ext4_extent, ext4_extent_header, ext4_extent_idx},
};
use alloc::vec::Vec;

/// Extent 树遍历器
///
/// 用于解析 inode 中的 extent 树并将逻辑块号映射到物理块号
pub struct ExtentTree<'a, D: BlockDevice> {
    bdev: &'a mut BlockDev<D>,
    block_size: u32,
}

impl<'a, D: BlockDevice> ExtentTree<'a, D> {
    /// 创建新的 extent 树遍历器
    pub fn new(bdev: &'a mut BlockDev<D>, block_size: u32) -> Self {
        Self { bdev, block_size }
    }

    /// 将逻辑块号映射到物理块号
    ///
    /// # 参数
    ///
    /// * `inode` - inode 引用
    /// * `logical_block` - 逻辑块号
    ///
    /// # 返回
    ///
    /// 成功返回物理块号，如果找不到对应的 extent 返回 None
    pub fn map_block(&mut self, inode: &Inode, logical_block: u32) -> Result<Option<u64>> {
        // 检查 inode 是否使用 extent
        if !inode.has_extents() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "Inode does not use extents",
            ));
        }

        // extent 树根节点位于 inode 的 blocks 数组中
        // blocks[0..14] 包含 extent 树的根节点数据（60 字节）
        let inode_inner = inode.inner();
        let root_data = unsafe {
            core::slice::from_raw_parts(
                inode_inner.blocks.as_ptr() as *const u8,
                60, // 15 * 4 = 60 bytes
            )
        };

        // 解析根节点的 extent header
        let header = unsafe {
            core::ptr::read_unaligned(root_data.as_ptr() as *const ext4_extent_header)
        };

        if !header.is_valid() {
            return Err(Error::new(
                ErrorKind::Corrupted,
                "Invalid extent header magic",
            ));
        }

        // 从根节点开始查找
        self.find_extent_in_node(root_data, &header, logical_block)
    }

    /// 在给定的节点中查找 extent
    fn find_extent_in_node(
        &mut self,
        node_data: &[u8],
        header: &ext4_extent_header,
        logical_block: u32,
    ) -> Result<Option<u64>> {
        if header.is_leaf() {
            // 叶子节点：包含实际的 extent
            self.search_leaf_node(node_data, header, logical_block)
        } else {
            // 索引节点：包含指向下层节点的索引
            self.search_index_node(node_data, header, logical_block)
        }
    }

    /// 在叶子节点中搜索 extent
    fn search_leaf_node(
        &mut self,
        node_data: &[u8],
        header: &ext4_extent_header,
        logical_block: u32,
    ) -> Result<Option<u64>> {
        let entries = header.entries_count() as usize;
        let header_size = core::mem::size_of::<ext4_extent_header>();
        let extent_size = core::mem::size_of::<ext4_extent>();

        for i in 0..entries {
            let offset = header_size + i * extent_size;
            if offset + extent_size > node_data.len() {
                return Err(Error::new(
                    ErrorKind::Corrupted,
                    "Extent node data too short",
                ));
            }

            let extent = unsafe {
                core::ptr::read_unaligned(
                    node_data[offset..].as_ptr() as *const ext4_extent
                )
            };

            let extent_start = extent.logical_block();
            let extent_len = extent.actual_len() as u32;
            let extent_end = extent_start + extent_len;

            // 检查逻辑块是否在这个 extent 范围内
            if logical_block >= extent_start && logical_block < extent_end {
                let offset_in_extent = logical_block - extent_start;
                let physical_block = extent.physical_block() + offset_in_extent as u64;
                return Ok(Some(physical_block));
            }
        }

        Ok(None)
    }

    /// 在索引节点中搜索
    fn search_index_node(
        &mut self,
        node_data: &[u8],
        header: &ext4_extent_header,
        logical_block: u32,
    ) -> Result<Option<u64>> {
        let entries = header.entries_count() as usize;
        let header_size = core::mem::size_of::<ext4_extent_header>();
        let idx_size = core::mem::size_of::<ext4_extent_idx>();

        // 找到应该包含目标逻辑块的索引
        let mut target_idx: Option<ext4_extent_idx> = None;

        for i in 0..entries {
            let offset = header_size + i * idx_size;
            if offset + idx_size > node_data.len() {
                return Err(Error::new(
                    ErrorKind::Corrupted,
                    "Extent index node data too short",
                ));
            }

            let idx = unsafe {
                core::ptr::read_unaligned(
                    node_data[offset..].as_ptr() as *const ext4_extent_idx
                )
            };

            let idx_block = idx.logical_block();

            // 索引按逻辑块号排序
            // 找到第一个 logical_block >= idx_block 的索引
            if logical_block >= idx_block {
                target_idx = Some(idx);
            } else {
                break;
            }
        }

        if let Some(idx) = target_idx {
            // 读取子节点
            let child_block = idx.leaf_block();
            let mut child_data = alloc::vec![0u8; self.block_size as usize];
            self.bdev.read_block(child_block, &mut child_data)?;

            // 解析子节点的头部
            let child_header = unsafe {
                core::ptr::read_unaligned(child_data.as_ptr() as *const ext4_extent_header)
            };

            if !child_header.is_valid() {
                return Err(Error::new(
                    ErrorKind::Corrupted,
                    "Invalid extent header in child node",
                ));
            }

            // 递归查找
            self.find_extent_in_node(&child_data, &child_header, logical_block)
        } else {
            Ok(None)
        }
    }

    /// 读取文件的某个逻辑块
    ///
    /// # 参数
    ///
    /// * `inode` - inode 引用
    /// * `logical_block` - 逻辑块号
    /// * `buf` - 输出缓冲区（大小应该等于块大小）
    pub fn read_block(
        &mut self,
        inode: &Inode,
        logical_block: u32,
        buf: &mut [u8],
    ) -> Result<()> {
        if buf.len() < self.block_size as usize {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Buffer too small for block",
            ));
        }

        match self.map_block(inode, logical_block)? {
            Some(physical_block) => {
                self.bdev.read_block(physical_block, buf)?;
                Ok(())
            }
            None => Err(Error::new(
                ErrorKind::NotFound,
                "Logical block not found in extent tree",
            )),
        }
    }

    /// 读取文件内容
    ///
    /// # 参数
    ///
    /// * `inode` - inode 引用
    /// * `offset` - 文件内偏移（字节）
    /// * `buf` - 输出缓冲区
    ///
    /// # 返回
    ///
    /// 实际读取的字节数
    pub fn read_file(
        &mut self,
        inode: &Inode,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<usize> {
        let file_size = inode.file_size();

        // 检查偏移是否超出文件大小
        if offset >= file_size {
            return Ok(0);
        }

        // 计算实际可以读取的字节数
        let remaining = file_size - offset;
        let to_read = core::cmp::min(buf.len() as u64, remaining) as usize;

        let block_size = self.block_size as u64;
        let mut bytes_read = 0;

        while bytes_read < to_read {
            let current_offset = offset + bytes_read as u64;
            let block_num = (current_offset / block_size) as u32;
            let block_offset = (current_offset % block_size) as usize;

            // 计算本次读取的字节数
            let bytes_in_block = core::cmp::min(
                block_size as usize - block_offset,
                to_read - bytes_read,
            );

            // 读取块
            let mut block_buf = alloc::vec![0u8; block_size as usize];
            self.read_block(inode, block_num, &mut block_buf)?;

            // 复制数据到输出缓冲区
            buf[bytes_read..bytes_read + bytes_in_block]
                .copy_from_slice(&block_buf[block_offset..block_offset + bytes_in_block]);

            bytes_read += bytes_in_block;
        }

        Ok(bytes_read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ext4_extent_header;

    #[test]
    fn test_extent_header_validation() {
        let mut header = ext4_extent_header::default();
        assert!(!header.is_valid());

        header.magic = 0xF30Au16.to_le();
        assert!(header.is_valid());
    }

    #[test]
    fn test_extent_header_depth() {
        let mut header = ext4_extent_header::default();
        header.magic = 0xF30Au16.to_le();
        header.depth = 0u16.to_le();
        assert!(header.is_leaf());

        header.depth = 1u16.to_le();
        assert!(!header.is_leaf());
    }

    #[test]
    fn test_extent_physical_block() {
        let mut extent = ext4_extent::default();
        extent.start_lo = 0x12345678u32.to_le();
        extent.start_hi = 0xABCDu16.to_le();

        let physical = extent.physical_block();
        assert_eq!(physical, 0x0000ABCD12345678u64);
    }
}
