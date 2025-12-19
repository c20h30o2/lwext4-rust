//! xattr 公共 API
//!
//! 提供用户级别的扩展属性操作接口

use crate::{Result, Error, ErrorKind, inode::Inode, superblock::Superblock};
use alloc::vec::Vec;

use super::{ibody, block as xattr_block, prefix, write, hash};

/// 列出所有扩展属性
///
/// 对应 lwext4 的 `ext4_xattr_list()`
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode引用
/// * `inode_data` - 完整的 inode 原始数据
/// * `xattr_block_data` - xattr block 数据（如果有）
/// * `buffer` - 输出缓冲区
///
/// # 返回
///
/// 成功返回属性名称列表长度（以 \0 分隔）
///
/// # 示例
///
/// ```ignore
/// let mut buffer = vec![0u8; 1024];
/// let len = list(sb, inode, inode_data, block_data, &mut buffer)?;
/// // buffer 包含: "user.comment\0security.selinux\0"
/// ```
pub fn list(
    sb: &Superblock,
    inode: &Inode,
    inode_data: &[u8],
    xattr_block_data: Option<&[u8]>,
    buffer: &mut [u8],
) -> Result<usize> {
    let mut written = 0;

    // 1. 列出 inode 内部的 xattr
    if inode.get_extra_isize(sb) > 0 {
        let ibody_written = ibody::list_ibody_xattr(sb, inode, inode_data, &mut buffer[written..])?;
        written += ibody_written;
    }

    // 2. 如果有 xattr block，列出 block 中的 xattr
    if let Some(block_data) = xattr_block_data {
        let block_written = xattr_block::list_block_xattr(sb, block_data, &mut buffer[written..])?;
        written += block_written;
    }

    Ok(written)
}

/// 获取扩展属性值
///
/// 对应 lwext4 的 `ext4_xattr_get()`
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode引用
/// * `inode_data` - 完整的 inode 原始数据
/// * `xattr_block_data` - xattr block 数据（如果有）
/// * `name` - 属性名（含前缀，如 "user.comment"）
/// * `buffer` - 输出缓冲区
///
/// # 返回
///
/// 成功返回值的长度，如果属性不存在返回 NotFound 错误
///
/// # 示例
///
/// ```ignore
/// let mut buffer = vec![0u8; 256];
/// let len = get(sb, inode, inode_data, block_data, "user.comment", &mut buffer)?;
/// let value = &buffer[..len];
/// ```
pub fn get(
    sb: &Superblock,
    inode: &Inode,
    inode_data: &[u8],
    xattr_block_data: Option<&[u8]>,
    name: &str,
    buffer: &mut [u8],
) -> Result<usize> {
    // 解析属性名称
    let (name_index, name_without_prefix, _) = prefix::extract_xattr_name(name)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid xattr name"))?;

    // 1. 先在 inode 内部查找
    if inode.get_extra_isize(sb) > 0 {
        if let Some((_, value_offset, value_size)) =
            ibody::find_ibody_entry(sb, inode, inode_data, name_index, name_without_prefix.as_bytes())?
        {
            let value_size = value_size as usize;
            if value_size > buffer.len() {
                return Err(Error::new(ErrorKind::InvalidInput, "buffer too small"));
            }

            // 从 inode 数据中复制值
            buffer[..value_size].copy_from_slice(&inode_data[value_offset..value_offset + value_size]);
            return Ok(value_size);
        }
    }

    // 2. 如果在 inode 内部没找到，尝试在 xattr block 中查找
    if let Some(block_data) = xattr_block_data {
        if let Some((_, value_offset, value_size)) =
            xattr_block::find_block_entry(sb, block_data, name_index, name_without_prefix.as_bytes())?
        {
            let value_size = value_size as usize;
            if value_size > buffer.len() {
                return Err(Error::new(ErrorKind::InvalidInput, "buffer too small"));
            }

            // 从 block 数据中复制值
            buffer[..value_size].copy_from_slice(&block_data[value_offset..value_offset + value_size]);
            return Ok(value_size);
        }
    }

    // 属性不存在
    Err(Error::new(ErrorKind::NotFound, "xattr not found"))
}

