//! 该模块实现目录inode的操作，包括目录条目查找、读取、添加和删除。

use core::{mem, slice};

use crate::{Ext4Result, SystemHal, error::Context, ffi::*, util::revision_tuple};

use super::{InodeRef, InodeType};

impl<Hal: SystemHal> InodeRef<Hal> {
    /// 读取目录条目（从offset开始），返回目录读取器
    pub fn read_dir(mut self, offset: u64) -> Ext4Result<DirReader<Hal>> {
        unsafe {
            let mut iter = mem::zeroed(); // 初始化目录迭代器
            // 调用C函数初始化迭代器
            ext4_dir_iterator_init(&mut iter, self.inner.as_mut(), offset)
                .context("ext4_dir_iterator_init")?;

            Ok(DirReader {
                parent: self,
                inner: iter,
            })
        }
    }

    /// 在目录中查找指定名称的条目
    pub fn lookup(mut self, name: &str) -> Ext4Result<DirLookupResult<Hal>> {
        unsafe {
            let mut result = mem::zeroed(); // 初始化查找结果
            // 调用C函数查找目录条目
            ext4_dir_find_entry(
                &mut result,
                self.inner.as_mut(),
                name.as_ptr() as *const _,
                name.len() as _,
            )
            .context("ext4_dir_find_entry")?;

            Ok(DirLookupResult {
                parent: self,
                inner: result,
            })
        }
    }

    /// 检查目录是否有子目录/文件（非"."和".."）
    pub fn has_children(self) -> Ext4Result<bool> {
        if self.inode_type() != InodeType::Directory {
            return Ok(false);
        }
        let mut reader = self.read_dir(0)?;
        // 遍历目录条目
        while let Some(curr) = reader.current() {
            let name = curr.name();
            // 排除"."和".."
            if name != b"." && name != b".." {
                return Ok(true);
            }
            reader.step()?; // 移动到下一个条目
        }
        Ok(false)
    }

    /// 向目录添加条目（关联名称和inode）
    pub(crate) fn add_entry(&mut self, name: &str, entry: &mut InodeRef<Hal>) -> Ext4Result {
        unsafe {
            // 调用C函数添加目录条目
            ext4_dir_add_entry(
                self.inner.as_mut(),
                name.as_ptr() as *const _,
                name.len() as _,
                entry.inner.as_mut(),
            )
            .context("ext4_dir_add_entry")?;
        }
        entry.inc_nlink(); // 增加inode的链接计数
        Ok(())
    }

    /// 从目录删除条目
    pub(crate) fn remove_entry(&mut self, name: &str, entry: &mut InodeRef<Hal>) -> Ext4Result {
        unsafe {
            // 调用C函数删除目录条目
            ext4_dir_remove_entry(
                self.inner.as_mut(),
                name.as_ptr() as *const _,
                name.len() as _,
            )
            .context("ext4_dir_remove_entry")?;
        }
        entry.dec_nlink(); // 减少inode的链接计数
        Ok(())
    }
}

/// 目录查找结果，包含父目录inode和查找结果
pub struct DirLookupResult<Hal: SystemHal> {
    parent: InodeRef<Hal>,
    inner: ext4_dir_search_result,
}

impl<Hal: SystemHal> DirLookupResult<Hal> {
    /// 获取找到的目录条目
    pub fn entry(&mut self) -> DirEntry {
        DirEntry {
            inner: unsafe { &mut *(self.inner.dentry as *mut _) }, //  unsafe：转换原始指针
            sb: self.parent.superblock(),
        }
    }
}

/// 当DirLookupResult被销毁时，释放底层资源
impl<Hal: SystemHal> Drop for DirLookupResult<Hal> {
    fn drop(&mut self) {
        unsafe {
            ext4_dir_destroy_result(self.parent.inner.as_mut(), &mut self.inner);
        }
    }
}

/// 原始目录条目（封装C结构体ext4_dir_en）
#[repr(transparent)]
pub struct RawDirEntry {
    inner: ext4_dir_en,
}

impl RawDirEntry {
    /// 获取条目的inode编号
    pub fn ino(&self) -> u32 {
        u32::from_le(self.inner.inode) // 转换从小端存储
    }

