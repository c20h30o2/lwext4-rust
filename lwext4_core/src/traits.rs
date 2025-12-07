//! Trait 定义模块

use crate::{Ext4Error, Ext4Result};

/// 块设备接口
///
/// 为 ext4 文件系统提供底层块设备访问能力。
/// 实现此 trait 的类型可以作为 ext4 文件系统的存储后端。
///
/// # 示例
///
/// ```ignore
/// use lwext4_core::{BlockDevice, Ext4Result, Ext4Error};
/// use std::fs::File;
/// use std::io::{Read, Write, Seek, SeekFrom};
///
/// struct FileBlockDevice {
///     file: File,
///     block_size: u32,
/// }
///
/// impl BlockDevice for FileBlockDevice {
///     fn block_size(&self) -> u32 {
///         self.block_size
///     }
///
///     fn physical_block_size(&self) -> u32 {
///         512
///     }
///
///     fn total_blocks(&self) -> u64 {
///         // 实现获取总块数
///         0
///     }
///
///     fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Ext4Result<usize> {
///         // 实现块读取
///         Ok(0)
///     }
///
///     fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Ext4Result<usize> {
///         // 实现块写入
///         Ok(0)
///     }
/// }
/// ```
pub trait BlockDevice {
    /// 获取逻辑块大小（字节）
    ///
    /// 这是 ext4 文件系统使用的块大小，通常是 1024, 2048, 4096 字节之一。
    fn block_size(&self) -> u32;

    /// 获取物理块大小（字节）
    ///
    /// 这是底层设备的物理块大小，通常是 512 或 4096 字节。
    fn physical_block_size(&self) -> u32;

    /// 获取总块数
    ///
    /// 返回设备上可用的总块数（以逻辑块大小计算）。
    fn total_blocks(&self) -> u64;

    /// 读取多个块
    ///
    /// # 参数
    ///
    /// * `lba` - 逻辑块地址（起始块号）
    /// * `count` - 要读取的块数
    /// * `buf` - 目标缓冲区（必须足够大以容纳 `count * block_size()` 字节）
    ///
    /// # 返回
    ///
    /// 成功时返回实际读取的字节数，失败时返回错误。
    ///
    /// # 错误
    ///
    /// * `EIO` - I/O 错误
    /// * `EINVAL` - 参数无效（如 lba 超出范围，buf 太小）
    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Ext4Result<usize>;

    /// 写入多个块
    ///
    /// # 参数
    ///
    /// * `lba` - 逻辑块地址（起始块号）
    /// * `count` - 要写入的块数
    /// * `buf` - 源数据缓冲区（必须包含至少 `count * block_size()` 字节）
    ///
    /// # 返回
    ///
    /// 成功时返回实际写入的字节数，失败时返回错误。
    ///
    /// # 错误
    ///
    /// * `EIO` - I/O 错误
    /// * `EINVAL` - 参数无效（如 lba 超出范围，buf 太小）
    /// * `EROFS` - 只读文件系统
    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Ext4Result<usize>;

    /// 刷新缓存
    ///
    /// 确保所有挂起的写操作都已提交到持久存储。
    /// 默认实现为空操作（no-op）。
    ///
    /// # 错误
    ///
    /// * `EIO` - I/O 错误
    fn flush(&mut self) -> Ext4Result<()> {
        Ok(())
    }

    /// 获取设备是否为只读
    ///
    /// 默认实现返回 false（可写）。
    fn is_read_only(&self) -> bool {
        false
    }
}
