//! 块 I/O 操作实现

use super::{BlockDev, BlockDevice};
use crate::error::{Error, ErrorKind, Result};
use alloc::vec;

impl<D: BlockDevice> BlockDev<D> {
    /// 读取单个逻辑块
    ///
    /// 从指定逻辑块地址读取一个完整的块到缓冲区。
    ///
    /// # 参数
    ///
    /// * `lba` - 逻辑块地址
    /// * `buf` - 目标缓冲区（大小至少为 block_size）
    ///
    /// # 返回
    ///
    /// 成功返回读取的字节数
    pub fn read_block(&mut self, lba: u64, buf: &mut [u8]) -> Result<usize> {
        let block_size = self.device().block_size();

        if buf.len() < block_size as usize {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "buffer too small for block",
            ));
        }

        let pba = self.logical_to_physical(lba);
        let count = self.sectors_per_block();

        self.inc_read_count();
        self.device_mut().read_blocks(pba, count, buf)
    }

    /// 写入单个逻辑块
    ///
    /// 将缓冲区数据写入指定逻辑块地址。
    ///
    /// # 参数
    ///
    /// * `lba` - 逻辑块地址
    /// * `buf` - 源数据缓冲区（大小至少为 block_size）
    ///
    /// # 返回
    ///
    /// 成功返回写入的字节数
    pub fn write_block(&mut self, lba: u64, buf: &[u8]) -> Result<usize> {
        let block_size = self.device().block_size();

        if buf.len() < block_size as usize {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "buffer too small for block",
            ));
        }

        let pba = self.logical_to_physical(lba);
        let count = self.sectors_per_block();

        self.inc_write_count();
        self.device_mut().write_blocks(pba, count, buf)
    }

    /// 读取字节
    ///
    /// 从任意字节偏移读取，自动处理跨块情况。
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
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut buf = vec![0u8; 100];
    /// block_dev.read_bytes(1024, &mut buf)?;
    /// ```
    pub fn read_bytes(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let len = buf.len();
        let block_size = self.device().block_size() as u64;

        // 计算起始块和块内偏移
        let start_block = offset / block_size;
        let block_offset = (offset % block_size) as usize;

        // 计算需要读取的块数
        let total_size = block_offset + len;
        let block_count = ((total_size as u64 + block_size - 1) / block_size) as usize;

        // 分配临时缓冲区
        let mut temp = vec![0u8; block_count * block_size as usize];

        // 读取所有相关块
        for i in 0..block_count {
            let lba = start_block + i as u64;
            let block_buf = &mut temp[i * block_size as usize..(i + 1) * block_size as usize];
            self.read_block(lba, block_buf)?;
        }

        // 复制所需字节
        buf.copy_from_slice(&temp[block_offset..block_offset + len]);

        Ok(len)
    }

    /// 写入字节
    ///
    /// 向任意字节偏移写入，自动处理跨块情况。
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
    /// # 示例
    ///
    /// ```rust,ignore
    /// let data = b"Hello, ext4!";
    /// block_dev.write_bytes(1024, data)?;
    /// ```
    pub fn write_bytes(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        let len = buf.len();
        let block_size = self.device().block_size() as u64;

        let start_block = offset / block_size;
        let block_offset = (offset % block_size) as usize;

        let total_size = block_offset + len;
        let block_count = ((total_size as u64 + block_size - 1) / block_size) as usize;

        let mut temp = vec![0u8; block_count * block_size as usize];

        // 如果不是块对齐，需要先读取现有数据
        if block_offset != 0 || len % block_size as usize != 0 {
            for i in 0..block_count {
                let lba = start_block + i as u64;
                let block_buf =
                    &mut temp[i * block_size as usize..(i + 1) * block_size as usize];
                // 忽略读取错误（可能是新块）
                let _ = self.read_block(lba, block_buf);
            }
        }

        // 写入数据到临时缓冲区
        temp[block_offset..block_offset + len].copy_from_slice(buf);

        // 写回所有块
        for i in 0..block_count {
            let lba = start_block + i as u64;
            let block_buf = &temp[i * block_size as usize..(i + 1) * block_size as usize];
            self.write_block(lba, block_buf)?;
        }

        Ok(len)
    }

    /// 刷新所有缓存
    ///
    /// 确保所有待写入数据已写入设备。
    pub fn flush(&mut self) -> Result<()> {
        self.device_mut().flush()
    }
}
