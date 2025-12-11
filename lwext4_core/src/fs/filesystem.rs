//! Ext4 文件系统核心结构

use crate::{
    block::{BlockDev, BlockDevice},
    dir::{lookup_path, read_dir, DirEntry},
    error::{Error, ErrorKind, Result},
    inode::Inode,
    superblock::Superblock,
};
use alloc::vec::Vec;

use super::{file::File, metadata::FileMetadata, inode_ref::InodeRef, block_group_ref::BlockGroupRef};

/// Ext4 文件系统
///
/// 提供完整的文件系统操作接口
///
/// # 示例
///
/// ```rust,ignore
/// use lwext4_core::{Ext4FileSystem, BlockDev};
///
/// let device = MyBlockDevice::new();
/// let mut bdev = BlockDev::new(device);
/// let mut fs = Ext4FileSystem::mount(&mut bdev)?;
///
/// // 打开文件
/// let mut file = fs.open("/etc/passwd")?;
/// let mut buf = vec![0u8; 1024];
/// let n = file.read(&mut buf)?;
///
/// // 读取目录
/// let entries = fs.read_dir("/bin")?;
/// for entry in entries {
///     println!("{}", entry.name);
/// }
///
/// // 获取文件元数据
/// let metadata = fs.metadata("/etc/passwd")?;
/// println!("File size: {} bytes", metadata.size);
/// ```
pub struct Ext4FileSystem<D: BlockDevice> {
    pub(crate) bdev: BlockDev<D>,
    sb: Superblock,
}

impl<D: BlockDevice> Ext4FileSystem<D> {
    /// 挂载文件系统
    ///
    /// # 参数
    ///
    /// * `bdev` - 块设备包装器
    ///
    /// # 返回
    ///
    /// 成功返回文件系统实例
    ///
    /// # 错误
    ///
    /// - `ErrorKind::Corrupted` - 无效的 superblock
    /// - `ErrorKind::Io` - 设备读取失败
    pub fn mount(mut bdev: BlockDev<D>) -> Result<Self> {
        let sb = Superblock::load(&mut bdev)?;

        Ok(Self { bdev, sb })
    }

    /// 获取 superblock 引用
    pub fn superblock(&self) -> &Superblock {
        &self.sb
    }

    /// 获取块设备引用
    pub fn block_device(&self) -> &BlockDev<D> {
        &self.bdev
    }

    /// 获取可变块设备引用
    pub fn block_device_mut(&mut self) -> &mut BlockDev<D> {
        &mut self.bdev
    }

    /// 获取可变 superblock 引用
    pub fn superblock_mut(&mut self) -> &mut Superblock {
        &mut self.sb
    }

    /// 获取 inode 引用
    ///
    /// # 参数
    ///
    /// * `inode_num` - inode 编号
    ///
    /// # 返回
    ///
    /// 成功返回 InodeRef，自动管理加载和写回
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut inode_ref = fs.get_inode_ref(2)?;
    /// println!("Size: {}", inode_ref.size());
    /// inode_ref.set_size(1024);
    /// inode_ref.mark_dirty();
    /// // 自动写回
    /// ```
    pub fn get_inode_ref(&mut self, inode_num: u32) -> Result<InodeRef<D>> {
        InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)
    }

    /// 获取块组引用
    ///
    /// # 参数
    ///
    /// * `bgid` - 块组 ID
    ///
    /// # 返回
    ///
    /// 成功返回 BlockGroupRef，自动管理加载和写回
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut bg_ref = fs.get_block_group_ref(0)?;
    /// println!("Free blocks: {}", bg_ref.free_blocks_count());
    /// bg_ref.dec_free_blocks(1);
    /// bg_ref.mark_dirty();
    /// // 自动写回
    /// ```
    pub fn get_block_group_ref(&mut self, bgid: u32) -> Result<BlockGroupRef<D>> {
        BlockGroupRef::get(&mut self.bdev, &mut self.sb, bgid)
    }

    /// 打开文件
    ///
    /// # 参数
    ///
    /// * `path` - 文件路径（绝对路径）
    ///
    /// # 返回
    ///
    /// 成功返回文件句柄
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut file = fs.open("/etc/passwd")?;
    /// let mut buf = vec![0u8; 1024];
    /// let n = file.read(&mut buf)?;
    /// ```
    pub fn open(&mut self, path: &str) -> Result<File<D>> {
        let inode_num = lookup_path(&mut self.bdev, &self.sb, path)?;
        let inode = Inode::load(&mut self.bdev, &self.sb, inode_num)?;

        if !inode.is_file() {
            return Err(Error::new(ErrorKind::InvalidInput, "Not a regular file"));
        }

        File::new(&mut self.bdev, &self.sb, inode, inode_num)
    }

    /// 读取目录内容
    ///
    /// # 参数
    ///
    /// * `path` - 目录路径（绝对路径）
    ///
    /// # 返回
    ///
    /// 目录项列表
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let entries = fs.read_dir("/bin")?;
    /// for entry in entries {
    ///     println!("{} (inode: {})", entry.name, entry.inode);
    /// }
    /// ```
    pub fn read_dir(&mut self, path: &str) -> Result<Vec<DirEntry>> {
        let inode_num = lookup_path(&mut self.bdev, &self.sb, path)?;
        let inode = Inode::load(&mut self.bdev, &self.sb, inode_num)?;

        if !inode.is_dir() {
            return Err(Error::new(ErrorKind::InvalidInput, "Not a directory"));
        }

        read_dir(&mut self.bdev, &self.sb, &inode)
    }

    /// 获取文件元数据
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    ///
    /// # 返回
    ///
    /// 文件元数据
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let metadata = fs.metadata("/etc/passwd")?;
    /// println!("Size: {} bytes", metadata.size);
    /// println!("UID: {}, GID: {}", metadata.uid, metadata.gid);
    /// ```
    pub fn metadata(&mut self, path: &str) -> Result<FileMetadata> {
        let inode_num = lookup_path(&mut self.bdev, &self.sb, path)?;
        let inode = Inode::load(&mut self.bdev, &self.sb, inode_num)?;

        Ok(FileMetadata::from_inode(&inode, inode_num))
    }

    /// 检查路径是否存在
    ///
    /// # 参数
    ///
    /// * `path` - 路径（绝对路径）
    pub fn exists(&mut self, path: &str) -> bool {
        lookup_path(&mut self.bdev, &self.sb, path).is_ok()
    }

    /// 检查路径是否是目录
    ///
    /// # 参数
    ///
    /// * `path` - 路径（绝对路径）
    pub fn is_dir(&mut self, path: &str) -> Result<bool> {
        let inode_num = lookup_path(&mut self.bdev, &self.sb, path)?;
        let inode = Inode::load(&mut self.bdev, &self.sb, inode_num)?;
        Ok(inode.is_dir())
    }

    /// 检查路径是否是普通文件
    ///
    /// # 参数
    ///
    /// * `path` - 路径（绝对路径）
    pub fn is_file(&mut self, path: &str) -> Result<bool> {
        let inode_num = lookup_path(&mut self.bdev, &self.sb, path)?;
        let inode = Inode::load(&mut self.bdev, &self.sb, inode_num)?;
        Ok(inode.is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_api() {
        // 这些测试需要实际的块设备和 ext4 文件系统
        // 主要是验证 API 的设计和编译
    }
}
