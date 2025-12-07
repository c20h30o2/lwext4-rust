//! 块设备核心类型

use crate::error::Result;

/// 块设备接口
///
/// 实现此 trait 以提供底层块设备访问。
///
/// # 示例
///
/// ```rust,ignore
/// use lwext4_core::{BlockDevice, Result};
///
/// struct MyDevice {
///     // ...
/// }
///
/// impl BlockDevice for MyDevice {
///     fn block_size(&self) -> u32 {
///         4096
///     }
///
///     fn sector_size(&self) -> u32 {
///         512
///     }
///
///     fn total_blocks(&self) -> u64 {
///         1000000
///     }
///
///     fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<usize> {
///         // 实现块读取
///         Ok(count as usize * self.sector_size() as usize)
///     }
///
///     fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<usize> {
///         // 实现块写入
///         Ok(count as usize * self.sector_size() as usize)
///     }
/// }
/// ```
pub trait BlockDevice {
    /// 逻辑块大小（通常 4096）
    fn block_size(&self) -> u32;

    /// 物理扇区大小（通常 512）
    fn sector_size(&self) -> u32;

    /// 总块数
    fn total_blocks(&self) -> u64;

    /// 读取扇区
    ///
    /// # 参数
    ///
    /// * `lba` - 逻辑块地址（以扇区为单位）
    /// * `count` - 要读取的扇区数
    /// * `buf` - 目标缓冲区（大小至少为 count * sector_size）
    ///
    /// # 返回
    ///
    /// 成功返回实际读取的字节数
    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<usize>;

    /// 写入扇区
    ///
    /// # 参数
    ///
    /// * `lba` - 逻辑块地址（以扇区为单位）
    /// * `count` - 要写入的扇区数
    /// * `buf` - 源缓冲区（大小至少为 count * sector_size）
    ///
    /// # 返回
    ///
    /// 成功返回实际写入的字节数
    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<usize>;

    /// 刷新缓存
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    /// 是否只读
    fn is_read_only(&self) -> bool {
        false
    }
}

/// 块设备包装器
///
/// 为 ext4 文件系统提供块级访问，包含统计信息。
pub struct BlockDev<D> {
    /// 底层设备
    device: D,
    /// 分区偏移（字节）
    partition_offset: u64,
    /// 分区大小（字节）
    partition_size: u64,
    /// 读取次数
    read_count: u64,
    /// 写入次数
    write_count: u64,
}

impl<D: BlockDevice> BlockDev<D> {
    /// 创建新的块设备包装器
    pub fn new(device: D) -> Self {
        Self {
            device,
            partition_offset: 0,
            partition_size: 0,
            read_count: 0,
            write_count: 0,
        }
    }

    /// 获取底层设备的引用
    pub fn device(&self) -> &D {
        &self.device
    }

    /// 获取底层设备的可变引用
    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    /// 获取逻辑块大小
    pub fn block_size(&self) -> u32 {
        self.device.block_size()
    }

    /// 获取物理扇区大小
    pub fn sector_size(&self) -> u32 {
        self.device.sector_size()
    }

    /// 获取总块数
    pub fn total_blocks(&self) -> u64 {
        self.device.total_blocks()
    }

    /// 获取读取次数
    pub fn read_count(&self) -> u64 {
        self.read_count
    }

    /// 获取写入次数
    pub fn write_count(&self) -> u64 {
        self.write_count
    }

    /// 设置分区偏移和大小
    ///
    /// # 参数
    ///
    /// * `offset` - 分区起始偏移（字节）
    /// * `size` - 分区大小（字节）
    pub fn set_partition(&mut self, offset: u64, size: u64) {
        self.partition_offset = offset;
        self.partition_size = size;
    }

    /// 获取分区偏移
    pub fn partition_offset(&self) -> u64 {
        self.partition_offset
    }

    /// 获取分区大小
    pub fn partition_size(&self) -> u64 {
        self.partition_size
    }

    // 内部辅助方法

    /// 将逻辑块地址转换为物理扇区地址
    pub(super) fn logical_to_physical(&self, lba: u64) -> u64 {
        let block_size = self.device.block_size() as u64;
        let sector_size = self.device.sector_size() as u64;
        (lba * block_size + self.partition_offset) / sector_size
    }

    /// 每个逻辑块包含的物理扇区数
    pub(super) fn sectors_per_block(&self) -> u32 {
        self.device.block_size() / self.device.sector_size()
    }

    /// 增加读计数
    pub(super) fn inc_read_count(&mut self) {
        self.read_count += 1;
    }

    /// 增加写计数
    pub(super) fn inc_write_count(&mut self) {
        self.write_count += 1;
    }
}
