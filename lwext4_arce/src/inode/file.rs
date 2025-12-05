//! 该模块实现文件inode的读写、截断和符号链接等操作。

use core::{
    mem::{self, offset_of},
    slice,
};

use super::InodeRef;

use crate::{
    Ext4Result, InodeType, SystemHal, WritebackGuard, error::Context, ffi::*, util::get_block_size,
};

/// 从缓冲区中提取前cnt个字节，并更新缓冲区剩余部分
fn take<'a>(buf: &mut &'a [u8], cnt: usize) -> &'a [u8] {
    let (first, rem) = buf.split_at(cnt.min(buf.len()));
    *buf = rem;
    first
}

/// 从可变缓冲区中提取前cnt个字节，并更新缓冲区剩余部分
fn take_mut<'a>(buf: &mut &'a mut [u8], cnt: usize) -> &'a mut [u8] {
    let pos = cnt.min(buf.len());
    let (first, rem) = mem::take(buf).split_at_mut(pos);
    *buf = rem;
    first
}

impl<Hal: SystemHal> InodeRef<Hal> {
    /// 获取inode中指定逻辑块对应的物理块号
    fn get_inode_fblock(&mut self, block: u32) -> Ext4Result<u64> {
        unsafe {
            let mut fblock = 0u64;
            // 调用C函数获取物理块号
            ext4_fs_get_inode_dblk_idx(self.inner.as_mut(), block, &mut fblock, true)
                .context("ext4_fs_get_inode_dblk_idx")?;
            Ok(fblock)
        }
    }

    /// 初始化inode中指定逻辑块（分配物理块）
    fn init_inode_fblock(&mut self, block: u32) -> Ext4Result<u64> {
        unsafe {
            let mut fblock = 0u64;
            // 调用C函数初始化物理块
            ext4_fs_init_inode_dblk_idx(self.inner.as_mut(), block, &mut fblock)
                .context("ext4_fs_init_inode_dblk_idx")?;
            Ok(fblock)
        }
    }

    /// 为inode追加一个新的逻辑块（分配并返回物理块号和逻辑块号）
    fn append_inode_fblock(&mut self) -> Ext4Result<(u64, u32)> {
        unsafe {
            let mut fblock = 0u64;
            let mut block = 0u32;
            // 调用C函数追加块
            ext4_fs_append_inode_dblk(self.inner.as_mut(), &mut fblock, &mut block)
                .context("ext4_fs_append_inode_dblk")?;
            Ok((fblock, block))
        }
    }

    /// 从设备读取指定偏移量的字节
    fn read_bytes(&mut self, offset: u64, buf: &mut [u8]) -> Ext4Result<()> {
        unsafe {
            let bdev = (*self.inner.fs).bdev;
            // 调用C函数读取字节
            ext4_block_readbytes(bdev, offset, buf.as_mut_ptr() as _, buf.len() as _)
                .context("ext4_block_readbytes")
        }
    }

    /// 向设备写入指定偏移量的字节
    fn write_bytes(&mut self, offset: u64, buf: &[u8]) -> Ext4Result<()> {
        unsafe {
            let bdev = (*self.inner.fs).bdev;
            // 调用C函数写入字节
            ext4_block_writebytes(bdev, offset, buf.as_ptr() as _, buf.len() as _)
                .context("ext4_block_writebytes")
        }
    }

