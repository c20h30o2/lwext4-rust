//! 目录操作模块

use log::debug;
use crate::{Ext4InodeRef, Ext4DirIterator, Ext4DirEntry, Ext4DirSearchResult};
use crate::consts::*;

/// 查找目录项（占位实现）
pub fn ext4_dir_find_entry(
    result: *mut Ext4DirSearchResult,
    parent: *mut Ext4InodeRef,
    name: *const u8,
    name_len: u32,
) -> i32 {
    // TODO: 实现目录项查找
    // 1. 遍历父目录的数据块
    // 2. 解析每个目录项
    // 3. 比较名称

    debug!("ext4_dir_find_entry: name_len={}", name_len);
    ENOENT  // 暂时返回未找到
}

/// 添加目录项（占位实现）
pub fn ext4_dir_add_entry(
    parent: *mut Ext4InodeRef,
    name: *const u8,
    name_len: u32,
    child: *mut Ext4InodeRef,
) -> i32 {
    // TODO: 实现目录项添加
    debug!("ext4_dir_add_entry: name_len={}", name_len);
    EOK
}

/// 删除目录项（占位实现）
pub fn ext4_dir_remove_entry(
    parent: *mut Ext4InodeRef,
    name: *const u8,
    name_len: u32,
) -> i32 {
    // TODO: 实现目录项删除
    debug!("ext4_dir_remove_entry: name_len={}", name_len);
    EOK
}

/// 初始化目录迭代器（占位实现）
pub fn ext4_dir_iterator_init(
    it: *mut Ext4DirIterator,
    inode_ref: *mut Ext4InodeRef,
    pos: u64,
) -> i32 {
    // TODO: 初始化迭代器
    debug!("ext4_dir_iterator_init: pos={}", pos);
    EOK
}

/// 获取下一个目录项（占位实现）
pub fn ext4_dir_iterator_next(it: *mut Ext4DirIterator) -> i32 {
    // TODO: 移动到下一个目录项
    debug!("ext4_dir_iterator_next");
    ENOENT  // 暂时返回结束
}

/// 销毁目录迭代器（占位实现）
pub fn ext4_dir_iterator_fini(it: *mut Ext4DirIterator) -> i32 {
    debug!("ext4_dir_iterator_fini");
    EOK
}

/// 销毁查找结果（占位实现）
pub fn ext4_dir_destroy_result(
    parent: *mut Ext4InodeRef,
    result: *mut Ext4DirSearchResult,
) {
    debug!("ext4_dir_destroy_result");
    // TODO: 释放查找结果占用的资源
}
