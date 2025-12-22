//! inode 内部 xattr 操作
//!
//! 处理存储在 inode 额外空间中的扩展属性

use crate::{
    consts::*,
    error::{Error, ErrorKind, Result},
    inode::Inode,
    superblock::Superblock,
    types::{ext4_xattr_entry, ext4_xattr_ibody_header},
};
use core::mem::size_of;

use super::search::XattrSearch;

/// 获取 inode 内部 xattr header 的偏移
///
/// 对应 C 宏 `EXT4_XATTR_IHDR(sb, raw_inode)`
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode 引用
///
/// # 返回
///
/// header 在 inode 数据中的偏移，如果没有 extra_isize 则返回 None
fn get_ibody_header_offset(sb: &Superblock, inode: &Inode) -> Option<usize> {
    let extra_isize = inode.get_extra_isize(sb) as usize;
    if extra_isize == 0 {
        return None;
    }

    // header 位置 = EXT4_GOOD_OLD_INODE_SIZE + extra_isize
    Some(EXT4_GOOD_OLD_INODE_SIZE as usize + extra_isize)
}

/// 获取第一个 entry 的偏移
///
/// 对应 C 宏 `EXT4_XATTR_IFIRST(hdr)`
///
/// # 参数
///
/// * `header_offset` - header 偏移
///
/// # 返回
///
/// 第一个 entry 的偏移
#[inline]
fn get_first_entry_offset(header_offset: usize) -> usize {
    // first entry = header + sizeof(ext4_xattr_ibody_header)
    header_offset + size_of::<ext4_xattr_ibody_header>()
}

/// 验证 inode 内部 xattr 数据的有效性
///
/// 对应 lwext4 的 `ext4_xattr_is_ibody_valid()`
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode 引用
/// * `inode_data` - 完整的 inode 原始数据
///
/// # 返回
///
/// 如果有效返回 Ok(())，否则返回错误
/// issue: 这里的参数仍然有优化空间， 使用Inode与切片无法保障一致性
pub fn validate_ibody_xattr(sb: &Superblock, inode: &Inode, inode_data: &[u8]) -> Result<()> {
    let header_offset = match get_ibody_header_offset(sb, inode) {
        Some(offset) => offset,
        None => return Ok(()), // 没有 extra_isize，跳过验证
    };

    let inode_size = sb.inode_size() as usize;

    // 检查 header 是否在范围内
    if header_offset + size_of::<ext4_xattr_ibody_header>() > inode_size {
        return Err(Error::new(ErrorKind::InvalidInput, "xattr header out of inode bounds"));
    }

    // 读取 header
    let header_bytes = &inode_data[header_offset..header_offset + size_of::<ext4_xattr_ibody_header>()];
    let header = unsafe {
        core::ptr::read(header_bytes.as_ptr() as *const ext4_xattr_ibody_header)
    };

    // 检查魔数
    if u32::from_le(header.h_magic) != EXT4_XATTR_MAGIC {
        return Err(Error::new(ErrorKind::InvalidInput, "invalid xattr magic number"));
    }

    // 验证所有 entry
    let base = header_offset;
    let end = inode_size;
    let mut min_offs = end - base;

    let first_entry_offset = get_first_entry_offset(header_offset);
    let mut entry_offset = first_entry_offset;

    loop {
        // 检查是否到达末尾
        if entry_offset + 4 > end {
            break;
        }

        // 检查是否是最后一个 entry
        let first_u32 = u32::from_le_bytes([
            inode_data[entry_offset],
            inode_data[entry_offset + 1],
            inode_data[entry_offset + 2],
            inode_data[entry_offset + 3],
        ]);

        if first_u32 == 0 {
            break;
        }

        // 读取 entry
        if entry_offset + size_of::<ext4_xattr_entry>() > end {
            return Err(Error::new(ErrorKind::InvalidInput, "entry out of bounds"));
        }

        let entry_bytes = &inode_data[entry_offset..entry_offset + size_of::<ext4_xattr_entry>()];
        let entry = unsafe {
            core::ptr::read(entry_bytes.as_ptr() as *const ext4_xattr_entry)
        };

        let value_size = entry.value_size();
        let value_offs = u16::from_le(entry.e_value_offs) as usize;

        // 检查：如果 value_size 为 0，value_offs 也应该为 0
        if value_size == 0 && value_offs != 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "invalid value offset for empty value"));
        }

        // 检查 value 是否在范围内
        if value_size > 0 {
            if base + value_offs + value_size as usize > end {
                return Err(Error::new(ErrorKind::InvalidInput, "value out of bounds"));
            }

            // 更新最小值偏移
            if value_offs < min_offs {
                min_offs = value_offs;
            }
        }

        // 移动到下一个 entry
        let name_len = entry.e_name_len as usize;
        let entry_len = ((name_len + EXT4_XATTR_ROUND as usize + size_of::<ext4_xattr_entry>())
            & !(EXT4_XATTR_ROUND as usize));
        entry_offset += entry_len;
    }

    Ok(())
}

