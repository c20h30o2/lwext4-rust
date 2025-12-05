//! 该模块实现inode（索引节点）的核心逻辑，包括inode类型、引用管理及元数据操作。

// inode属性操作子模块
mod attr;
// 目录inode操作子模块
mod dir;
// 文件inode操作子模块
mod file;

// 引入内存分配相关类型
use alloc::boxed::Box;
// 对外暴露文件属性和目录相关类型
pub use attr::FileAttr;
pub use dir::{DirEntry, DirLookupResult, DirReader};

// 引入标记类型（用于泛型约束）
use core::marker::PhantomData;

// 引入系统硬件抽象层和FFI绑定
use crate::{SystemHal, ffi::*};

/// inode类型枚举，对应不同的文件系统对象类型
#[repr(u8)]
#[derive(PartialEq, Default, Eq, Clone, Copy, Debug)]
pub enum InodeType {
    #[default]
    Unknown = 0,      // 未知类型
    Fifo = 1,         // 命名管道
    CharacterDevice = 2, // 字符设备
    Directory = 4,    // 目录
    BlockDevice = 6,  // 块设备
    RegularFile = 8,  // 普通文件
    Symlink = 10,     // 符号链接
    Socket = 12,      // 套接字
}

/// 从u8值转换为InodeType
impl From<u8> for InodeType {
    fn from(value: u8) -> Self {
        match value {
            1 => InodeType::Fifo,
            2 => InodeType::CharacterDevice,
            4 => InodeType::Directory,
            6 => InodeType::BlockDevice,
            8 => InodeType::RegularFile,
            10 => InodeType::Symlink,
            12 => InodeType::Socket,
            _ => InodeType::Unknown, // 未知类型默认值
        }
    }
}

/// inode引用结构体，封装了底层C结构体ext4_inode_ref
/// 泛型参数Hal表示系统硬件抽象层
#[repr(transparent)]
pub struct InodeRef<Hal: SystemHal> {
    pub(crate) inner: Box<ext4_inode_ref>, // 内部封装的C结构体
    _phantom: PhantomData<Hal>, // 泛型标记，确保Hal的生命周期
}

impl<Hal: SystemHal> InodeRef<Hal> {
    /// 创建新的InodeRef实例
    pub(crate) fn new(inner: ext4_inode_ref) -> Self {
        Self {
            inner: Box::new(inner),
            _phantom: PhantomData,
        }
    }

    /// 获取inode编号
    pub fn ino(&self) -> u32 {
        self.inner.index
    }

    /// 获取超级块的不可变引用
    pub(crate) fn superblock(&self) -> &ext4_sblock {
        unsafe { &(*self.inner.fs).sb } //  unsafe：直接访问原始指针，需确保有效性
    }

    /// 获取超级块的可变引用
    pub(crate) fn superblock_mut(&mut self) -> &mut ext4_sblock {
        unsafe { &mut (*self.inner.fs).sb } //  unsafe：直接访问原始指针，需确保有效性
    }

    /// 标记inode为"脏"（数据已修改但未写入磁盘）
    pub(crate) fn mark_dirty(&mut self) {
        self.inner.dirty = true;
    }

    /// 增加硬链接计数
    pub(crate) fn inc_nlink(&mut self) {
        unsafe {
            // 调用C函数增加链接计数
            ext4_fs_inode_links_count_inc(self.inner.as_mut());
        }
        self.mark_dirty(); // 标记为脏
    }

    /// 减少硬链接计数
    pub(crate) fn dec_nlink(&mut self) {
        self.set_nlink(self.nlink() - 1);
        self.mark_dirty();
    }

    /// 设置硬链接计数
    pub(crate) fn set_nlink(&mut self, nlink: u16) {
        self.raw_inode_mut().links_count = u16::to_le(nlink); // 转换为小端存储
        self.mark_dirty();
    }

    /// 获取原始inode结构体的不可变引用
    pub(crate) fn raw_inode(&self) -> &ext4_inode {
        unsafe { &*self.inner.inode } //  unsafe：直接访问原始指针
    }

    /// 获取原始inode结构体的可变引用
    pub(crate) fn raw_inode_mut(&mut self) -> &mut ext4_inode {
        unsafe { &mut *self.inner.inode } //  unsafe：直接访问原始指针
    }
}

/// 当InodeRef被销毁时，释放底层资源
impl<Hal: SystemHal> Drop for InodeRef<Hal> {
    fn drop(&mut self) {
        // 调用C函数释放inode引用
        let ret = unsafe { ext4_fs_put_inode_ref(self.inner.as_mut()) };
        if ret != 0 {
            panic!("ext4_fs_put_inode_ref failed: {}", ret);
        }
    }
}