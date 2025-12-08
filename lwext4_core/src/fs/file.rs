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
