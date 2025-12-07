//! 块设备抽象模块，封装底层块设备的读写操作。
//!
//! 此模块提供了 ext4 文件系统所需的块设备接口，基于 lwext4_core 的纯 Rust 实现。

use crate::Ext4Result;

/// 设备的物理块大小（固定为512字节，与ext4规范一致）
pub const EXT4_DEV_BSIZE: usize = 512;

/// 块设备接口，定义了块设备的基本操作
///
/// 这个 trait 需要被底层块设备实现。在 ArceOS 中，需要有一个实现了 BlockDevice trait
/// 的结构体，在其中调用 ArceOS 块设备驱动的方法与磁盘交互，进行磁盘块的读写。
///
/// # 示例
///
/// ```ignore
/// struct MyBlockDevice {
///     // ... 底层设备字段
/// }
///
/// impl BlockDevice for MyBlockDevice {
///     fn block_size(&self) -> u32 {
///         4096 // ext4 逻辑块大小
///     }
///
///     fn physical_block_size(&self) -> u32 {
///         512 // 物理块大小
///     }
///
///     fn total_blocks(&self) -> u64 {
///         // 返回总块数
///     }
///
///     fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Ext4Result<usize> {
///         // 实现块读取
///     }
///
///     fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Ext4Result<usize> {
///         // 实现块写入
///     }
/// }
/// ```
pub use lwext4_core::BlockDevice;

/// ext4 块设备包装器
///
/// 这个结构体封装了 lwext4_core 的 `Ext4BlockDev<D>`，提供了对外的统一接口。
pub struct Ext4BlockDevice<Dev: BlockDevice> {
    /// 内部的 lwext4_core 块设备
    pub(crate) inner: lwext4_core::Ext4BlockDev<Dev>,
}

impl<Dev: BlockDevice> Ext4BlockDevice<Dev> {
    /// 创建新的 Ext4BlockDevice 实例
    ///
    /// # 参数
    ///
    /// * `dev` - 实现了 BlockDevice trait 的底层设备
    ///
    /// # 返回
    ///
    /// 成功返回 Ext4BlockDevice 实例
    pub fn new(dev: Dev) -> Ext4Result<Self> {
        Ok(Self {
            inner: lwext4_core::Ext4BlockDev::new(dev),
        })
    }

    /// 获取内部 Ext4BlockDev 的可变引用
    ///
    /// 用于访问 lwext4_core 提供的块设备操作方法
    pub fn inner_mut(&mut self) -> &mut lwext4_core::Ext4BlockDev<Dev> {
        &mut self.inner
    }

    /// 获取内部 Ext4BlockDev 的引用
    pub fn inner(&self) -> &lwext4_core::Ext4BlockDev<Dev> {
        &self.inner
    }

    /// 获取逻辑块大小
    pub fn lg_bsize(&self) -> u32 {
        self.inner.lg_bsize()
    }

    /// 获取物理块大小
    pub fn ph_bsize(&self) -> u32 {
        self.inner.ph_bsize()
    }

    /// 获取逻辑块数量
    pub fn lg_bcnt(&self) -> u64 {
        self.inner.lg_bcnt()
    }

    /// 获取物理块数量
    pub fn ph_bcnt(&self) -> u64 {
        self.inner.ph_bcnt()
    }

    /// 获取读操作计数
    pub fn bread_ctr(&self) -> u64 {
        self.inner.bread_ctr()
    }

    /// 获取写操作计数
    pub fn bwrite_ctr(&self) -> u64 {
        self.inner.bwrite_ctr()
    }
}
