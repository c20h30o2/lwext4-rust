//! Inode 释放功能

use crate::{
    bitmap::*,
    block::{Block, BlockDev, BlockDevice},
    block_group::BlockGroup,
    error::{Error, ErrorKind, Result},
    superblock::Superblock,
};

use super::{checksum::*, helpers::*};

/// 释放一个 inode
///
/// 对应 lwext4 的 `ext4_ialloc_free_inode()`
///
/// # 参数
///
/// * `bdev` - 块设备引用
/// * `sb` - superblock 可变引用
/// * `inode` - 要释放的 inode 编号
/// * `is_dir` - 是否是目录
///
/// # 返回
///
/// 成功返回 Ok(())
pub fn free_inode<D: BlockDevice>(
    bdev: &mut BlockDev<D>,
    sb: &mut Superblock,
    inode: u32,
    is_dir: bool,
) -> Result<()> {
    // 计算块组编号
    let block_group = get_bgid_of_inode(sb, inode);

    // 加载块组描述符
    let mut bg = BlockGroup::load(bdev, sb, block_group)?;

    // 使用作用域确保 bitmap_block 在后续操作前被释放
    {
        // 获取位图块句柄并操作
        let bitmap_block_addr = bg.get_inode_bitmap(sb);
        let mut bitmap_block = Block::get(bdev, bitmap_block_addr)?;

        // 在闭包内操作位图数据
        bitmap_block.with_data_mut(|bitmap_data| {
            // 验证位图校验和（如果启用）
            if !verify_bitmap_csum(sb, &bg, bitmap_data) {
                // 这里只是记录警告，不阻止操作
                // 在实际应用中可以添加日志
            }

            // 在位图中释放 inode
            let index_in_group = inode_to_bgidx(sb, inode);
            clear_bit(bitmap_data, index_in_group)?;

            // 更新位图校验和
            set_bitmap_csum(sb, &mut bg, bitmap_data);

            Ok(())
        })??;
        // bitmap_block 在此处自动释放并写回
    }

    // 如果释放的是目录，递减已使用目录计数
    if is_dir {
        let mut used_dirs = bg.get_used_dirs_count(sb);
        if used_dirs > 0 {
            used_dirs -= 1;
        }
        bg.set_used_dirs_count(sb, used_dirs);
    }

    // 更新块组空闲 inode 计数
    let mut free_inodes = bg.get_free_inodes_count(sb);
    free_inodes += 1;
    bg.set_free_inodes_count(sb, free_inodes);

    // 写回块组描述符
    bg.write(bdev, sb)?;

    // 更新 superblock 空闲 inode 计数
    let sb_free_inodes = sb.free_inodes_count() + 1;
    sb.set_free_inodes_count(sb_free_inodes);

    // 写回 superblock
    sb.write(bdev)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_inode_placeholder() {
        // 这是一个占位测试
        // 实际测试需要创建一个模拟的文件系统
        assert!(true);
    }
}
