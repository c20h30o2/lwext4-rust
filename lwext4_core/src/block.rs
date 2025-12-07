//! 块操作模块
//!
//! 提供块设备的读写操作和缓存管理

use crate::consts::*;
use crate::types::Ext4BlockDev;
use crate::{BlockDevice, Ext4Error, Ext4Result};
use log::debug;

/// 块设备操作实现
impl<D: BlockDevice> Ext4BlockDev<D> {
    /// 直接从块设备读取块
    ///
    /// # 参数
    ///
    /// * `lba` - 逻辑块地址
    /// * `buf` - 目标缓冲区
    ///
    /// # 返回
    ///
    /// 成功返回读取的字节数
    ///
    /// # 对应 C 函数
    ///
    /// `ext4_blocks_get_direct`
    pub fn ext4_blocks_get_direct(&mut self, lba: u64, buf: &mut [u8]) -> Ext4Result<usize> {
        // 计算物理块地址
        let pba = (lba * self.lg_bsize() as u64 + self.part_offset()) / self.ph_bsize() as u64;
        let pb_cnt = (self.lg_bsize() / self.ph_bsize()) as u32;

        // 检查缓冲区大小
        let required_size = (pb_cnt * self.ph_bsize()) as usize;
        if buf.len() < required_size {
            return Err(Ext4Error::new(EINVAL, "buffer too small"));
        }

        // 增加读取计数
        self.inc_bread_ctr();

        // 调用底层设备读取
        self.device_mut().read_blocks(pba, pb_cnt, buf)
    }

    /// 直接向块设备写入块
    ///
    /// # 参数
    ///
    /// * `lba` - 逻辑块地址
    /// * `buf` - 源数据缓冲区
    ///
    /// # 返回
    ///
    /// 成功返回写入的字节数
    ///
    /// # 对应 C 函数
    ///
    /// `ext4_blocks_set_direct`
    pub fn ext4_blocks_set_direct(&mut self, lba: u64, buf: &[u8]) -> Ext4Result<usize> {
        // 计算物理块地址
        let pba = (lba * self.lg_bsize() as u64 + self.part_offset()) / self.ph_bsize() as u64;
        let pb_cnt = (self.lg_bsize() / self.ph_bsize()) as u32;

        // 检查缓冲区大小
        let required_size = (pb_cnt * self.ph_bsize()) as usize;
        if buf.len() < required_size {
            return Err(Ext4Error::new(EINVAL, "buffer too small"));
        }

        // 增加写入计数
        self.inc_bwrite_ctr();

        // 调用底层设备写入
        self.device_mut().write_blocks(pba, pb_cnt, buf)
    }

    /// 按字节偏移读取数据
    ///
    /// 支持跨块读取，自动处理块边界
    ///
    /// # 参数
    ///
    /// * `offset` - 字节偏移量
    /// * `buf` - 目标缓冲区
    ///
    /// # 返回
    ///
    /// 成功返回读取的字节数
    ///
    /// # 对应 C 函数
    ///
    /// `ext4_block_readbytes`
    pub fn ext4_block_readbytes(&mut self, offset: u64, buf: &mut [u8]) -> Ext4Result<usize> {
        let len = buf.len();
        let lg_bsize = self.lg_bsize() as u64;

        // 计算起始块号和块内偏移
        let start_block = offset / lg_bsize;
        let block_offset = (offset % lg_bsize) as usize;

        // 计算需要读取的块数
        let total_size = block_offset + len;
        let block_count = ((total_size + lg_bsize as usize - 1) / lg_bsize as usize) as u64;

        // 分配临时缓冲区
        let mut temp_buf = alloc::vec![0u8; (block_count * lg_bsize) as usize];

        // 读取所有相关块
        for i in 0..block_count {
            let lba = start_block + i;
            let block_buf = &mut temp_buf[(i * lg_bsize) as usize..((i + 1) * lg_bsize) as usize];
            self.ext4_blocks_get_direct(lba, block_buf)?;
        }

        // 复制所需字节到目标缓冲区
        buf.copy_from_slice(&temp_buf[block_offset..block_offset + len]);

        Ok(len)
    }