    /// 从inode读取数据（从偏移量pos开始，写入buf）
    pub fn read_at(&mut self, mut buf: &mut [u8], pos: u64) -> Ext4Result<usize> {
        unsafe {
            let file_size = self.size(); // 文件总大小
            let block_size = get_block_size(self.superblock()); // 块大小
            let bdev = (*self.inner.fs).bdev;

            // 如果偏移量超出文件大小或缓冲区为空，返回0
            if pos >= file_size || buf.is_empty() {
                return Ok(0);
            }
            // 计算实际可读取的字节数
            let to_be_read = buf.len().min((file_size - pos) as usize);
            buf = &mut buf[..to_be_read];

            let inode = self.raw_inode();

            // 处理符号链接的内联数据（短路径直接存储在inode中）
            if self.inode_type() == InodeType::Symlink && file_size < size_of::<[u32; 15]>() as u64
            {
                // 从inode的blocks字段读取内联数据
                let content = (inode as *const _ as *const u8).add(offset_of!(ext4_inode, blocks));
                let buf = take_mut(&mut buf, (file_size - pos) as usize);
                buf.copy_from_slice(slice::from_raw_parts(content.add(pos as usize), buf.len()));
            }

            // 计算起始块和结束块（逻辑块号）
            let mut block_start = (pos / block_size as u64) as u32;
            let block_end = ((pos + buf.len() as u64).min(file_size) / block_size as u64) as u32;

            // 处理块内的偏移量（非块对齐的起始部分）
            let offset = pos % block_size as u64;
            if offset > 0 {
                let buf_segment = take_mut(&mut buf, block_size as usize - offset as usize);
                let fblock = self.get_inode_fblock(block_start)?;
                if fblock != 0 {
                    // 读取物理块中从偏移量开始的数据
                    self.read_bytes(fblock * block_size as u64 + offset, buf_segment)?;
                } else {
                    // 块未分配，填充0
                    buf_segment.fill(0);
                }
                block_start += 1;
            }

            // 启用写回模式（确保缓存一致性）
            let guard = WritebackGuard::new(bdev);

            // 批量读取连续的块（优化性能）
            let mut fblock_start = 0;
            let mut fblock_count = 0;

            // 刷新连续块的读取（内部函数）
            let flush_fblock_segment = |buf: &mut &mut [u8], start: u64, count: u32| {
                if count == 0 {
                    return Ok(());
                }
                let buf_segment = take_mut(buf, count as usize * block_size as usize);
                // 调用C函数批量读取块
                ext4_blocks_get_direct(bdev, buf_segment.as_mut_ptr() as _, start, count)
                    .context("ext4_blocks_get_direct")
            };

            // 处理中间的完整块
            for block in block_start..block_end {
                let fblock = self.get_inode_fblock(block)?;
                // 如果当前块不连续，刷新之前的连续块
                if fblock != fblock_start + fblock_count as u64 {
                    flush_fblock_segment(&mut buf, fblock_start, fblock_count)?;
                    fblock_start = fblock;
                    fblock_count = 0;
                }

                if fblock == 0 {
                    // 块未分配，填充0
                    take_mut(&mut buf, block_size as usize).fill(0);
                } else {
                    fblock_count += 1;
                }
            }
            // 刷新剩余的连续块
            flush_fblock_segment(&mut buf, fblock_start, fblock_count)?;

            drop(guard); // 关闭写回模式

            // 处理块内的剩余部分（非块对齐的结束部分）
            assert!(buf.len() < block_size as usize);
            if !buf.is_empty() {
                let fblock = self.get_inode_fblock(block_end)?;
                if fblock != 0 {
                    self.read_bytes(fblock * block_size as u64, buf)?;
                } else {
                    buf.fill(0);
                }
            }

            Ok(to_be_read)
        }
    }

    /// 向inode写入数据（从偏移量pos开始，读取buf）
    pub fn write_at(&mut self, mut buf: &[u8], pos: u64) -> Ext4Result<usize> {
        unsafe {
            let mut file_size = self.size();
            // 如果写入偏移量超出文件大小，扩展文件
            if pos > file_size {
                self.set_len(pos)?;
                file_size = self.size(); // 更新文件大小
            }

            let block_size = get_block_size(self.superblock());
            let block_count = file_size.div_ceil(block_size as u64) as u32; // 当前块数
            let bdev = (*self.inner.fs).bdev;

            if buf.is_empty() {
                return Ok(0);
            }
            let to_be_written = buf.len();

            // 获取或分配物理块（内部函数）
            let get_fblock = |this: &mut Self, block: u32| -> Ext4Result<u64> {
                if block < block_count {
                    this.init_inode_fblock(block) // 已存在的块，初始化
                } else {
                    let (fblock, new_block) = this.append_inode_fblock()?; // 新块，追加
                    assert_eq!(block, new_block);
                    Ok(fblock)
                }
            };

            // 计算起始块和结束块（逻辑块号）
            let mut block_start = (pos / block_size as u64) as u32;
            let block_end = ((pos + buf.len() as u64) / block_size as u64) as u32;

            // 处理块内的偏移量（非块对齐的起始部分）
            let offset = pos % block_size as u64;
            if offset > 0 {
                let buf_segment = take(&mut buf, block_size as usize - offset as usize);
                let fblock = get_fblock(self, block_start)?;
                // 写入物理块中从偏移量开始的位置
                self.write_bytes(fblock * block_size as u64 + offset, buf_segment)?;
                block_start += 1;
            }

            // 批量写入连续的块（优化性能）
            let mut fblock_start = 0;
            let mut fblock_count = 0;

            // 刷新连续块的写入（内部函数）
            let flush_fblock_segment = |buf: &mut &[u8], start: u64, count: u32| {
                if count == 0 {
                    return Ok(());
                }
                let buf_segment = take(buf, count as usize * block_size as usize);
                // 调用C函数批量写入块
                ext4_blocks_set_direct(bdev, buf_segment.as_ptr() as _, start, count)
                    .context("ext4_blocks_set_direct")
            };

            // 处理中间的完整块
            for block in block_start..block_end {
                let fblock = get_fblock(self, block)?;
                // 如果当前块不连续，刷新之前的连续块
                if fblock != fblock_start + fblock_count as u64 {
                    flush_fblock_segment(&mut buf, fblock_start, fblock_count)?;
                    fblock_start = fblock;
                    fblock_count = 0;
                }
                fblock_count += 1;
            }
            // 刷新剩余的连续块
            flush_fblock_segment(&mut buf, fblock_start, fblock_count)?;

            // 处理块内的剩余部分（非块对齐的结束部分）
            assert!(buf.len() < block_size as usize);
            if !buf.is_empty() {
                let fblock = get_fblock(self, block_end)?;
                self.write_bytes(fblock * block_size as u64, buf)?;
            }

            // 如果写入超出原文件大小，更新文件大小
            let end = pos + to_be_written as u64;
            if end > file_size {
                ext4_inode_set_size(self.inner.inode, end);
                self.mark_dirty();
            }

            Ok(to_be_written)
        }
    }

