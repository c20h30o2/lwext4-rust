//! 文件系统核心逻辑模块，实现ext4文件系统的初始化、inode管理及文件操作。

use core::{marker::PhantomData, mem, time::Duration};

use alloc::boxed::Box;

use crate::{
    DirLookupResult, DirReader, Ext4Error, Ext4Result, FileAttr, InodeRef, InodeType,
    blockdev::{BlockDevice, Ext4BlockDevice},
    error::Context,
    ffi::*,
    util::get_block_size,
};

/// 系统硬件抽象层（HAL）接口，提供时间相关功能
pub trait SystemHal {
    /// 获取当前时间（可选，用于更新文件的访问/修改时间）
    fn now() -> Option<Duration>;
}

/// 默认的硬件抽象层实现（不提供时间）
pub struct DummyHal;
impl SystemHal for DummyHal {
    fn now() -> Option<Duration> {
        None
    }
}

/// 文件系统配置参数
#[derive(Debug, Clone)]
pub struct FsConfig {
    pub bcache_size: u32, // 块缓存大小
}

impl Default for FsConfig {
    fn default() -> Self {
        Self {
            bcache_size: CONFIG_BLOCK_DEV_CACHE_SIZE, // 使用默认缓存大小
        }
    }
}

/// 文件系统状态信息
#[derive(Debug, Clone)]
pub struct StatFs {
    pub inodes_count: u32,       // 总inode数
    pub free_inodes_count: u32,  // 空闲inode数
    pub blocks_count: u64,       // 总块数
    pub free_blocks_count: u64,  // 空闲块数
    pub block_size: u32,         // 块大小
}

/// ext4文件系统实例结构体
/// 泛型参数：Hal（硬件抽象层）、Dev（块设备）
pub struct Ext4Filesystem<Hal: SystemHal, Dev: BlockDevice> {
    inner: Box<ext4_fs>, // 底层C结构体
    bdev: Ext4BlockDevice<Dev>, // 块设备包装器
    _phantom: PhantomData<Hal>, // 泛型标记
}

impl<Hal: SystemHal, Dev: BlockDevice> Ext4Filesystem<Hal, Dev> {
    /// 创建新的ext4文件系统实例
    pub fn new(dev: Dev, config: FsConfig) -> Ext4Result<Self> {
        // 初始化块设备
        let mut bdev = Ext4BlockDevice::new(dev)?;
        // 初始化文件系统结构体
        let mut fs = Box::new(unsafe { mem::zeroed() });
        unsafe {
            let bd = bdev.inner.as_mut();
            // 初始化ext4文件系统
            ext4_fs_init(&mut *fs, bd, false).context("ext4_fs_init")?;

            // 配置块大小和缓存
            let bs = get_block_size(&fs.sb);
            ext4_block_set_lb_size(bd, bs);
            ext4_bcache_init_dynamic(bd.bc, config.bcache_size, bs)
                .context("ext4_bcache_init_dynamic")?;
            if bs != (*bd.bc).itemsize {
                return Err(Ext4Error::new(ENOTSUP as _, "block size mismatch"));
            }

            // 关联块设备和文件系统
            bd.fs = &mut *fs;

            let mut result = Self {
                inner: fs,
                bdev,
                _phantom: PhantomData,
            };
            let bd = result.bdev.inner.as_mut();
            ext4_block_bind_bcache(bd, bd.bc).context("ext4_block_bind_bcache")?;
            Ok(result)
        }
    }

    /// 获取指定inode编号的InodeRef
    fn inode_ref(&mut self, ino: u32) -> Ext4Result<InodeRef<Hal>> {
        unsafe {
            let mut result = InodeRef::new(mem::zeroed());
            // 调用C函数获取inode引用
            ext4_fs_get_inode_ref(self.inner.as_mut(), ino, result.inner.as_mut())
                .context("ext4_fs_get_inode_ref")?;
            Ok(result)
        }
    }

    /// 克隆inode引用（用于需要多个引用的场景）
    fn clone_ref(&mut self, inode: &InodeRef<Hal>) -> InodeRef<Hal> {
        self.inode_ref(inode.ino()).expect("inode ref clone failed")
    }

    /// 对指定inode执行操作（通过闭包）
    pub fn with_inode_ref<R>(
        &mut self,
        ino: u32,
        f: impl FnOnce(&mut InodeRef<Hal>) -> Ext4Result<R>,
    ) -> Ext4Result<R> {
        let mut inode = self.inode_ref(ino)?;
        f(&mut inode)
    }