/// 初始化 inode 内部 xattr 区域
///
/// 对应 lwext4 的 `ext4_xattr_ibody_initialize()`
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode 引用
/// * `inode_data` - 完整的 inode 原始数据（可变）
///
/// # 返回
///
/// 成功返回 Ok(())
/// issue: 从未被调用， 且缺乏一致性保障
pub fn initialize_ibody_xattr(sb: &Superblock, inode: &Inode, inode_data: &mut [u8]) -> Result<()> {
    let header_offset = match get_ibody_header_offset(sb, inode) {
        Some(offset) => offset,
        None => return Ok(()), // 没有 extra_isize，无需初始化
    };

    let inode_size = sb.inode_size() as usize;
    let extra_isize = inode.get_extra_isize(sb) as usize;

    // 清零 xattr 区域
    let xattr_start = EXT4_GOOD_OLD_INODE_SIZE as usize + extra_isize;
    let xattr_end = inode_size;

    if xattr_start < xattr_end && xattr_end <= inode_data.len() {
        for byte in &mut inode_data[xattr_start..xattr_end] {
            *byte = 0;
        }
    }

    // 设置魔数
    if header_offset + 4 <= inode_data.len() {
        let magic_bytes = EXT4_XATTR_MAGIC.to_le_bytes();
        inode_data[header_offset..header_offset + 4].copy_from_slice(&magic_bytes);
    }

    Ok(())
}

/// 在 inode 内部查找 xattr entry
///
/// 对应 lwext4 的 `ext4_xattr_ibody_find_entry()`
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode 引用
/// * `inode_data` - 完整的 inode 原始数据
/// * `name_index` - 命名空间索引
/// * `name` - 属性名称（不含前缀）
///
/// # 返回
///
/// 成功返回 Some((entry_offset, value_offset, value_size))，未找到返回 None
pub fn find_ibody_entry(
    sb: &Superblock,
    inode: &Inode,
    inode_data: &[u8],
    name_index: u8,
    name: &[u8],
) -> Result<Option<(usize, usize, u32)>> {
    let header_offset = match get_ibody_header_offset(sb, inode) {
        Some(offset) => offset,
        None => return Ok(None), // 没有 extra_isize
    };

    let inode_size = sb.inode_size() as usize;

    // 验证数据有效性
    validate_ibody_xattr(sb, inode, inode_data)?;

    // 创建搜索上下文
    let first_entry_offset = get_first_entry_offset(header_offset);

    // 构造从 header 开始到 inode 结束的数据切片
    let xattr_data = &inode_data[header_offset..inode_size];
    let first_offset_in_slice = first_entry_offset - header_offset;

    let mut search = XattrSearch::new(xattr_data, first_offset_in_slice);

    // 查找 entry
    if let Some((entry_offset_in_slice, value_offset, value_size)) = search.find_entry(name_index, name) {
        // 将相对偏移转换为绝对偏移
        let entry_offset = header_offset + entry_offset_in_slice;
        let value_offset_abs = if value_offset > 0 {
            header_offset + value_offset
        } else {
            0
        };

        Ok(Some((entry_offset, value_offset_abs, value_size)))
    } else {
        Ok(None)
    }
}

/// 列出 inode 内部的所有 xattr entry 名称
///
/// # 参数
///
/// * `sb` - superblock
/// * `inode` - inode 引用
/// * `inode_data` - 完整的 inode 原始数据
/// * `buffer` - 输出缓冲区（名称以 \0 分隔）
///
/// # 返回
///
/// 成功返回写入的字节数
pub fn list_ibody_xattr(
    sb: &Superblock,
    inode: &Inode,
    inode_data: &[u8],
    buffer: &mut [u8],
) -> Result<usize> {
    let header_offset = match get_ibody_header_offset(sb, inode) {
        Some(offset) => offset,
        None => return Ok(0), // 没有 extra_isize
    };

    let inode_size = sb.inode_size() as usize;

    // 验证数据有效性
    validate_ibody_xattr(sb, inode, inode_data)?;

    let first_entry_offset = get_first_entry_offset(header_offset);
    let mut entry_offset = first_entry_offset;
    let mut written = 0;

    loop {
        // 检查是否到达末尾
        if entry_offset + 4 > inode_size {
            break;
        }

        // 检查是否是最后一个 entry
        let first_u32 = u32::from_le_bytes([
            inode_data[entry_offset],
            inode_data[entry_offset + 1],
            inode_data[entry_offset + 2],
            inode_data[entry_offset + 3],
        ]);

        if first_u32 == 0 {
            break;
        }

        // 读取 entry
        if entry_offset + size_of::<ext4_xattr_entry>() > inode_size {
            break;
        }

        let entry_bytes = &inode_data[entry_offset..entry_offset + size_of::<ext4_xattr_entry>()];
        let entry = unsafe {
            core::ptr::read(entry_bytes.as_ptr() as *const ext4_xattr_entry)
        };

        let name_len = entry.e_name_len as usize;
        let name_offset = entry_offset + size_of::<ext4_xattr_entry>();

        if name_offset + name_len <= inode_size {
            let entry_name = &inode_data[name_offset..name_offset + name_len];

            // 获取命名空间前缀
            use super::prefix::get_xattr_name_prefix;
            if let Some((prefix, prefix_len)) = get_xattr_name_prefix(entry.e_name_index) {
                let total_len = prefix_len + name_len + 1; // +1 for null terminator

                if written + total_len <= buffer.len() {
                    // 写入前缀
                    buffer[written..written + prefix_len].copy_from_slice(prefix.as_bytes());
                    written += prefix_len;

                    // 写入名称
                    buffer[written..written + name_len].copy_from_slice(entry_name);
                    written += name_len;

                    // 写入 null terminator
                    buffer[written] = 0;
                    written += 1;
                }
            }
        }

        // 移动到下一个 entry
        let entry_len = ((name_len + EXT4_XATTR_ROUND as usize + size_of::<ext4_xattr_entry>())
            & !(EXT4_XATTR_ROUND as usize));
        entry_offset += entry_len;
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ibody_header_offset() {
        // 模拟一个有 extra_isize 的 inode
        // 这里需要创建一个假的 superblock 和 inode
        // 暂时跳过，因为需要完整的上下文
    }

    #[test]
    fn test_validate_empty_xattr() {
        // 测试空的 xattr 区域
        // 需要构造测试数据
    }
}
