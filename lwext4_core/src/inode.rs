//! Inode 操作模块

use log::debug;
use crate::{Ext4Result, Ext4Error, Ext4Filesystem, Ext4InodeRef, Ext4Inode, BlockDevice};
use crate::consts::*;

/// 初始化文件系统并获取 inode 引用（占位实现）
pub fn ext4_fs_get_inode_ref(
    fs: *mut Ext4Filesystem,
    ino: u32,
    inode_ref: *mut Ext4InodeRef,
) -> i32 {
    // TODO: 实现 inode 读取逻辑
    // 1. 计算 inode 所在的块组
    // 2. 计算 inode 在块组中的偏移
    // 3. 读取 inode 数据
    // 4. 填充 inode_ref

    debug!("ext4_fs_get_inode_ref: ino={}", ino);
    EOK  // 临时返回成功
}

/// 释放 inode 引用（占位实现）
pub fn ext4_fs_put_inode_ref(inode_ref: *mut Ext4InodeRef) -> i32 {
    // TODO: 实现 inode 释放逻辑
    // 1. 如果 dirty，写回磁盘
    // 2. 释放内存

    debug!("ext4_fs_put_inode_ref");
    EOK
}

/// 获取 inode 大小
pub fn ext4_inode_get_size(inode: *const Ext4Inode) -> u64 {
    unsafe {
        let size_lo = u32::from_le((*inode).size_lo) as u64;
        let size_hi = u32::from_le((*inode).size_hi) as u64;
        (size_hi << 32) | size_lo
    }
}

/// 设置 inode 大小
pub fn ext4_inode_set_size(inode: *mut Ext4Inode, size: u64) {
    unsafe {
        (*inode).size_lo = (size as u32).to_le();
        (*inode).size_hi = ((size >> 32) as u32).to_le();
    }
}

/// 获取 inode 模式
pub fn ext4_inode_get_mode(inode: *const Ext4Inode) -> u16 {
    unsafe { u16::from_le((*inode).mode) }
}

/// 设置 inode 模式
pub fn ext4_inode_set_mode(inode: *mut Ext4Inode, mode: u16) {
    unsafe { (*inode).mode = mode.to_le(); }
}

/// 获取 inode 块数
pub fn ext4_inode_get_blocks_count(inode: *const Ext4Inode) -> u32 {
    unsafe { u32::from_le((*inode).blocks_count_lo) }
}

/// 设置 inode 删除时间
pub fn ext4_inode_set_del_time(inode: *mut Ext4Inode, time: u32) {
    unsafe { (*inode).dtime = time.to_le(); }
}

/// 清除 inode 标志
pub fn ext4_inode_clear_flag(inode: *mut Ext4Inode, flag: u32) {
    unsafe {
        let flags = u32::from_le((*inode).flags);
        (*inode).flags = (flags & !flag).to_le();
    }
}

/// 增加硬链接计数（占位实现）
pub fn ext4_fs_inode_links_count_inc(inode_ref: *mut Ext4InodeRef) {
    // TODO: 实现链接计数增加
    debug!("ext4_fs_inode_links_count_inc");
}

/// 初始化 inode 块结构（占位实现）
pub fn ext4_fs_inode_blocks_init(fs: *mut Ext4Filesystem, inode_ref: *mut Ext4InodeRef) {
    // TODO: 初始化 inode 的块指针
    debug!("ext4_fs_inode_blocks_init");
}

/// 获取 inode 的第 idx 个数据块号（占位实现）
pub fn ext4_fs_get_inode_dblk_idx(
    inode_ref: *mut Ext4InodeRef,
    idx: u64,
    fblock: *mut u64,
) -> i32 {
    // TODO: 实现块映射逻辑（extent 或传统间接块）
    debug!("ext4_fs_get_inode_dblk_idx: idx={}", idx);
    EOK
}

/// 为 inode 追加数据块（占位实现）
pub fn ext4_fs_append_inode_dblk(inode_ref: *mut Ext4InodeRef, fblock: *mut u64) -> i32 {
    // TODO: 实现块分配和追加
    debug!("ext4_fs_append_inode_dblk");
    EOK
}

/// 分配 inode（占位实现）
pub fn ext4_fs_alloc_inode(
    fs: *mut Ext4Filesystem,
    inode_ref: *mut Ext4InodeRef,
    inode_type: u32,
) -> i32 {
    // TODO: 实现 inode 分配（位图操作）
    debug!("ext4_fs_alloc_inode: type={}", inode_type);
    EOK
}

/// 释放 inode（占位实现）
pub fn ext4_fs_free_inode(inode_ref: *mut Ext4InodeRef) {
    // TODO: 实现 inode 释放（位图操作）
    debug!("ext4_fs_free_inode");
}

/// 截断 inode（占位实现）
pub fn ext4_fs_truncate_inode(inode_ref: *mut Ext4InodeRef, new_size: u64) -> i32 {
    // TODO: 实现文件截断
    debug!("ext4_fs_truncate_inode: new_size={}", new_size);
    EOK
}
