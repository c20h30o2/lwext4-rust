//! 块设备抽象模块，封装底层块设备的读写操作，适配ext4的C接口。

use core::{
    ffi::{c_int, c_void},
    mem, ptr, slice,
};

use crate::{Ext4Result, error::Context, ffi::*};
use alloc::boxed::Box;

/// 设备的物理块大小（固定为512字节，与ext4规范一致）
pub const EXT4_DEV_BSIZE: usize = 512;

/// 块设备接口，定义了块设备的基本操作
/// 这些是暴露给使用本crate的项目的接口， 如在arceos中， 需要有一个实现了blockdevice trait的bd结构体， 在其中调用arceos块设备驱动的方法与磁盘交互，进行磁盘块的读写
/// ext4filesystem对象需要持有一个fs和一个bd（block device） ，当需要从磁盘读取数据时就会调用bd的blockdevice trait中的方法读写磁盘
/// 可以知道，本crate的使用方只需为...TODO 
pub trait BlockDevice {
    /// 向设备写入块（从block_id开始，写入buf中的数据）
    fn write_blocks(&mut self, block_id: u64, buf: &[u8]) -> Ext4Result<usize>;

    /// 从设备读取块（从block_id开始，读取到buf中）
    fn read_blocks(&mut self, block_id: u64, buf: &mut [u8]) -> Ext4Result<usize>;

    /// 获取设备的总块数
    fn num_blocks(&self) -> Ext4Result<u64>;
}


/// 资源守卫：管理块设备相关资源的生命周期（确保安全释放）
#[allow(dead_code)]
struct ResourceGuard<Dev> {
    dev: Box<Dev>, // 底层块设备实例
    block_buf: Box<[u8; EXT4_DEV_BSIZE]>, // 块缓冲区
    block_cache_buf: Box<ext4_bcache>, // 块缓存
    block_dev_iface: Box<ext4_blockdev_iface>, // 块设备接口（C兼容）
}

/// ext4块设备包装器，适配C接口的块设备实现
pub struct Ext4BlockDevice<Dev: BlockDevice> {
    pub(crate) inner: Box<ext4_blockdev>, // 底层C结构体
    _guard: ResourceGuard<Dev>, // 资源守卫（管理生命周期）
}

impl<Dev: BlockDevice> Ext4BlockDevice<Dev> {
    /// 创建新的Ext4BlockDevice实例
    pub fn new(dev: Dev) -> Ext4Result<Self> {
        let mut dev = Box::new(dev); // 包装底层设备

        // 初始化块缓冲区（用于单个块的读写）
        let mut block_buf = Box::new([0u8; EXT4_DEV_BSIZE]);
        // 初始化块设备接口（C函数指针）
        let mut block_dev_iface = Box::new(ext4_blockdev_iface {
            open: Some(Self::dev_open), // 打开设备
            bread: Some(Self::dev_bread), // 读块
            bwrite: Some(Self::dev_bwrite), // 写块
            close: Some(Self::dev_close), // 关闭设备
            lock: None, // 未实现锁定
            unlock: None, // 未实现解锁
            ph_bsize: EXT4_DEV_BSIZE as u32, // 物理块大小
            ph_bcnt: 0, // 总块数（后续初始化）
            ph_bbuf: block_buf.as_mut_ptr(), // 块缓冲区指针
            ph_refctr: 0, // 引用计数
            bread_ctr: 0, // 读计数
            bwrite_ctr: 0, // 写计数
            p_user: dev.as_mut() as *mut _ as *mut c_void, // 底层设备指针
        });

        // 初始化块缓存
        let mut block_cache_buf: Box<ext4_bcache> = Box::new(unsafe { mem::zeroed() });
        // 初始化块设备结构体
        let mut blockdev = Box::new(ext4_blockdev {
            bdif: block_dev_iface.as_mut(), // 块设备接口
            part_offset: 0, // 分区偏移
            part_size: 0, // 分区大小
            bc: block_cache_buf.as_mut(), // 块缓存
            lg_bsize: 0, // 逻辑块大小（后续设置）
            lg_bcnt: 0, // 逻辑块数（后续设置）
            cache_write_back: 0, // 缓存写回模式
            fs: ptr::null_mut(), // 关联的文件系统（后续设置）
            journal: ptr::null_mut(), // 日志（未使用）
        });

        unsafe {
            // 初始化块设备
            ext4_block_init(blockdev.as_mut()).context("ext4_block_init")?;
            // 启用缓存写回
            ext4_block_cache_write_back(blockdev.as_mut(), 1)
                .context("ext4_block_cache_write_back")
                .inspect_err(|_| {
                    ext4_block_fini(blockdev.as_mut()); // 初始化失败时清理
                })?;
        }

        Ok(Self {
            inner: blockdev,
            _guard: ResourceGuard {
                dev,
                block_buf,
                block_cache_buf,
                block_dev_iface,
            },
        })
    }