    /// 截断文件到指定大小
    pub fn truncate(&mut self, size: u64) -> Ext4Result<()> {
        unsafe {
            let bdev = (*self.inner.fs).bdev;
            let _guard = WritebackGuard::new(bdev); // 启用写回模式
            // 调用C函数截断inode
            ext4_fs_truncate_inode(self.inner.as_mut(), size).context("ext4_fs_truncate_inode")
        }
    }

    /// 设置符号链接的目标路径
    pub fn set_symlink(&mut self, target: &[u8]) -> Ext4Result<()> {
        let block_size = get_block_size(self.superblock());
        // 路径过长（超过块大小）
        if target.len() > block_size as usize {
            return 36.context("symlink too long"); // 36对应ENAMETOOLONG
        }

        unsafe {
            // 短路径：直接存储在inode的blocks字段中（内联数据）
            if target.len() < size_of::<u32>() * EXT4_INODE_BLOCKS as usize {
                let ptr = (self.inner.inode as *mut u8).add(offset_of!(ext4_inode, blocks));
                slice::from_raw_parts_mut(ptr, target.len()).copy_from_slice(target);
                ext4_inode_clear_flag(self.inner.inode, EXT4_INODE_FLAG_EXTENTS); // 清除扩展标志
            } else {
                // 长路径：存储在数据块中
                ext4_fs_inode_blocks_init(self.inner.fs, self.inner.as_mut());
                let mut fblock: u64 = 0;
                let mut sblock: u32 = 0;
                // 分配数据块
                ext4_fs_append_inode_dblk(self.inner.as_mut(), &mut fblock, &mut sblock)
                    .context("ext4_fs_append_inode_dblk")?;

                // 写入目标路径到数据块
                let off = fblock * block_size as u64;
                self.write_bytes(off, target)?;
            }
            // 设置符号链接的大小
            ext4_inode_set_size(self.inner.inode, target.len() as u64);
        }

        Ok(())
    }

    /// 设置文件长度（扩展或截断）
    pub fn set_len(&mut self, len: u64) -> Ext4Result<()> {
        static EMPTY: [u8; 4096] = [0; 4096]; // 空数据块（用于填充）

        let cur_len = self.size();
        if len < cur_len {
            self.truncate(len)?;
        } else if len > cur_len {
            // TODO: correct implementation
            let block_size = get_block_size(self.superblock());
            let old_blocks = cur_len.div_ceil(block_size as u64) as u32;
            let new_blocks = len.div_ceil(block_size as u64) as u32;
            for block in old_blocks..new_blocks {
                let (fblock, new_block) = self.append_inode_fblock()?;
                assert_eq!(block, new_block);
                self.write_bytes(fblock * block_size as u64, &EMPTY[..block_size as usize])?;
            }

            // Clear the last block extended part
            let old_last_block = (cur_len / block_size as u64) as u32;
            let old_block_start = (cur_len - (old_last_block as u64 * block_size as u64)) as usize;
            let fblock = self.init_inode_fblock(old_last_block)?;
            assert!(fblock != 0, "fblock should not be zero");
            let length = block_size as usize - old_block_start;
            self.write_bytes(
                fblock * block_size as u64 + old_block_start as u64,
                &EMPTY[..length],
            )?;

            unsafe {
                ext4_inode_set_size(self.inner.inode, len);
            }
            self.mark_dirty();
        }
        Ok(())
    }
}