    /// 设置条目的inode编号
    pub fn set_ino(&mut self, ino: u32) {
        self.inner.inode = u32::to_le(ino); // 转换为小端存储
    }

    /// 获取条目的长度（字节）
    pub fn len(&self) -> u16 {
        u16::from_le(self.inner.entry_len)
    }

    /// 获取条目的名称（字节数组）
    pub fn name<'a>(&'a self, sb: &ext4_sblock) -> &'a [u8] {
        let mut name_len = self.inner.name_len as u16;
        // 处理旧版本的ext4（名称长度可能存储在高位）
        if revision_tuple(sb) < (0, 5) {
            let high = self.inner.in_.name_length_high();  // 方法调用
            name_len |= (high as u16) << 8;
        }
        // 从原始数据中提取名称
        let name_slice = self.inner.name();  // 方法调用获取&[u8]
        &name_slice[..name_len as usize]
    }

    /// 获取条目对应的inode类型
    pub fn inode_type(&self, sb: &ext4_sblock) -> InodeType {
        // 旧版本不支持类型字段
        if revision_tuple(sb) < (0, 5) {
            InodeType::Unknown
        } else {
            // 转换C类型值为InodeType
            match self.inner.in_.inode_type() as u32 {  // 方法调用
                EXT4_DE_DIR => InodeType::Directory,
                EXT4_DE_REG_FILE => InodeType::RegularFile,
                EXT4_DE_SYMLINK => InodeType::Symlink,
                EXT4_DE_CHRDEV => InodeType::CharacterDevice,
                EXT4_DE_BLKDEV => InodeType::BlockDevice,
                EXT4_DE_FIFO => InodeType::Fifo,
                EXT4_DE_SOCK => InodeType::Socket,
                _ => InodeType::Unknown,
            }
        }
    }
}

/// 目录条目（包含超级块引用，用于解析名称和类型）
pub struct DirEntry<'a> {
    inner: &'a mut RawDirEntry,
    sb: &'a ext4_sblock,
}

impl DirEntry<'_> {
    /// 获取inode编号
    pub fn ino(&self) -> u32 {
        self.inner.ino()
    }

    /// 获取名称
    pub fn name(&self) -> &[u8] {
        self.inner.name(self.sb)
    }

    /// 获取inode类型
    pub fn inode_type(&self) -> InodeType {
        self.inner.inode_type(self.sb)
    }

    /// 获取条目长度
    pub fn len(&self) -> u16 {
        self.inner.len()
    }

    /// 检查条目是否为空
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    /// 获取原始条目（不可变）
    pub fn raw_entry(&self) -> &RawDirEntry {
        self.inner
    }

    /// 获取原始条目（可变）
    pub fn raw_entry_mut(&mut self) -> &mut RawDirEntry {
        self.inner
    }
}

/// 目录读取器，用于迭代目录条目
pub struct DirReader<Hal: SystemHal> {
    parent: InodeRef<Hal>, // 父目录inode
    inner: ext4_dir_iter, // 底层C迭代器
}

impl<Hal: SystemHal> DirReader<Hal> {
    /// 获取当前条目（如果存在）
    pub fn current(&self) -> Option<DirEntry> {
        if self.inner.curr.is_null() {
            return None;
        }
        let curr = unsafe { &mut *(self.inner.curr as *mut _) }; //  unsafe：转换原始指针
        let sb = self.parent.superblock();

        Some(DirEntry { inner: curr, sb })
    }

    /// 移动到下一个条目
    pub fn step(&mut self) -> Ext4Result {
        if !self.inner.curr.is_null() {
            unsafe {
                // 调用C函数移动迭代器
                ext4_dir_iterator_next(&mut self.inner).context("ext4_dir_iterator_next")?;
            }
        }
        Ok(())
    }

    /// 获取当前偏移量
    pub fn offset(&self) -> u64 {
        self.inner.curr_off
    }
}

/// 当DirReader被销毁时，释放迭代器资源
impl<Hal: SystemHal> Drop for DirReader<Hal> {
    fn drop(&mut self) {
        unsafe {
            ext4_dir_iterator_fini(&mut self.inner);
        }
    }
}