    /// 从C接口中解析设备相关字段（辅助函数）
    unsafe fn dev_read_fields<'a>(
        bdev: *mut ext4_blockdev,
    ) -> (
        &'a mut ext4_blockdev,
        &'a mut ext4_blockdev_iface,
        &'a mut Dev,
    ) {
        let bdev = unsafe { &mut *bdev };
        let bdif = unsafe { &mut *bdev.bdif };
        let dev = unsafe { &mut *(bdif.p_user as *mut Dev) };
        (bdev, bdif, dev)
    }

    /// C接口：打开设备（初始化块数）
    unsafe extern "C" fn dev_open(bdev: *mut ext4_blockdev) -> c_int {
        debug!("open ext4 block device");
        let (bdev, bdif, dev) = unsafe { Self::dev_read_fields(bdev) };

        // 获取设备总块数
        bdif.ph_bcnt = match dev.num_blocks() {
            Ok(cur) => cur,
            Err(err) => {
                error!("num_blocks failed: {err:?}");
                return EIO as _; // 输入输出错误
            }
        };

        // 设置分区信息
        bdev.part_offset = 0;
        bdev.part_size = bdif.ph_bcnt * bdif.ph_bsize as u64;
        EOK as _ // 成功
    }

    /// C接口：读块（从设备读取指定块）
    unsafe extern "C" fn dev_bread(
        bdev: *mut ext4_blockdev,
        buf: *mut c_void,
        blk_id: u64,
        blk_cnt: u32,
    ) -> c_int {
        trace!("read ext4 block id={blk_id} count={blk_cnt}");
        if blk_cnt == 0 {
            return EOK as _;
        }

        let (_bdev, bdif, dev) = unsafe { Self::dev_read_fields(bdev) };
        // 计算读取的总字节数
        let buf_len = (bdif.ph_bsize * blk_cnt) as usize;
        // 转换为Rust切片
        let buffer = unsafe { slice::from_raw_parts_mut(buf as *mut u8, buf_len) };
        // 调用底层设备的读方法
        if let Err(err) = dev.read_blocks(blk_id, buffer) {
            error!("read_blocks failed: {err:?}");
            return EIO as _;
        }

        EOK as _
    }

    /// C接口：写块（向设备写入指定块）
    unsafe extern "C" fn dev_bwrite(
        bdev: *mut ext4_blockdev,
        buf: *const c_void,
        blk_id: u64,
        blk_cnt: u32,
    ) -> c_int {
        trace!("write ext4 block id={blk_id} count={blk_cnt}");
        if blk_cnt == 0 {
            return EOK as _;
        }

        let (_bdev, bdif, dev) = unsafe { Self::dev_read_fields(bdev) };
        // 计算写入的总字节数
        let buf_len = (bdif.ph_bsize * blk_cnt) as usize;
        // 转换为Rust切片
        let buffer = unsafe { slice::from_raw_parts(buf as *const u8, buf_len) };
        // 调用底层设备的写方法
        if let Err(err) = dev.write_blocks(blk_id, buffer) {
            error!("write_blocks failed: {err:?}");
            return EIO as _;
        }

        EOK as _
    }

    /// C接口：关闭设备
    unsafe extern "C" fn dev_close(_bdev: *mut ext4_blockdev) -> c_int {
        debug!("close ext4 block device");
        EOK as _
    }
}

/// 当Ext4BlockDevice被销毁时，释放资源
impl<Dev: BlockDevice> Drop for Ext4BlockDevice<Dev> {
    fn drop(&mut self) {
        unsafe {
            let bdev = self.inner.as_mut();
            ext4_block_fini(bdev); // 关闭块设备
        }
    }
}