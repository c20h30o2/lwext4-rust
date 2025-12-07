//! 块操作模块

use crate::consts::*;
use crate::{BlockDevice, Ext4BlockCache, Ext4BlockDevice, Ext4Error, Ext4Result};
use log::debug;

/// 锁定块设备接口
///
/// 如果块设备接口提供了 lock 回调，则调用它。
/// 这用于在多线程环境中保护块设备访问。
pub fn ext4_bdif_lock(bdev: *mut Ext4BlockDevice) {
    unsafe {
        // 检查 lock 函数指针是否存在
        if (*(*bdev).bdif).lock.is_none() {
            return;
        }

        // 调用 lock 函数
        if let Some(lock_fn) = (*(*bdev).bdif).lock {
            let r = lock_fn(bdev);
            debug_assert_eq!(r, EOK, "ext4_bdif_lock failed with error: {}", r);
        }
    }
}

/// 解锁块设备接口
///
/// 如果块设备接口提供了 unlock 回调，则调用它。
pub fn ext4_bdif_unlock(bdev: *mut Ext4BlockDevice) {
    unsafe {
        // 检查 unlock 函数指针是否存在
        if (*(*bdev).bdif).unlock.is_none() {
            return;
        }

        // 调用 unlock 函数
        if let Some(unlock_fn) = (*(*bdev).bdif).unlock {
            let r = unlock_fn(bdev);
            debug_assert_eq!(r, EOK, "ext4_bdif_unlock failed with error: {}", r);
        }
    }
}

/// 初始化块设备（占位实现）
pub fn ext4_block_init(bdev: *mut Ext4BlockDevice) -> i32 {
    // TODO: 初始化块设备
    debug!("ext4_block_init");
    EOK
}

/// 关闭块设备（占位实现）
pub fn ext4_block_fini(bdev: *mut Ext4BlockDevice) -> i32 {
    debug!("ext4_block_fini");
    EOK
}

/// 读取字节（占位实现）
pub fn ext4_block_readbytes(
    bdev: *mut Ext4BlockDevice,
    offset: u64,
    buf: *mut u8,
    len: usize,
) -> i32 {
    // TODO: 实现字节读取
    // 1. 计算起始块号
    // 2. 读取跨越的所有块
    // 3. 复制所需字节到 buf
        debug!("ext4_block_readbytes: offset={}, len={}", offset, len);
        EOK
    
}

/// 写入字节（占位实现）
pub fn ext4_block_writebytes(
    bdev: *mut Ext4BlockDevice,
    offset: u64,
    buf: *const u8,
    len: usize,
) -> i32 {
    // TODO: 实现字节写入
    debug!("ext4_block_writebytes: offset={}, len={}", offset, len);
    EOK
}

/// 刷新块缓存（占位实现）
pub fn ext4_block_cache_flush(bdev: *mut Ext4BlockDevice) -> i32 {
    debug!("ext4_block_cache_flush");
    EOK
}

/// 绑定块缓存（占位实现）
pub fn ext4_block_bind_bcache(bdev: *mut Ext4BlockDevice, bc: *mut Ext4BlockCache) -> i32 {
    debug!("ext4_block_bind_bcache");
    EOK
}

/// 设置逻辑块大小（占位实现）
pub fn ext4_block_set_lb_size(bdev: *mut Ext4BlockDevice, lb_size: u32) {
    unsafe {
        (*bdev).lg_bsize = lb_size;
    }
    debug!("ext4_block_set_lb_size: {}", lb_size);
}

/// 启用/禁用块缓存写回模式（占位实现）
pub fn ext4_block_cache_write_back(bdev: *mut Ext4BlockDevice, enable: i32) -> i32 {
    debug!("ext4_block_cache_write_back: enable={}", enable);
    EOK
}

/// 初始化动态块缓存（占位实现）
pub fn ext4_bcache_init_dynamic(bc: *mut Ext4BlockCache, cnt: u32, itemsize: u32) -> i32 {
    debug!(
        "ext4_bcache_init_dynamic: cnt={}, itemsize={}",
        cnt, itemsize
    );
    unsafe {
        if !bc.is_null() {
            (*bc).cnt = cnt;
            (*bc).itemsize = itemsize;
        }
    }
    EOK
}

/// 销毁动态块缓存（占位实现）
pub fn ext4_bcache_fini_dynamic(bc: *mut Ext4BlockCache) -> i32 {
    debug!("ext4_bcache_fini_dynamic");
    EOK
}

/// 清理块缓存（占位实现）
pub fn ext4_bcache_cleanup(bc: *mut Ext4BlockCache) {
    debug!("ext4_bcache_cleanup");
}

/// 底层块读取（带锁）
fn ext4_bdif_bread(
    bdev: *mut Ext4BlockDevice,
    buf: *mut core::ffi::c_void,
    blk_id: u64,
    blk_cnt: u32,
) -> i32 {
    unsafe {
        ext4_bdif_lock(bdev);

        let bread_fn = (*(*bdev).bdif).bread;
        let r = if let Some(bread) = bread_fn {
            bread(bdev, buf, blk_id, blk_cnt)
        } else {
            ENOTSUP
        };

        (*(*bdev).bdif).bread_ctr += 1;
        ext4_bdif_unlock(bdev);
        r
    }
}

/// 底层块写入（带锁）
fn ext4_bdif_bwrite(
    bdev: *mut Ext4BlockDevice,
    buf: *const core::ffi::c_void,
    blk_id: u64,
    blk_cnt: u32,
) -> i32 {
    unsafe {
        ext4_bdif_lock(bdev);

        let bwrite_fn = (*(*bdev).bdif).bwrite;
        let r = if let Some(bwrite) = bwrite_fn {
            bwrite(bdev, buf, blk_id, blk_cnt)
        } else {
            ENOTSUP
        };

        (*(*bdev).bdif).bwrite_ctr += 1;
        ext4_bdif_unlock(bdev);
        r
    }
}

/// 从块设备直接读取块数据
///
/// 将逻辑块地址转换为物理块地址并读取数据
pub fn ext4_blocks_get_direct(
    bdev: *mut Ext4BlockDevice,
    buf: *mut core::ffi::c_void,
    lba: u64,
    cnt: u32,
) -> i32 {
    unsafe {
        debug_assert!(!bdev.is_null() && !buf.is_null());

        let lg_bsize = (*bdev).lg_bsize as u64;
        let ph_bsize = (*(*bdev).bdif).ph_bsize as u64;
        let part_offset = (*bdev).part_offset;

        // 计算物理块地址
        let pba = (lba * lg_bsize + part_offset) / ph_bsize;
        let pb_cnt = (lg_bsize / ph_bsize) as u32;

        ext4_bdif_bread(bdev, buf, pba, pb_cnt * cnt)
    }
}

/// 向块设备直接写入块数据
///
/// 将逻辑块地址转换为物理块地址并写入数据
pub fn ext4_blocks_set_direct(
    bdev: *mut Ext4BlockDevice,
    buf: *const core::ffi::c_void,
    lba: u64,
    cnt: u32,
) -> i32 {
    unsafe {
        debug_assert!(!bdev.is_null() && !buf.is_null());

        let lg_bsize = (*bdev).lg_bsize as u64;
        let ph_bsize = (*(*bdev).bdif).ph_bsize as u64;
        let part_offset = (*bdev).part_offset;

        // 计算物理块地址
        let pba = (lba * lg_bsize + part_offset) / ph_bsize;
        let pb_cnt = (lg_bsize / ph_bsize) as u32;

        ext4_bdif_bwrite(bdev, buf, pba, pb_cnt * cnt)
    }
}
