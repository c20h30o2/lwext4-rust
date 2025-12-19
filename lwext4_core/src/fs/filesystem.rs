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

/// 文件系统统计信息
#[derive(Debug, Clone)]
pub struct FileSystemStats {
    /// 块大小（字节）
    pub block_size: u32,
    /// 总块数
    pub blocks_total: u64,
    /// 空闲块数
    pub blocks_free: u64,
    /// 可用块数（考虑保留块）
    pub blocks_available: u64,
    /// 总 inode 数
    pub inodes_total: u32,
    /// 空闲 inode 数
    pub inodes_free: u32,
    /// 文件系统 ID
    pub filesystem_id: u64,
    /// 最大文件名长度
    pub max_filename_len: u32,
}

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

    /// 卸载文件系统
    ///
    /// 显式卸载文件系统，确保所有数据写回磁盘。
    ///
    /// # 返回
    ///
    /// 成功时返回底层的块设备，失败时返回错误
    ///
    /// # 注意
    ///
    /// - 此方法会消费 `self`，之后无法再使用该文件系统实例
    /// - 确保所有文件句柄已经关闭
    /// - 自动写回 superblock
    /// - 同步块设备缓存
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut fs = Ext4FileSystem::mount(bdev)?;
    /// // ... 进行文件系统操作 ...
    /// let bdev = fs.unmount()?; // 显式卸载
    /// ```
    ///
    /// # 与 Drop 的区别
    ///
    /// 如果不调用此方法，`Ext4FileSystem` 被 drop 时不会自动刷新数据。
    /// 建议显式调用此方法以确保数据完整性。
    pub fn unmount(mut self) -> Result<BlockDev<D>> {
        // 1. 写回 superblock
        self.sb.write(&mut self.bdev)?;

        // 2. 同步块设备（确保所有写操作完成）
        // 注意：BlockDev 目前没有显式的 sync 方法，
        // 但所有写操作都是同步的，所以数据已经在磁盘上

        // 3. 返回块设备的所有权
        Ok(self.bdev)
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

    /// 获取文件系统统计信息
    ///
    /// # 返回
    ///
    /// 文件系统使用情况统计
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let stats = fs.stats()?;
    /// println!("Total blocks: {}", stats.blocks_total);
    /// println!("Free blocks: {}", stats.blocks_free);
    /// println!("Free inodes: {}", stats.inodes_free);
    /// ```
    pub fn stats(&self) -> Result<FileSystemStats> {
        let sb_inner = self.sb.inner();

        Ok(FileSystemStats {
            block_size: self.sb.block_size(),
            blocks_total: u32::from_le(sb_inner.blocks_count_lo) as u64
                | ((u32::from_le(sb_inner.blocks_count_hi) as u64) << 32),
            blocks_free: u32::from_le(sb_inner.free_blocks_count_lo) as u64
                | ((u32::from_le(sb_inner.free_blocks_count_hi) as u64) << 32),
            blocks_available: {
                let free = u32::from_le(sb_inner.free_blocks_count_lo) as u64
                    | ((u32::from_le(sb_inner.free_blocks_count_hi) as u64) << 32);
                let reserved = u32::from_le(sb_inner.r_blocks_count_lo) as u64
                    | ((u32::from_le(sb_inner.r_blocks_count_hi) as u64) << 32);
                free.saturating_sub(reserved)
            },
            inodes_total: u32::from_le(sb_inner.inodes_count),
            inodes_free: u32::from_le(sb_inner.free_inodes_count),
            filesystem_id: {
                // UUID 的前 8 字节作为文件系统 ID
                let uuid = &sb_inner.uuid;
                u64::from_le_bytes([
                    uuid[0], uuid[1], uuid[2], uuid[3],
                    uuid[4], uuid[5], uuid[6], uuid[7],
                ])
            },
            max_filename_len: 255, // EXT4_NAME_LEN
        })
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
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
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
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

        if !inode_ref.is_dir()? {
            return Err(Error::new(ErrorKind::InvalidInput, "Not a directory"));
        }

        read_dir(&mut inode_ref)
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
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let inode = Inode::load(&mut self.bdev, &self.sb, inode_num)?;

        Ok(FileMetadata::from_inode(&inode, inode_num))
    }

    /// 检查路径是否存在
    ///
    /// # 参数
    ///
    /// * `path` - 路径（绝对路径）
    pub fn exists(&mut self, path: &str) -> bool {
        lookup_path(&mut self.bdev, &mut self.sb, path).is_ok()
    }

    /// 检查路径是否是目录
    ///
    /// # 参数
    ///
    /// * `path` - 路径（绝对路径）
    pub fn is_dir(&mut self, path: &str) -> Result<bool> {
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let inode = Inode::load(&mut self.bdev, &self.sb, inode_num)?;
        Ok(inode.is_dir())
    }

    /// 检查路径是否是普通文件
    ///
    /// # 参数
    ///
    /// * `path` - 路径（绝对路径）
    pub fn is_file(&mut self, path: &str) -> Result<bool> {
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let inode = Inode::load(&mut self.bdev, &self.sb, inode_num)?;
        Ok(inode.is_file())
    }

    // ========== Metadata Write Operations ==========

    /// 修改文件/目录权限
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    /// * `mode` - Unix 权限位（0o000 - 0o7777）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 设置为 rw-r--r-- (0o644)
    /// fs.set_mode("/tmp/test.txt", 0o644)?;
    ///
    /// // 设置为 rwxr-xr-x (0o755)
    /// fs.set_mode("/usr/bin/app", 0o755)?;
    /// ```
    pub fn set_mode(&mut self, path: &str, mode: u16) -> Result<()> {
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let mut inode_ref = self.get_inode_ref(inode_num)?;
        inode_ref.set_mode(mode)?;
        inode_ref.mark_dirty()?;
        Ok(())
    }

    /// 修改文件/目录所有者
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    /// * `uid` - 用户 ID
    /// * `gid` - 组 ID
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 修改所有者为 root:root
    /// fs.set_owner("/tmp/test.txt", 0, 0)?;
    ///
    /// // 修改所有者为 user:group
    /// fs.set_owner("/home/user/file.txt", 1000, 1000)?;
    /// ```
    pub fn set_owner(&mut self, path: &str, uid: u32, gid: u32) -> Result<()> {
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let mut inode_ref = self.get_inode_ref(inode_num)?;
        inode_ref.set_owner(uid, gid)?;
        inode_ref.mark_dirty()?;
        Ok(())
    }

    /// 修改访问时间
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    /// * `atime` - Unix 时间戳（秒）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// let now = SystemTime::now()
    ///     .duration_since(UNIX_EPOCH)
    ///     .unwrap()
    ///     .as_secs() as u32;
    /// fs.set_atime("/tmp/test.txt", now)?;
    /// ```
    pub fn set_atime(&mut self, path: &str, atime: u32) -> Result<()> {
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let mut inode_ref = self.get_inode_ref(inode_num)?;
        inode_ref.set_atime(atime)?;
        inode_ref.mark_dirty()?;
        Ok(())
    }

    /// 修改修改时间
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    /// * `mtime` - Unix 时间戳（秒）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// let now = SystemTime::now()
    ///     .duration_since(UNIX_EPOCH)
    ///     .unwrap()
    ///     .as_secs() as u32;
    /// fs.set_mtime("/tmp/test.txt", now)?;
    /// ```
    pub fn set_mtime(&mut self, path: &str, mtime: u32) -> Result<()> {
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let mut inode_ref = self.get_inode_ref(inode_num)?;
        inode_ref.set_mtime(mtime)?;
        inode_ref.mark_dirty()?;
        Ok(())
    }

    /// 修改变更时间
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    /// * `ctime` - Unix 时间戳（秒）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// let now = SystemTime::now()
    ///     .duration_since(UNIX_EPOCH)
    ///     .unwrap()
    ///     .as_secs() as u32;
    /// fs.set_ctime("/tmp/test.txt", now)?;
    /// ```
    pub fn set_ctime(&mut self, path: &str, ctime: u32) -> Result<()> {
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let mut inode_ref = self.get_inode_ref(inode_num)?;
        inode_ref.set_ctime(ctime)?;
        inode_ref.mark_dirty()?;
        Ok(())
    }

    // ========== Extended Attributes (xattr) API ==========

    /// 列出文件/目录的所有扩展属性
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    ///
    /// # 返回
    ///
    /// 扩展属性名称列表（Vec<String>）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let attrs = fs.listxattr("/etc/passwd")?;
    /// for attr in attrs {
    ///     println!("Attribute: {}", attr);
    /// }
    /// ```
    pub fn listxattr(&mut self, path: &str) -> Result<Vec<alloc::string::String>> {
        use crate::xattr;
        use alloc::string::String;

        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;

        // 读取所有需要的数据，然后释放 inode_ref
        let (inode, inode_data, xattr_block_data) = {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;
            let inode_data = inode_ref.get_inode_data()?;
            let xattr_block_data = inode_ref.read_xattr_block()?;
            let inode = inode_ref.get_inode()?;
            (inode, inode_data, xattr_block_data)
        }; // inode_ref 在这里被 drop

        // 调用底层 xattr API
        let mut buffer = alloc::vec![0u8; 4096]; // 4KB 缓冲区
        let len = xattr::list(
            &self.sb,
            &inode,
            &inode_data,
            xattr_block_data.as_deref(),
            &mut buffer,
        )?;

        // 解析结果（以 \0 分隔的字符串列表）
        let mut result = Vec::new();
        let mut start = 0;
        for i in 0..len {
            if buffer[i] == 0 {
                if i > start {
                    let name = String::from_utf8_lossy(&buffer[start..i]).into_owned();
                    result.push(name);
                }
                start = i + 1;
            }
        }

        Ok(result)
    }

    /// 获取扩展属性的值
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    /// * `name` - 属性名（含前缀，如 "user.comment"）
    ///
    /// # 返回
    ///
    /// 属性值（Vec<u8>）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let value = fs.getxattr("/etc/passwd", "user.comment")?;
    /// let text = String::from_utf8_lossy(&value);
    /// println!("Comment: {}", text);
    /// ```
    pub fn getxattr(&mut self, path: &str, name: &str) -> Result<Vec<u8>> {
        use crate::xattr;

        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;

        // 读取所有需要的数据，然后释放 inode_ref
        let (inode, inode_data, xattr_block_data) = {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;
            let inode_data = inode_ref.get_inode_data()?;
            let xattr_block_data = inode_ref.read_xattr_block()?;
            let inode = inode_ref.get_inode()?;
            (inode, inode_data, xattr_block_data)
        }; // inode_ref 在这里被 drop

        // 调用底层 xattr API
        let mut buffer = alloc::vec![0u8; 65536]; // 64KB 缓冲区（xattr 值最大 64KB）
        let len = xattr::get(
            &self.sb,
            &inode,
            &inode_data,
            xattr_block_data.as_deref(),
            name,
            &mut buffer,
        )?;

        buffer.truncate(len);
        Ok(buffer)
    }

    /// 设置扩展属性
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    /// * `name` - 属性名（含前缀，如 "user.comment"）
    /// * `value` - 属性值
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// fs.setxattr("/etc/passwd", "user.comment", b"System password file")?;
    /// ```
    pub fn setxattr(&mut self, path: &str, name: &str, value: &[u8]) -> Result<()> {
        use crate::{xattr, balloc::BlockAllocator};

        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;
        let block_size = self.sb.block_size();

        // 第一次尝试：使用现有的 xattr 空间
        let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

        let mut inode_data = inode_ref.get_inode_data_mut()?;
        let mut xattr_block_data = inode_ref.read_xattr_block_mut()?;
        let inode = inode_ref.get_inode()?;

        let sb_ptr = inode_ref.superblock_mut() as *mut crate::superblock::Superblock;
        let sb_ref = unsafe { &*sb_ptr };

        let result = xattr::set(
            sb_ref,
            &inode,
            &mut inode_data,
            xattr_block_data.as_deref_mut(),
            name,
            value,
        );

        match result {
            Ok(_) => {
                // 成功，写回 inode 数据
                inode_ref.write_inode_data(&inode_data)?;

                // 如果有 xattr block，也写回
                if let Some(ref block_data) = xattr_block_data {
                    let file_acl_lo = inode_ref.with_inode(|inode| u32::from_le(inode.file_acl_lo))?;
                    if file_acl_lo != 0 {
                        drop(inode_ref);
                        self.bdev.write_block(file_acl_lo as u64, block_data)?;
                    }
                }

                Ok(())
            }
            Err(e) if e.kind() == ErrorKind::NoSpace && xattr_block_data.is_none() => {
                // 需要分配新的 xattr block
                drop(inode_ref);

                // 分配新块
                let mut allocator = BlockAllocator::new();
                let new_block_addr = allocator.alloc_block(&mut self.bdev, &mut self.sb, 0)?;

                if new_block_addr == 0 {
                    return Err(Error::new(ErrorKind::NoSpace, "Failed to allocate xattr block"));
                }

                // 初始化新的 xattr block
                let mut new_block_data = alloc::vec![0u8; block_size as usize];

                // 设置 xattr block header
                use crate::consts::EXT4_XATTR_MAGIC;
                let magic_bytes = EXT4_XATTR_MAGIC.to_le_bytes();
                new_block_data[0..4].copy_from_slice(&magic_bytes);

                // 设置引用计数为 1
                let refcount = 1u32.to_le_bytes();
                new_block_data[4..8].copy_from_slice(&refcount);

                // 设置块数为 1（单块 xattr）
                let blocks = 1u32.to_le_bytes();
                new_block_data[8..12].copy_from_slice(&blocks);

                // 重新获取 inode_ref 并更新 file_acl
                let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

                inode_ref.with_inode_mut(|inode| {
                    inode.file_acl_lo = (new_block_addr as u32).to_le();
                    // file_acl_high 通常为 0（32 位块地址足够）
                })?;

                inode_ref.mark_dirty()?;

                // 重新获取数据
                let mut inode_data = inode_ref.get_inode_data_mut()?;
                let inode = inode_ref.get_inode()?;

                let sb_ptr = inode_ref.superblock_mut() as *mut crate::superblock::Superblock;
                let sb_ref = unsafe { &*sb_ptr };

                // 再次尝试设置 xattr（现在有 xattr block 了）
                xattr::set(
                    sb_ref,
                    &inode,
                    &mut inode_data,
                    Some(&mut new_block_data),
                    name,
                    value,
                )?;

                // 写回 inode 数据
                inode_ref.write_inode_data(&inode_data)?;

                // 写回 xattr block
                drop(inode_ref);
                self.bdev.write_block(new_block_addr, &new_block_data)?;

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// 删除扩展属性
    ///
    /// # 参数
    ///
    /// * `path` - 文件或目录路径（绝对路径）
    /// * `name` - 属性名（含前缀，如 "user.comment"）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// fs.removexattr("/etc/passwd", "user.comment")?;
    /// ```
    pub fn removexattr(&mut self, path: &str, name: &str) -> Result<()> {
        use crate::xattr;

        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, path)?;

        // 获取 inode_ref 并修改 xattr
        let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

        // 获取 inode 和数据（可变）
        let mut inode_data = inode_ref.get_inode_data_mut()?;
        let mut xattr_block_data = inode_ref.read_xattr_block_mut()?;
        let inode = inode_ref.get_inode()?;

        // 使用 unsafe 来获取不可变 superblock 引用
        let sb_ptr = inode_ref.superblock_mut() as *mut crate::superblock::Superblock;
        let sb_ref = unsafe { &*sb_ptr };

        // 调用底层 xattr API
        xattr::remove(
            sb_ref,
            &inode,
            &mut inode_data,
            xattr_block_data.as_deref_mut(),
            name,
        )?;

        // 写回修改的 inode 块数据
        inode_ref.write_inode_data(&inode_data)?;

        // 写回 xattr block（如果存在）
        if let Some(ref block_data) = xattr_block_data {
            let file_acl_lo = inode_ref.with_inode(|inode| u32::from_le(inode.file_acl_lo))?;
            if file_acl_lo != 0 {
                drop(inode_ref);
                self.bdev.write_block(file_acl_lo as u64, block_data)?;
                return Ok(());
            }
        }

        Ok(())
    }

    // ========== Inode 分配和释放 API ==========

    /// 分配一个新的 inode
    ///
    /// 对应 lwext4 的 `ext4_fs_alloc_inode()`
    ///
    /// # 参数
    ///
    /// * `is_dir` - 是否是目录
    ///
    /// # 返回
    ///
    /// 成功返回新分配的 inode 编号
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let inode_num = fs.alloc_inode(false)?; // 分配普通文件的 inode
    /// let mut inode_ref = fs.get_inode_ref(inode_num)?;
    /// // 初始化 inode 并使用
    /// ```
    pub fn alloc_inode(&mut self, is_dir: bool) -> Result<u32> {
        use crate::ialloc::InodeAllocator;

        let mut allocator = InodeAllocator::new();
        let inode_num = allocator.alloc_inode(&mut self.bdev, &mut self.sb, is_dir)?;

        Ok(inode_num)
    }

    /// 释放一个 inode
    ///
    /// 对应 lwext4 的 `ext4_fs_free_inode()`
    ///
    /// # 参数
    ///
    /// * `inode_num` - 要释放的 inode 编号
    /// * `is_dir` - 是否是目录
    ///
    /// # 返回
    ///
    /// 成功返回 Ok(())
    ///
    /// # 注意
    ///
    /// - 调用前应确保 inode 的数据块已全部释放
    /// - 调用前应确保 inode 的引用计数为 0
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 先释放 inode 的所有数据块
    /// let mut inode_ref = fs.get_inode_ref(inode_num)?;
    /// let is_dir = inode_ref.is_dir()?;
    /// inode_ref.truncate(&mut fs.superblock_mut(), 0)?;
    /// drop(inode_ref);
    ///
    /// // 然后释放 inode 本身
    /// fs.free_inode(inode_num, is_dir)?;
    /// ```
    pub fn free_inode(&mut self, inode_num: u32, is_dir: bool) -> Result<()> {
        use crate::ialloc::free_inode;

        free_inode(&mut self.bdev, &mut self.sb, inode_num, is_dir)?;

        Ok(())
    }

    /// 分配一个数据块（用于文件写入）
    ///
    /// # 参数
    ///
    /// * `goal` - 建议的块组 ID（用于局部性优化）
    ///
    /// # 返回
    ///
    /// 成功返回新分配的物理块号
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let inode_ref = fs.get_inode_ref(inode_num)?;
    /// let goal = inode_ref.get_alloc_goal();
    /// let block_addr = fs.alloc_block(goal as u64)?;
    /// // 使用 block_addr 写入数据
    /// ```
    pub fn alloc_block(&mut self, goal: u64) -> Result<u64> {
        use crate::balloc::BlockAllocator;

        let mut allocator = BlockAllocator::new();
        let block_addr = allocator.alloc_block(&mut self.bdev, &mut self.sb, goal)?;

        Ok(block_addr)
    }

    /// 释放一个数据块
    ///
    /// # 参数
    ///
    /// * `block_addr` - 要释放的物理块号
    ///
    /// # 返回
    ///
    /// 成功返回 Ok(())
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// fs.free_block(block_addr)?;
    /// ```
    pub fn free_block(&mut self, block_addr: u64) -> Result<()> {
        use crate::balloc::free_block;

        free_block(&mut self.bdev, &mut self.sb, block_addr)?;

        Ok(())
    }

    /// 截断文件到指定大小
    ///
    /// # 参数
    ///
    /// * `inode_num` - inode 编号
    /// * `new_size` - 新的文件大小
    ///
    /// # 注意
    ///
    /// 这是一个简化的实现，仅更新 inode 的大小字段。
    /// 实际的块释放需要单独调用 extent::remove_space。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// fs.truncate_file(inode_num, 1024)?; // 截断到 1KB
    /// ```
    pub fn truncate_file(&mut self, inode_num: u32, new_size: u64) -> Result<()> {
        use crate::extent::remove_space;

        // 第一步：获取旧大小并更新 inode 大小
        let old_size = {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;
            let old_size = inode_ref.size()?;

            if old_size == new_size {
                return Ok(());
            }

            if old_size < new_size {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Cannot enlarge file with truncate (use write operations instead)",
                ));
            }

            // 更新 inode 大小
            inode_ref.set_size(new_size)?;
            inode_ref.mark_dirty()?;

            old_size
            // inode_ref 在这里 drop，自动写回
        };

        // 第二步：释放不再需要的数据块
        // 计算需要释放的逻辑块范围
        let block_size = self.sb.block_size() as u64;
        let first_block_to_remove = ((new_size + block_size - 1) / block_size) as u32;
        let last_block_to_remove = ((old_size + block_size - 1) / block_size) as u32;

        if first_block_to_remove < last_block_to_remove {
            // 调用 remove_space 释放块
            // remove_space 的参数是 [from, to)，即删除 [from, to-1] 的块
            {
                let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

                // 注意：remove_space API 需要重构，因为它同时要求 inode_ref 和 sb
                // 但 inode_ref 已经持有 &mut sb。这里使用 unsafe 来绕过借用检查
                //
                // 安全性说明：
                // - inode_ref.sb 指向 self.sb
                // - 我们只在 inode_ref 的生命周期内使用这个引用
                // - remove_space 不会同时修改同一个字段
                let sb_ptr = inode_ref.superblock_mut() as *mut crate::superblock::Superblock;
                let sb_ref = unsafe { &mut *sb_ptr };

                remove_space(&mut inode_ref, sb_ref, first_block_to_remove, last_block_to_remove)?;
            }
            // inode_ref 在这里 drop，自动写回修改
        }

        Ok(())
    }

    // ========== 内部辅助方法 ==========

    /// 获取或分配文件块（供 File::write 使用）
    ///
    /// # 参数
    ///
    /// * `inode_num` - Inode 编号
    /// * `logical_block` - 逻辑块号
    ///
    /// # 返回
    ///
    /// 物理块号
    ///
    /// # 注意
    ///
    /// 由于借用检查器限制，目前仅支持查找已分配的块，不支持自动分配
    pub(crate) fn get_file_block(&mut self, inode_num: u32, logical_block: u32) -> Result<u64> {
        use crate::extent::ExtentTree;

        let inode = {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;
            inode_ref.get_inode()?
            // inode_ref dropped here
        };

        let mut extent_tree = ExtentTree::new(&mut self.bdev, self.sb.block_size());
        let physical_block = extent_tree.map_block(&inode, logical_block)?
            .ok_or_else(|| Error::new(ErrorKind::Unsupported, "Block not allocated - automatic allocation requires API redesign"))?;

        Ok(physical_block)
    }

    /// 添加目录项（内部辅助方法）
    ///
    /// # 注意
    ///
    /// 由于借用检查器限制，这个方法暂时标记为 TODO
    fn add_dir_entry(&mut self, dir_inode: u32, name: &str, child_inode: u32, file_type: u8) -> Result<()> {
        use crate::dir::write;

        let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, dir_inode)?;

        // 安全性说明：
        // - dir::write::add_entry 需要 &mut Superblock 但 inode_ref 已持有 &mut sb
        // - add_entry 只读取 superblock 的一些字段（block_size 等），不会修改
        // - 使用 unsafe 指针绕过借用检查器，确保不会产生数据竞争
        let sb_ptr = inode_ref.superblock_mut() as *mut Superblock;
        let sb_ref = unsafe { &mut *sb_ptr };

        write::add_entry(&mut inode_ref, sb_ref, name, child_inode, file_type)?;

        Ok(())
    }

    /// 删除目录项（内部辅助方法）
    ///
    /// # 注意
    ///
    /// 使用 unsafe 指针绕过借用检查器的限制
    fn remove_dir_entry(&mut self, dir_inode: u32, name: &str) -> Result<()> {
        use crate::dir::write;

        let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, dir_inode)?;

        // dir::write::remove_entry 只需要 inode_ref，不需要单独的 superblock
        write::remove_entry(&mut inode_ref, name)?;

        Ok(())
    }

    // ========== 高级文件操作 API ==========

    /// 创建新文件
    ///
    /// # 参数
    ///
    /// * `parent_path` - 父目录路径
    /// * `name` - 文件名
    /// * `mode` - 文件权限（Unix 权限位，如 0o644）
    ///
    /// # 返回
    ///
    /// 新文件的 inode 编号
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let inode_num = fs.create_file("/tmp", "test.txt", 0o644)?;
    /// ```
    pub fn create_file(&mut self, parent_path: &str, name: &str, mode: u16) -> Result<u32> {
        use crate::{consts::*, dir::write::{self, EXT4_DE_REG_FILE}, extent::tree_init};

        // 1. 分配新 inode
        let inode_num = self.alloc_inode(false)?;

        // 2. 初始化 inode
        {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

            // 设置文件模式（类型 + 权限）
            let file_mode = EXT4_INODE_MODE_FILE as u32 | mode as u32;
            inode_ref.with_inode_mut(|inode| {
                inode.mode = (file_mode as u16).to_le();
            })?;

            // 设置初始大小为 0
            inode_ref.set_size(0)?;

            // 设置链接计数为 1
            inode_ref.with_inode_mut(|inode| {
                inode.links_count = 1u16.to_le();
            })?;

            // 设置时间戳（使用简单的时间戳，实际应该从系统获取）
            let now = 0u32; // TODO: 获取当前时间
            inode_ref.with_inode_mut(|inode| {
                inode.atime = now.to_le();
                inode.ctime = now.to_le();
                inode.mtime = now.to_le();
            })?;

            // 设置 EXTENTS 标志
            inode_ref.with_inode_mut(|inode| {
                let flags = u32::from_le(inode.flags);
                inode.flags = (flags | EXT4_INODE_FLAG_EXTENTS).to_le();
            })?;

            // 初始化 extent 树
            tree_init(&mut inode_ref)?;

            inode_ref.mark_dirty()?;
            // inode_ref drop 时自动写回
        }

        // 3. 查找父目录并添加条目
        let parent_inode = lookup_path(&mut self.bdev, &mut self.sb, parent_path)?;

        // 4. 添加到父目录（通过辅助方法避免借用冲突）
        self.add_dir_entry(parent_inode, name, inode_num, EXT4_DE_REG_FILE)?;

        Ok(inode_num)
    }

    /// 创建新目录
    ///
    /// # 参数
    ///
    /// * `parent_path` - 父目录路径
    /// * `name` - 目录名
    /// * `mode` - 目录权限（Unix 权限位，如 0o755）
    ///
    /// # 返回
    ///
    /// 新目录的 inode 编号
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let inode_num = fs.create_dir("/tmp", "mydir", 0o755)?;
    /// ```
    pub fn create_dir(&mut self, parent_path: &str, name: &str, mode: u16) -> Result<u32> {
        use crate::{consts::*, dir::write::{self, EXT4_DE_DIR}, extent::tree_init};

        // 1. 分配新 inode
        let inode_num = self.alloc_inode(true)?;

        // 2. 查找父目录 inode
        let parent_inode = lookup_path(&mut self.bdev, &mut self.sb, parent_path)?;

        // 3. 初始化目录 inode
        {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

            // 设置目录模式（类型 + 权限）
            let dir_mode = EXT4_INODE_MODE_DIRECTORY as u32 | mode as u32;
            inode_ref.with_inode_mut(|inode| {
                inode.mode = (dir_mode as u16).to_le();
            })?;

            // 设置初始大小为 0（目录项会自动增长）
            inode_ref.set_size(0)?;

            // 设置链接计数为 2（自己 + "." 条目）
            inode_ref.with_inode_mut(|inode| {
                inode.links_count = 2u16.to_le();
            })?;

            // 设置时间戳
            let now = 0u32; // TODO: 获取当前时间
            inode_ref.with_inode_mut(|inode| {
                inode.atime = now.to_le();
                inode.ctime = now.to_le();
                inode.mtime = now.to_le();
            })?;

            // 设置 EXTENTS 标志
            inode_ref.with_inode_mut(|inode| {
                let flags = u32::from_le(inode.flags);
                inode.flags = (flags | EXT4_INODE_FLAG_EXTENTS).to_le();
            })?;

            // 初始化 extent 树
            tree_init(&mut inode_ref)?;

            inode_ref.mark_dirty()?;
            // inode_ref drop 时自动写回
        }

        // 4. 添加 "." 和 ".." 条目到新目录
        self.add_dir_entry(inode_num, ".", inode_num, EXT4_DE_DIR)?;
        self.add_dir_entry(inode_num, "..", parent_inode, EXT4_DE_DIR)?;

        // 5. 添加到父目录
        self.add_dir_entry(parent_inode, name, inode_num, EXT4_DE_DIR)?;

        // 6. 增加父目录的链接计数（因为新目录的 ".." 指向父目录）
        {
            let mut parent_inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, parent_inode)?;

            parent_inode_ref.with_inode_mut(|inode| {
                let links = u16::from_le(inode.links_count);
                inode.links_count = (links + 1).to_le();
            })?;

            parent_inode_ref.mark_dirty()?;
        }

        Ok(inode_num)
    }

    /// 创建硬链接
    ///
    /// 为现有文件创建一个新的硬链接（多个目录项指向同一个 inode）。
    ///
    /// # 参数
    ///
    /// * `src_path` - 源文件的完整路径
    /// * `dst_dir` - 目标目录路径
    /// * `dst_name` - 新链接的名称
    ///
    /// # 返回
    ///
    /// 成功返回 Ok(())
    ///
    /// # 错误
    ///
    /// - `ErrorKind::NotFound` - 源文件不存在
    /// - `ErrorKind::InvalidInput` - 源不是普通文件（不能对目录创建硬链接）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 为 /tmp/original.txt 创建硬链接 /tmp/link.txt
    /// fs.flink("/tmp/original.txt", "/tmp", "link.txt")?;
    /// ```
    ///
    /// # 说明
    ///
    /// 硬链接与原文件共享相同的 inode 和数据块，修改任一文件都会影响另一个。
    /// 只有当所有硬链接都被删除后，文件数据才会被真正释放。
    pub fn flink(&mut self, src_path: &str, dst_dir: &str, dst_name: &str) -> Result<()> {
        use crate::dir::write::EXT4_DE_REG_FILE;

        // 1. 查找源文件 inode
        let src_inode = lookup_path(&mut self.bdev, &mut self.sb, src_path)?;

        // 2. 验证源是普通文件（不能对目录创建硬链接）
        {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, src_inode)?;
            if !inode_ref.is_file()? {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Cannot create hard link to non-regular file",
                ));
            }
        }

        // 3. 查找目标目录 inode
        let dst_dir_inode = lookup_path(&mut self.bdev, &mut self.sb, dst_dir)?;

        // 4. 增加源文件的链接计数
        {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, src_inode)?;
            inode_ref.with_inode_mut(|inode| {
                let links = u16::from_le(inode.links_count);
                inode.links_count = (links + 1).to_le();
            })?;
            inode_ref.mark_dirty()?;
        }

        // 5. 在目标目录添加新的目录项（指向相同的 inode）
        self.add_dir_entry(dst_dir_inode, dst_name, src_inode, EXT4_DE_REG_FILE)?;

        Ok(())
    }

    /// 创建符号链接
    ///
    /// 创建一个指向目标路径的符号链接。
    ///
    /// # 参数
    ///
    /// * `target` - 符号链接指向的目标路径（可以是相对或绝对路径）
    /// * `link_dir` - 符号链接所在目录的路径
    /// * `link_name` - 符号链接的名称
    ///
    /// # 返回
    ///
    /// 成功返回新创建的符号链接的 inode 编号
    ///
    /// # 说明
    ///
    /// - 快速符号链接（< 60 字节）：目标路径直接存储在 inode.block 中
    /// - 慢速符号链接（>= 60 字节）：需要分配数据块存储目标路径
    /// - 符号链接的权限通常为 0o777
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 创建符号链接 /tmp/link -> /etc/passwd
    /// fs.fsymlink("/etc/passwd", "/tmp", "link")?;
    /// ```
    pub fn fsymlink(&mut self, target: &str, link_dir: &str, link_name: &str) -> Result<u32> {
        use crate::{consts::*, dir::write::EXT4_DE_SYMLINK, extent::tree_init};

        // 1. 分配新 inode
        let inode_num = self.alloc_inode(false)?;

        // 提取 block_size（避免借用冲突）
        let block_size = self.sb.block_size();

        // 2. 初始化符号链接 inode
        {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

            // 设置符号链接类型和权限
            let symlink_mode = EXT4_INODE_MODE_SOFTLINK | 0o777;
            inode_ref.with_inode_mut(|inode| {
                inode.mode = symlink_mode.to_le();
                inode.links_count = 1u16.to_le();
            })?;

            // 设置大小为目标路径长度
            inode_ref.set_size(target.len() as u64)?;

            // 设置时间戳
            let now = 0u32; // TODO: 获取当前时间
            inode_ref.with_inode_mut(|inode| {
                inode.atime = now.to_le();
                inode.ctime = now.to_le();
                inode.mtime = now.to_le();
            })?;

            // 存储目标路径
            let target_bytes = target.as_bytes();
            if target.len() < 60 {
                // 快速符号链接：存储在 inode.block 中
                inode_ref.with_inode_mut(|inode| {
                    let block_slice = unsafe {
                        core::slice::from_raw_parts_mut(
                            inode.blocks.as_mut_ptr() as *mut u8,
                            60,
                        )
                    };
                    block_slice[..target_bytes.len()].copy_from_slice(target_bytes);
                })?;
            } else {
                // 慢速符号链接：需要分配块存储
                // 设置 EXTENTS 标志
                inode_ref.with_inode_mut(|inode| {
                    let flags = u32::from_le(inode.flags);
                    inode.flags = (flags | EXT4_INODE_FLAG_EXTENTS).to_le();
                })?;

                // 初始化 extent 树
                tree_init(&mut inode_ref)?;

                // 分配块并写入目标路径
                let block_addr = inode_ref.get_inode_dblk_idx(0, true)?;
                if block_addr == 0 {
                    return Err(Error::new(ErrorKind::NoSpace, "Failed to allocate block for symlink"));
                }

                inode_ref.mark_dirty()?;

                // drop inode_ref，然后写块
                drop(inode_ref);

                // 写入目标路径到块
                let mut block_buf = alloc::vec![0u8; block_size as usize];
                block_buf[..target_bytes.len()].copy_from_slice(target_bytes);
                self.bdev.write_block(block_addr, &block_buf)?;

                // 重新获取 inode_ref 以便继续（实际上已经不需要了）
                // return 会退出，所以这里直接返回
                let dir_inode = lookup_path(&mut self.bdev, &mut self.sb, link_dir)?;
                self.add_dir_entry(dir_inode, link_name, inode_num, EXT4_DE_SYMLINK)?;
                return Ok(inode_num);
            }

            inode_ref.mark_dirty()?;
        }

        // 3. 在目录中添加符号链接条目
        let dir_inode = lookup_path(&mut self.bdev, &mut self.sb, link_dir)?;
        self.add_dir_entry(dir_inode, link_name, inode_num, EXT4_DE_SYMLINK)?;

        Ok(inode_num)
    }

    /// 读取符号链接的目标路径
    ///
    /// # 参数
    ///
    /// * `link_path` - 符号链接的完整路径
    ///
    /// # 返回
    ///
    /// 成功返回符号链接指向的目标路径
    ///
    /// # 错误
    ///
    /// - `ErrorKind::NotFound` - 路径不存在
    /// - `ErrorKind::InvalidInput` - 路径不是符号链接
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let target = fs.readlink("/tmp/link")?;
    /// println!("Link points to: {}", target);
    /// ```
    pub fn readlink(&mut self, link_path: &str) -> Result<alloc::string::String> {
        use crate::consts::*;

        // 1. 查找符号链接 inode
        let inode_num = lookup_path(&mut self.bdev, &mut self.sb, link_path)?;

        // 提取 block_size（避免借用冲突）
        let block_size = self.sb.block_size();

        let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, inode_num)?;

        // 2. 验证是符号链接
        let mode = inode_ref.with_inode(|inode| u16::from_le(inode.mode))?;
        if (mode & EXT4_INODE_MODE_TYPE_MASK) != EXT4_INODE_MODE_SOFTLINK {
            return Err(Error::new(ErrorKind::InvalidInput, "Not a symlink"));
        }

        let size = inode_ref.size()? as usize;
        if size == 0 {
            return Ok(alloc::string::String::new());
        }

        // 3. 读取目标路径
        let target_bytes = if size < 60 {
            // 快速符号链接：从 inode.blocks 读取
            inode_ref.with_inode(|inode| {
                let block_slice = unsafe {
                    core::slice::from_raw_parts(inode.blocks.as_ptr() as *const u8, size)
                };
                block_slice.to_vec()
            })?
        } else {
            // 慢速符号链接：从数据块读取
            let block_addr = inode_ref.get_inode_dblk_idx(0, false)?;
            if block_addr == 0 {
                return Err(Error::new(ErrorKind::NotFound, "Symlink data block not found"));
            }

            // drop inode_ref，然后读块
            drop(inode_ref);

            let mut block_buf = alloc::vec![0u8; block_size as usize];
            self.bdev.read_block(block_addr, &mut block_buf)?;
            block_buf[..size].to_vec()
        };

        alloc::string::String::from_utf8(target_bytes)
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid UTF-8 in symlink target"))
    }

    /// 删除文件
    ///
    /// # 参数
    ///
    /// * `parent_path` - 父目录路径
    /// * `name` - 文件名
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// fs.remove_file("/tmp", "test.txt")?;
    /// ```
    pub fn remove_file(&mut self, parent_path: &str, name: &str) -> Result<()> {
        use crate::consts::{EXT4_INODE_MODE_TYPE_MASK, EXT4_INODE_MODE_SOFTLINK};

        // 1. 查找父目录
        let parent_inode = lookup_path(&mut self.bdev, &mut self.sb, parent_path)?;

        // 2. 构造完整路径查找文件 inode
        let full_path = if parent_path.ends_with('/') {
            alloc::format!("{}{}", parent_path, name)
        } else {
            alloc::format!("{}/{}", parent_path, name)
        };
        let file_inode = lookup_path(&mut self.bdev, &mut self.sb, &full_path)?;

        // 3. 检查是否是普通文件或符号链接（不能删除目录）
        {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, file_inode)?;
            let is_dir = inode_ref.is_dir()?;
            if is_dir {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Cannot remove directory with remove_file (use remove_dir)",
                ));
            }
            // 允许删除普通文件和符号链接
        }

        // 4. 从父目录删除条目
        self.remove_dir_entry(parent_inode, name)?;

        // 5. 减少链接计数
        let (should_free, is_fast_symlink) = {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, file_inode)?;
            inode_ref.with_inode_mut(|inode| {
                let links = u16::from_le(inode.links_count);
                inode.links_count = (links.saturating_sub(1)).to_le();
            })?;
            inode_ref.mark_dirty()?;

            let links = inode_ref.with_inode(|inode| {
                u16::from_le(inode.links_count)
            })?;

            // 检查是否是快速符号链接（< 60 字节，无数据块）
            let mode = inode_ref.with_inode(|inode| u16::from_le(inode.mode))?;
            let size = inode_ref.size()?;
            let is_symlink = (mode & EXT4_INODE_MODE_TYPE_MASK) == EXT4_INODE_MODE_SOFTLINK;
            let is_fast = is_symlink && size < 60;

            (links == 0, is_fast)
        };

        // 6. 如果链接计数为 0，释放 inode 和数据块
        if should_free {
            // 快速符号链接没有数据块，跳过截断
            if !is_fast_symlink {
                // 先截断文件以释放所有数据块
                self.truncate_file(file_inode, 0)?;
            }

            // 释放 inode
            self.free_inode(file_inode, false)?;
        }

        Ok(())
    }

    /// 删除目录
    ///
    /// # 参数
    ///
    /// * `parent_path` - 父目录路径
    /// * `name` - 目录名
    ///
    /// # 注意
    ///
    /// 只能删除空目录（只包含 "." 和 ".." 条目）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// fs.remove_dir("/tmp", "mydir")?;
    /// ```
    pub fn remove_dir(&mut self, parent_path: &str, name: &str) -> Result<()> {
        use crate::dir::iterator::DirIterator;

        // 1. 查找父目录
        let parent_inode = lookup_path(&mut self.bdev, &mut self.sb, parent_path)?;

        // 2. 构造完整路径查找目录 inode
        let full_path = if parent_path.ends_with('/') {
            alloc::format!("{}{}", parent_path, name)
        } else {
            alloc::format!("{}/{}", parent_path, name)
        };
        let dir_inode = lookup_path(&mut self.bdev, &mut self.sb, &full_path)?;

        // 3. 检查是否是目录
        {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, dir_inode)?;
            if !inode_ref.is_dir()? {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Not a directory",
                ));
            }
        }

        // 4. 检查目录是否为空（只有 "." 和 ".." 条目）
        {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, dir_inode)?;
            let mut iter = DirIterator::new(&mut inode_ref, 0)?;
            let mut entry_count = 0;

            while let Some(entry) = iter.next(&mut inode_ref)? {
                let name = &entry.name;
                if name != "." && name != ".." {
                    return Err(Error::new(
                        ErrorKind::NotEmpty,
                        "Directory not empty",
                    ));
                }
                entry_count += 1;
            }

            // 目录应该至少有 "." 和 ".."
            if entry_count < 2 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Invalid directory structure",
                ));
            }
        }

        // 5. 从父目录删除条目并更新父目录链接计数
        self.remove_dir_entry(parent_inode, name)?;

        // 减少父目录的链接计数（因为删除了指向父目录的 ".." 条目）
        {
            let mut parent_inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, parent_inode)?;

            // 减少父目录的链接计数（因为删除了指向父目录的 ".." 条目）
            parent_inode_ref.with_inode_mut(|inode| {
                let links = u16::from_le(inode.links_count);
                inode.links_count = (links.saturating_sub(1)).to_le();
            })?;

            parent_inode_ref.mark_dirty()?;
        }

        // 6. 释放目录 inode 和数据块
        // 先截断以释放数据块
        self.truncate_file(dir_inode, 0)?;

        // 释放 inode
        self.free_inode(dir_inode, true)?;

        Ok(())
    }

    /// 重命名文件或目录
    ///
    /// # 参数
    ///
    /// * `old_parent_path` - 旧的父目录路径
    /// * `old_name` - 旧名称
    /// * `new_parent_path` - 新的父目录路径
    /// * `new_name` - 新名称
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// fs.rename("/tmp", "old.txt", "/tmp", "new.txt")?;
    /// fs.rename("/tmp", "file.txt", "/home", "file.txt")?; // 移动文件
    /// ```
    pub fn rename(
        &mut self,
        old_parent_path: &str,
        old_name: &str,
        new_parent_path: &str,
        new_name: &str,
    ) -> Result<()> {
        use crate::dir::write::{EXT4_DE_DIR, EXT4_DE_REG_FILE};

        // 1. 查找旧父目录
        let old_parent_inode = lookup_path(&mut self.bdev, &mut self.sb, old_parent_path)?;

        // 2. 查找新父目录
        let new_parent_inode = lookup_path(&mut self.bdev, &mut self.sb, new_parent_path)?;

        // 3. 构造完整路径查找文件/目录 inode
        let old_full_path = if old_parent_path.ends_with('/') {
            alloc::format!("{}{}", old_parent_path, old_name)
        } else {
            alloc::format!("{}/{}", old_parent_path, old_name)
        };
        let target_inode = lookup_path(&mut self.bdev, &mut self.sb, &old_full_path)?;

        // 4. 获取文件类型
        let (is_dir, file_type) = {
            let mut inode_ref = InodeRef::get(&mut self.bdev, &mut self.sb, target_inode)?;
            let is_dir = inode_ref.is_dir()?;
            let file_type = if is_dir {
                EXT4_DE_DIR
            } else {
                EXT4_DE_REG_FILE
            };
            (is_dir, file_type)
        };

        // 5. 在新父目录添加条目
        self.add_dir_entry(new_parent_inode, new_name, target_inode, file_type)?;

        // 如果是目录且移动到新父目录，增加新父目录的链接计数
        if is_dir && old_parent_inode != new_parent_inode {
            let mut new_parent_inode_ref =
                InodeRef::get(&mut self.bdev, &mut self.sb, new_parent_inode)?;

            new_parent_inode_ref.with_inode_mut(|inode| {
                let links = u16::from_le(inode.links_count);
                inode.links_count = (links + 1).to_le();
            })?;
            new_parent_inode_ref.mark_dirty()?;
        }

        // 6. 从旧父目录删除条目
        self.remove_dir_entry(old_parent_inode, old_name)?;

        // 如果是目录且移动到新父目录，减少旧父目录的链接计数
        if is_dir && old_parent_inode != new_parent_inode {
            let mut old_parent_inode_ref =
                InodeRef::get(&mut self.bdev, &mut self.sb, old_parent_inode)?;

            old_parent_inode_ref.with_inode_mut(|inode| {
                let links = u16::from_le(inode.links_count);
                inode.links_count = (links.saturating_sub(1)).to_le();
            })?;
            old_parent_inode_ref.mark_dirty()?;
        }

        // 7. 如果是目录且移动到新父目录，更新 ".." 条目
        if is_dir && old_parent_inode != new_parent_inode {
            // 删除旧的 ".." 条目
            self.remove_dir_entry(target_inode, "..")?;

            // 添加新的 ".." 条目
            self.add_dir_entry(target_inode, "..", new_parent_inode, EXT4_DE_DIR)?;
        }

        Ok(())
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