    /// 分配新的inode（指定类型）
    pub(crate) fn alloc_inode(&mut self, ty: InodeType) -> Ext4Result<InodeRef<Hal>> {
        unsafe {
            // 转换InodeType为C接口的类型值
            let ty = match ty {
                InodeType::Fifo => EXT4_DE_FIFO,
                InodeType::CharacterDevice => EXT4_DE_CHRDEV,
                InodeType::Directory => EXT4_DE_DIR,
                InodeType::BlockDevice => EXT4_DE_BLKDEV,
                InodeType::RegularFile => EXT4_DE_REG_FILE,
                InodeType::Symlink => EXT4_DE_SYMLINK,
                InodeType::Socket => EXT4_DE_SOCK,
                InodeType::Unknown => EXT4_DE_UNKNOWN,
            };
            let mut result = InodeRef::new(mem::zeroed());
            // 调用C函数分配inode
            ext4_fs_alloc_inode(self.inner.as_mut(), result.inner.as_mut(), ty as _)
                .context("ext4_fs_alloc_inode")?;
            // 初始化inode的块结构
            ext4_fs_inode_blocks_init(self.inner.as_mut(), result.inner.as_mut());
            Ok(result)
        }
    }

    /// 获取指定inode的属性
    pub fn get_attr(&mut self, ino: u32, attr: &mut FileAttr) -> Ext4Result<()> {
        self.inode_ref(ino)?.get_attr(attr);
        Ok(())
    }

    /// 从指定inode读取数据（偏移量pos处）
    pub fn read_at(&mut self, ino: u32, buf: &mut [u8], offset: u64) -> Ext4Result<usize> {
        self.inode_ref(ino)?.read_at(buf, offset)
    }

    /// 向指定inode写入数据（偏移量pos处）
    pub fn write_at(&mut self, ino: u32, buf: &[u8], offset: u64) -> Ext4Result<usize> {
        self.inode_ref(ino)?.write_at(buf, offset)
    }

    /// 设置指定inode的文件大小
    pub fn set_len(&mut self, ino: u32, len: u64) -> Ext4Result<()> {
        self.inode_ref(ino)?.set_len(len)
    }

    /// 设置符号链接的目标路径
    pub fn set_symlink(&mut self, ino: u32, buf: &[u8]) -> Ext4Result<()> {
        self.inode_ref(ino)?.set_symlink(buf)
    }

    /// 在目录inode中查找指定名称的条目
    pub fn lookup(&mut self, parent: u32, name: &str) -> Ext4Result<DirLookupResult<Hal>> {
        self.inode_ref(parent)?.lookup(name)
    }

    /// 读取目录inode中的条目（从偏移量开始）
    pub fn read_dir(&mut self, parent: u32, offset: u64) -> Ext4Result<DirReader<Hal>> {
        self.inode_ref(parent)?.read_dir(offset)
    }

    /// 创建新文件/目录（在parent目录下，指定名称、类型和权限）
    pub fn create(&mut self, parent: u32, name: &str, ty: InodeType, mode: u32) -> Ext4Result<u32> {
        // 分配新inode
        let mut child = self.alloc_inode(ty)?;
        // 获取父目录inode
        let mut parent = self.inode_ref(parent)?;
        // 在父目录中添加条目
        parent.add_entry(name, &mut child)?;

        // 如果是目录，添加"."和".."条目
        if ty == InodeType::Directory {
            child.add_entry(".", &mut self.clone_ref(&child))?; // "."指向自身
            child.add_entry("..", &mut parent)?; // ".."指向父目录
            assert_eq!(child.nlink(), 2); // 目录初始链接数为2
        }

        // 设置文件权限
        child.set_mode((child.mode() & !0o777) | (mode & 0o777));

        Ok(child.ino())
    }

    /// 重命名文件/目录
    pub fn rename(
        &mut self,
        src_dir: u32,
        src_name: &str,
        dst_dir: u32,
        dst_name: &str,
    ) -> Ext4Result {
        let mut src_dir_ref = self.inode_ref(src_dir)?;
        let mut dst_dir_ref = self.inode_ref(dst_dir)?;

        // 先删除目标路径的现有文件（如果存在）
        match self.unlink(dst_dir, dst_name) {
            Ok(_) => {}
            Err(err) if err.code == ENOENT as i32 => {} // 目标不存在，忽略
            Err(err) => return Err(err),
        }

        // 获取源文件的inode
        let src = self.lookup(src_dir, src_name)?.entry().ino();
        let mut src_ref = self.inode_ref(src)?;

        // 如果是目录，更新".."指向
        if src_ref.is_dir() {
            let mut result = self.clone_ref(&src_ref).lookup("..")?;
            result.entry().raw_entry_mut().set_ino(dst_dir); // 更新".."为新父目录
            src_dir_ref.dec_nlink(); // 源目录的链接数减1
            dst_dir_ref.inc_nlink(); // 目标目录的链接数加1
        }

        // 从源目录移除条目，添加到目标目录
        src_dir_ref.remove_entry(src_name, &mut src_ref)?;
        dst_dir_ref.add_entry(dst_name, &mut src_ref)?;

        Ok(())
    }