    /// 按字节偏移写入数据
    ///
    /// 支持跨块写入，自动处理块边界
    ///
    /// # 参数
    ///
    /// * `offset` - 字节偏移量
    /// * `buf` - 源数据缓冲区
    ///
    /// # 返回
    ///
    /// 成功返回写入的字节数
    ///
    /// # 对应 C 函数
    ///
    /// `ext4_block_writebytes`
    pub fn ext4_block_writebytes(&mut self, offset: u64, buf: &[u8]) -> Ext4Result<usize> {
        let len = buf.len();
        let lg_bsize = self.lg_bsize() as u64;

        // 计算起始块号和块内偏移
        let start_block = offset / lg_bsize;
        let block_offset = (offset % lg_bsize) as usize;

        // 计算需要写入的块数
        let total_size = block_offset + len;
        let block_count = ((total_size + lg_bsize as usize - 1) / lg_bsize as usize) as u64;

        // 分配临时缓冲区
        let mut temp_buf = alloc::vec![0u8; (block_count * lg_bsize) as usize];

        // 如果写入不是块对齐的，需要先读取现有数据
        if block_offset != 0 || len % lg_bsize as usize != 0 {
            for i in 0..block_count {
                let lba = start_block + i;
                let block_buf = &mut temp_buf[(i * lg_bsize) as usize..((i + 1) * lg_bsize) as usize];
                // 忽略读取错误（可能是新块）
                let _ = self.ext4_blocks_get_direct(lba, block_buf);
            }
        }

        // 将数据写入临时缓冲区
        temp_buf[block_offset..block_offset + len].copy_from_slice(buf);

        // 写回所有相关块
        for i in 0..block_count {
            let lba = start_block + i;
            let block_buf = &temp_buf[(i * lg_bsize) as usize..((i + 1) * lg_bsize) as usize];
            self.ext4_blocks_set_direct(lba, block_buf)?;
        }

        Ok(len)
    }

    /// 刷新缓存到设备
    ///
    /// # 对应 C 函数
    ///
    /// `ext4_block_cache_flush`
    pub fn ext4_block_cache_flush(&mut self) -> Ext4Result<()> {
        debug!("ext4_block_cache_flush");
        self.device_mut().flush()
    }
}

//=============================================================================
// 自由函数形式的 API（保持 C 风格命名以便对照实现）
//=============================================================================

/// 直接从块设备读取块（自由函数形式）
///
/// # 对应 C 函数
///
/// `ext4_blocks_get_direct`
pub fn ext4_blocks_get_direct<D: BlockDevice>(
    bdev: &mut Ext4BlockDev<D>,
    lba: u64,
    buf: &mut [u8],
) -> Ext4Result<usize> {
    bdev.ext4_blocks_get_direct(lba, buf)
}

/// 直接向块设备写入块（自由函数形式）
///
/// # 对应 C 函数
///
/// `ext4_blocks_set_direct`
pub fn ext4_blocks_set_direct<D: BlockDevice>(
    bdev: &mut Ext4BlockDev<D>,
    lba: u64,
    buf: &[u8],
) -> Ext4Result<usize> {
    bdev.ext4_blocks_set_direct(lba, buf)
}

/// 按字节偏移读取数据（自由函数形式）
///
/// # 对应 C 函数
///
/// `ext4_block_readbytes`
pub fn ext4_block_readbytes<D: BlockDevice>(
    bdev: &mut Ext4BlockDev<D>,
    offset: u64,
    buf: &mut [u8],
) -> Ext4Result<usize> {
    bdev.ext4_block_readbytes(offset, buf)
}

/// 按字节偏移写入数据（自由函数形式）
///
/// # 对应 C 函数
///
/// `ext4_block_writebytes`
pub fn ext4_block_writebytes<D: BlockDevice>(
    bdev: &mut Ext4BlockDev<D>,
    offset: u64,
    buf: &[u8],
) -> Ext4Result<usize> {
    bdev.ext4_block_writebytes(offset, buf)
}

/// 刷新块缓存（自由函数形式）
///
/// # 对应 C 函数
///
/// `ext4_block_cache_flush`
pub fn ext4_block_cache_flush<D: BlockDevice>(bdev: &mut Ext4BlockDev<D>) -> Ext4Result<()> {
    bdev.ext4_block_cache_flush()
}

/// 初始化块设备（占位实现）
///
/// # 对应 C 函数
///
/// `ext4_block_init`
pub fn ext4_block_init<D: BlockDevice>(_bdev: &mut Ext4BlockDev<D>) -> Ext4Result<()> {
    debug!("ext4_block_init");
    Ok(())
}

/// 关闭块设备（占位实现）
///
/// # 对应 C 函数
///
/// `ext4_block_fini`
pub fn ext4_block_fini<D: BlockDevice>(_bdev: &mut Ext4BlockDev<D>) -> Ext4Result<()> {
    debug!("ext4_block_fini");
    Ok(())
}

/// 设置逻辑块大小（占位实现）
///
/// # 对应 C 函数
///
/// `ext4_block_set_lb_size`
pub fn ext4_block_set_lb_size<D: BlockDevice>(_bdev: &mut Ext4BlockDev<D>, lb_size: u32) {
    debug!("ext4_block_set_lb_size: {}", lb_size);
    // TODO: 实现设置逻辑块大小
}

/// 启用/禁用块缓存写回模式（占位实现）
///
/// # 对应 C 函数
///
/// `ext4_block_cache_write_back`
pub fn ext4_block_cache_write_back<D: BlockDevice>(
    _bdev: &mut Ext4BlockDev<D>,
    enable: bool,
) -> Ext4Result<()> {
    debug!("ext4_block_cache_write_back: enable={}", enable);
    Ok(())
}
