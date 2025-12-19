//! 文件句柄

use crate::{
    block::{BlockDev, BlockDevice},
    error::{Error, ErrorKind, Result},
    extent::ExtentTree,
    inode::Inode,
    superblock::Superblock,
};

use super::filesystem::Ext4FileSystem;

/// 文件句柄
///
/// 表示一个打开的文件，支持读取和定位操作
pub struct File<D: BlockDevice> {
    inode: Inode,
    inode_num: u32,
    offset: u64,
    block_size: u32,
    _phantom: core::marker::PhantomData<D>,
}

impl<D: BlockDevice> File<D> {
    /// 创建新的文件句柄（内部使用）
    pub(super) fn new(
        _bdev: &mut BlockDev<D>,
        sb: &Superblock,
        inode: Inode,
        inode_num: u32,
    ) -> Result<Self> {
        Ok(Self {
            inode,
            inode_num,
            offset: 0,
            block_size: sb.block_size(),
            _phantom: core::marker::PhantomData,
        })
    }

    /// 读取文件内容
    ///
    /// 从当前位置读取数据到缓冲区，并更新文件位置
    ///
    /// # 参数
    ///
    /// * `fs` - 文件系统引用
    /// * `buf` - 目标缓冲区
    ///
    /// # 返回
    ///
    /// 实际读取的字节数（可能小于缓冲区大小）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut file = fs.open("/etc/passwd")?;
    /// let mut buf = vec![0u8; 1024];
    /// let n = file.read(&mut fs, &mut buf)?;
    /// println!("Read {} bytes", n);
    /// ```
    pub fn read(&mut self, fs: &mut Ext4FileSystem<D>, buf: &mut [u8]) -> Result<usize> {
        if self.offset >= self.inode.file_size() {
            return Ok(0); // EOF
        }

        let mut extent_tree = ExtentTree::new(&mut fs.bdev, self.block_size);
        let n = extent_tree.read_file(&self.inode, self.offset, buf)?;

        self.offset += n as u64;

        Ok(n)
    }

    /// 读取整个文件内容
    ///
    /// # 参数
    ///
    /// * `fs` - 文件系统引用
    ///
    /// # 返回
    ///
    /// 文件内容（Vec<u8>）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut file = fs.open("/etc/passwd")?;
    /// let content = file.read_to_end(&mut fs)?;
    /// let text = String::from_utf8_lossy(&content);
    /// ```
    pub fn read_to_end(&mut self, fs: &mut Ext4FileSystem<D>) -> Result<alloc::vec::Vec<u8>> {
        let file_size = self.inode.file_size();

        if file_size > usize::MAX as u64 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "File too large to read into memory",
            ));
        }

        let mut buf = alloc::vec![0u8; file_size as usize];
        let mut total_read = 0;

        while total_read < buf.len() {
            let n = self.read(fs, &mut buf[total_read..])?;
            if n == 0 {
                break; // EOF
            }
            total_read += n;
        }

        buf.truncate(total_read);
        Ok(buf)
    }

    /// 移动文件指针
    ///
    /// # 参数
    ///
    /// * `pos` - 新的位置（字节偏移）
    ///
    /// # 返回
    ///
    /// 新的位置
    ///
    /// # 错误
    ///
    /// 如果位置超出文件大小，返回错误
    pub fn seek(&mut self, pos: u64) -> Result<u64> {
        if pos > self.inode.file_size() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Seek position beyond file size",
            ));
        }

        self.offset = pos;
        Ok(self.offset)
    }

    /// 获取当前文件指针位置
    pub fn position(&self) -> u64 {
        self.offset
    }

    /// 获取文件大小
    pub fn size(&self) -> u64 {
        self.inode.file_size()
    }

    /// 获取 inode 编号
    pub fn inode_num(&self) -> u32 {
        self.inode_num
    }

    /// 重置文件指针到起始位置
    pub fn rewind(&mut self) {
        self.offset = 0;
    }

    // ========== 写操作 ==========

    /// 写入数据到文件
    ///
    /// 从当前位置写入数据，并更新文件位置
    ///
    /// # 参数
    ///
    /// * `fs` - 文件系统引用
    /// * `buf` - 要写入的数据
    ///
    /// # 返回
    ///
    /// 实际写入的字节数
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut file = fs.open("/tmp/test.txt")?;
    /// let n = file.write(&mut fs, b"Hello, World!")?;
    /// println!("Wrote {} bytes", n);
    /// ```
    pub fn write(&mut self, fs: &mut Ext4FileSystem<D>, buf: &[u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // 计算当前 offset 对应的逻辑块号和块内偏移
        let block_size = self.block_size as u64;
        let logical_block = (self.offset / block_size) as u32;
        let offset_in_block = (self.offset % block_size) as usize;

        // 计算本次写入的数据量（不超过当前块的剩余空间）
        let remaining_in_block = block_size as usize - offset_in_block;
        let write_len = buf.len().min(remaining_in_block);

        // 使用 InodeRef 获取或分配物理块
        let physical_block = {
            let mut inode_ref = fs.get_inode_ref(self.inode_num)?;
            let phys = inode_ref.get_inode_dblk_idx(logical_block, true)?; // create=true 自动分配

            // 更新本地 inode 副本（可能分配了新块，inode 被修改）
            self.inode = inode_ref.get_inode()?;
            phys
        }; // inode_ref 在此 drop，自动写回修改

        if physical_block == 0 {
            return Err(Error::new(
                ErrorKind::NoSpace,
                "Failed to allocate block for write",
            ));
        }

        // 读取整个块（如果块是新分配的，会读到全零）
        let mut block_buf = alloc::vec![0u8; block_size as usize];
        fs.bdev.read_block(physical_block, &mut block_buf)?;

        // 在块内写入数据
        block_buf[offset_in_block..offset_in_block + write_len]
            .copy_from_slice(&buf[..write_len]);

        // 写回块
        fs.bdev.write_block(physical_block, &block_buf)?;

        // 更新文件位置
        self.offset += write_len as u64;

        // 如果写入超过了文件末尾，更新文件大小
        if self.offset > self.inode.file_size() {
            let mut inode_ref = fs.get_inode_ref(self.inode_num)?;
            inode_ref.set_size(self.offset)?;
            inode_ref.mark_dirty()?;

            // 更新本地 inode 副本
            self.inode = inode_ref.get_inode()?;
        }

        Ok(write_len)
    }

    /// 截断文件到指定大小
    ///
    /// # 参数
    ///
    /// * `fs` - 文件系统引用
    /// * `size` - 新的文件大小
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut file = fs.open("/tmp/test.txt")?;
    /// file.truncate(&mut fs, 100)?; // 截断到 100 字节
    /// ```
    pub fn truncate(&mut self, fs: &mut Ext4FileSystem<D>, size: u64) -> Result<()> {
        // 调用文件系统级别的 truncate
        fs.truncate_file(self.inode_num, size)?;

        // 更新本地 inode 的大小（简化：直接设置，不重新加载）
        // 注意：这假设 truncate_file 已经更新了磁盘上的 inode
        self.inode.set_size(size);

        // 如果当前 offset 超过了新大小，调整到文件末尾
        if self.offset > size {
            self.offset = size;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_api() {
        // 这些测试需要实际的块设备和 ext4 文件系统
        // 主要是验证 API 的设计和编译
    }
}