    /// 创建硬链接
    pub fn link(&mut self, dir: u32, name: &str, child: u32) -> Ext4Result {
        let mut child_ref = self.inode_ref(child)?;
        // 不允许对目录创建硬链接
        if child_ref.is_dir() {
            return Err(Ext4Error::new(EISDIR as _, "cannot link to directory"));
        }
        // 在目录中添加链接条目
        self.inode_ref(dir)?.add_entry(name, &mut child_ref)?;
        Ok(())
    }

    /// 删除文件/目录
    pub fn unlink(&mut self, dir: u32, name: &str) -> Ext4Result {
        let mut dir_ref = self.inode_ref(dir)?;
        // 获取要删除的子inode
        let child = self.clone_ref(&dir_ref).lookup(name)?.entry().ino();
        let mut child_ref = self.inode_ref(child)?;

        // 如果是目录且非空，返回错误
        if self.clone_ref(&child_ref).has_children()? {
            return Err(Ext4Error::new(ENOTEMPTY as _, None));
        }

        // 如果是目录，截断其数据块
        if child_ref.inode_type() == InodeType::Directory {
            let bs = get_block_size(&self.inner.as_mut().sb);
            child_ref.truncate(bs as _)?;
        }

        // 从目录中移除条目
        dir_ref.remove_entry(name, &mut child_ref)?;

        // 更新目录链接数
        if child_ref.is_dir() {
            dir_ref.dec_nlink();
            child_ref.dec_nlink();
        }

        // 如果链接数为0，释放inode
        if child_ref.nlink() == 0 {
            child_ref.truncate(0)?; // 截断数据
            unsafe {
                ext4_inode_set_del_time(child_ref.inner.inode, u32::MAX); // 标记删除时间
                child_ref.mark_dirty();
                ext4_fs_free_inode(child_ref.inner.as_mut()); // 释放inode
            }
        }
        Ok(())
    }

    /// 获取文件系统状态信息
    pub fn stat(&mut self) -> Ext4Result<StatFs> {
        let sb = &mut self.inner.as_mut().sb;
        Ok(StatFs {
            inodes_count: u32::from_le(sb.inodes_count),
            free_inodes_count: u32::from_le(sb.free_inodes_count),
            // 拼接高低位获取总块数
            blocks_count: (u32::from_le(sb.blocks_count_hi) as u64) << 32
                | u32::from_le(sb.blocks_count_lo) as u64,
            // 拼接高低位获取空闲块数
            free_blocks_count: (u32::from_le(sb.free_blocks_count_hi) as u64) << 32
                | u32::from_le(sb.free_blocks_count_lo) as u64,
            block_size: get_block_size(sb),
        })
    }

    /// 刷新缓存到磁盘
    pub fn flush(&mut self) -> Ext4Result<()> {
        unsafe {
            ext4_block_cache_flush(self.bdev.inner.as_mut()).context("ext4_cache_flush")?;
        }
        Ok(())
    }
}

/// 当文件系统实例被销毁时，释放资源
impl<Hal: SystemHal, Dev: BlockDevice> Drop for Ext4Filesystem<Hal, Dev> {
    fn drop(&mut self) {
        unsafe {
            // 关闭文件系统
            let r = ext4_fs_fini(self.inner.as_mut());
            if r != 0 {
                log::error!("ext4_fs_fini failed: {}", Ext4Error::new(r, None));
            }
            // 清理块设备和缓存
            let bdev = self.bdev.inner.as_mut();
            ext4_bcache_cleanup(bdev.bc);
            ext4_block_fini(bdev);
            ext4_bcache_fini_dynamic(bdev.bc);
        }
    }
}

/// 写回守卫：确保离开作用域时刷新缓存
pub(crate) struct WritebackGuard {
    bdev: *mut ext4_blockdev, // 块设备指针
}

impl WritebackGuard {
    /// 创建新的写回守卫（开启写回模式）
    pub fn new(bdev: *mut ext4_blockdev) -> Self {
        unsafe { ext4_block_cache_write_back(bdev, 1) };
        Self { bdev }
    }
}

/// 当写回守卫被销毁时，关闭写回模式
impl Drop for WritebackGuard {
    fn drop(&mut self) {
        unsafe { ext4_block_cache_write_back(self.bdev, 0) };
    }
}