/// 设置扩展属性（核心内存操作）
///
/// 对应 lwext4 的 `ext4_xattr_set()`
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode引用
/// * `inode_data` - 完整的 inode 原始数据（可变）
/// * `xattr_block_data` - xattr block 数据（可变，如果有）
/// * `name` - 属性名（含前缀）
/// * `value` - 属性值
///
/// # 返回
///
/// 成功返回 Ok(())
///
/// # 注意
///
/// 此函数执行核心的内存修改操作，包括：
/// 1. Entry 的插入/更新
/// 2. 空间管理
/// 3. 哈希和校验和更新
///
/// 调用者需要负责：
/// - 读取 inode 和 block 数据
/// - 处理 block 分配（如果空间不足）
/// - 处理 block COW（如果引用计数 > 1）
/// - 写回修改后的数据到磁盘
pub fn set(
    sb: &Superblock,
    inode: &Inode,
    inode_data: &mut [u8],
    xattr_block_data: Option<&mut [u8]>,
    name: &str,
    value: &[u8],
) -> Result<()> {
    // 解析属性名称
    let (name_index, name_without_prefix, _) = prefix::extract_xattr_name(name)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid xattr name"))?;

    let name_bytes = name_without_prefix.as_bytes();

    // 1. 尝试在 inode 内部设置
    if inode.get_extra_isize(sb) > 0 {
        use crate::consts::EXT4_GOOD_OLD_INODE_SIZE;
        use core::mem::size_of;
        use crate::types::ext4_xattr_ibody_header;

        let extra_isize = inode.get_extra_isize(sb) as usize;
        let inode_size = sb.inode_size() as usize;
        let header_offset = EXT4_GOOD_OLD_INODE_SIZE as usize + extra_isize;
        let first_offset = header_offset + size_of::<ext4_xattr_ibody_header>();

        // 尝试在 inode 内部设置
        let result = write::set_entry_in_memory(
            inode_data,
            first_offset,
            inode_size,
            name_index,
            name_bytes,
            Some(value),
            false, // 不是 dry_run
        );

        if result.is_ok() {
            // 成功在 inode 内部设置，初始化 header（如果需要）
            if header_offset + 4 <= inode_data.len() {
                let magic_bytes = crate::consts::EXT4_XATTR_MAGIC.to_le_bytes();
                inode_data[header_offset..header_offset + 4].copy_from_slice(&magic_bytes);
            }
            return Ok(());
        }
    }

    // 2. 如果 inode 内部空间不足，尝试在 xattr block 中设置
    if let Some(block_data) = xattr_block_data {
        use core::mem::size_of;
        use crate::types::ext4_xattr_header;

        let first_offset = size_of::<ext4_xattr_header>();
        let block_size = sb.block_size() as usize;

        // 在 block 中设置
        write::set_entry_in_memory(
            block_data,
            first_offset,
            block_size,
            name_index,
            name_bytes,
            Some(value),
            false,
        )?;

        // 更新 block header（如果需要初始化）
        if block_data.len() >= size_of::<ext4_xattr_header>() {
            let magic_bytes = crate::consts::EXT4_XATTR_MAGIC.to_le_bytes();
            block_data[0..4].copy_from_slice(&magic_bytes);
        }

        return Ok(());
    }

    // 3. 如果都不行，返回错误（需要分配新 block）
    Err(Error::new(
        ErrorKind::NoSpace,
        "no space in inode or block, need to allocate new block",
    ))
}

/// 删除扩展属性（核心内存操作）
///
/// 对应 lwext4 的 `ext4_xattr_remove()`
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode引用
/// * `inode_data` - 完整的 inode 原始数据（可变）
/// * `xattr_block_data` - xattr block 数据（可变，如果有）
/// * `name` - 属性名
///
/// # 返回
///
/// 成功返回 Ok(())
///
/// # 注意
///
/// 此函数执行核心的内存修改操作，包括：
/// 1. Entry 的删除
/// 2. 空间回收
/// 3. 哈希和校验和更新
///
/// 调用者需要负责：
/// - 读取 inode 和 block 数据
/// - 如果 block 为空，释放 block
/// - 处理引用计数
/// - 写回修改后的数据到磁盘
pub fn remove(
    sb: &Superblock,
    inode: &Inode,
    inode_data: &mut [u8],
    xattr_block_data: Option<&mut [u8]>,
    name: &str,
) -> Result<()> {
    // 解析属性名称
    let (name_index, name_without_prefix, _) = prefix::extract_xattr_name(name)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid xattr name"))?;

    let name_bytes = name_without_prefix.as_bytes();

    // 1. 尝试在 inode 内部删除
    if inode.get_extra_isize(sb) > 0 {
        use crate::consts::EXT4_GOOD_OLD_INODE_SIZE;
        use core::mem::size_of;
        use crate::types::ext4_xattr_ibody_header;

        let extra_isize = inode.get_extra_isize(sb) as usize;
        let inode_size = sb.inode_size() as usize;
        let header_offset = EXT4_GOOD_OLD_INODE_SIZE as usize + extra_isize;
        let first_offset = header_offset + size_of::<ext4_xattr_ibody_header>();

        // 尝试在 inode 内部删除
        let result = write::set_entry_in_memory(
            inode_data,
            first_offset,
            inode_size,
            name_index,
            name_bytes,
            None, // 删除
            false,
        );

        // 如果成功删除或属性不存在，都返回成功
        if result.is_ok() {
            return Ok(());
        }
    }

    // 2. 如果在 inode 内部没找到，尝试在 xattr block 中删除
    if let Some(block_data) = xattr_block_data {
        use core::mem::size_of;
        use crate::types::ext4_xattr_header;

        let first_offset = size_of::<ext4_xattr_header>();
        let block_size = sb.block_size() as usize;

        // 在 block 中删除
        write::set_entry_in_memory(
            block_data,
            first_offset,
            block_size,
            name_index,
            name_bytes,
            None, // 删除
            false,
        )?;

        return Ok(());
    }

    // 3. 属性不存在（幂等操作，返回成功）
    Ok(())
}
