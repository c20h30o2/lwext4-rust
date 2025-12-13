//! HTree 分裂功能使用示例
//!
//! 本示例展示了如何使用 lwext4_core 的 HTree 分裂功能。
//! 注意：这是一个概念示例，实际运行需要完整的文件系统支持。

use lwext4_core::{
    BlockDevice, Result,
    dir::{htree, write},
    fs::InodeRef,
    superblock::Superblock,
};

/// 示例：向 HTree 目录添加大量文件
///
/// 此函数演示了如何使用 add_entry 函数，它会在需要时自动触发 HTree 分裂
pub fn add_many_files_example<D: BlockDevice>(
    dir_inode: &mut InodeRef<D>,
    sb: &mut Superblock,
) -> Result<()> {
    // 添加 1000 个文件到目录
    // 当叶子块满时，会自动触发分裂
    for i in 0..1000 {
        let filename = format!("file_{:04}.txt", i);
        let child_inode = 1000 + i; // 假设的 inode 编号

        // add_entry 会自动处理 HTree 分裂
        // 如果叶子块满了，它会：
        // 1. 调用 htree::split_leaf_block 分裂叶子块
        // 2. 向父索引块插入新的索引条目
        // 3. 重试插入操作
        write::add_entry(
            dir_inode,
            sb,
            &filename,
            child_inode,
            write::EXT4_DE_REG_FILE,
        )?;

        if i % 100 == 0 {
            println!("Added {} files", i);
        }
    }

    println!("Successfully added 1000 files with automatic HTree splitting");
    Ok(())
}

/// 示例：手动触发叶子块分裂
///
/// 此函数展示了如何直接调用分裂函数（通常不需要手动调用）
pub fn manual_split_example<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    sb: &mut Superblock,
) -> Result<()> {
    // 1. 初始化哈希信息
    let name = "trigger_split.txt";
    let hash_info = htree::init_hash_info(inode_ref, name)?;

    // 2. 获取叶子块路径
    let path = htree::get_leaf_with_path(inode_ref, &hash_info)?;
    println!("Leaf block: {}", path.leaf_block);
    println!("Index blocks in path: {}", path.index_blocks.len());

    // 3. 获取叶子块的物理地址
    let leaf_block_addr = inode_ref.get_inode_dblk_idx(path.leaf_block, false)?;

    // 4. 手动触发分裂
    let (new_logical_block, split_hash) = htree::split_leaf_block(
        inode_ref,
        sb,
        leaf_block_addr,
        &hash_info,
    )?;

    println!("Split successful!");
    println!("  New block: {}", new_logical_block);
    println!("  Split hash: 0x{:08x}", split_hash);
    println!("  Continued: {}", (split_hash & 1) != 0);

    Ok(())
}

/// 示例：检查 HTree 结构
pub fn inspect_htree_example<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
) -> Result<()> {
    // 检查目录是否启用了 HTree 索引
    let is_indexed = htree::is_indexed(inode_ref)?;
    println!("Directory is indexed: {}", is_indexed);

    if !is_indexed {
        println!("Directory is not indexed, HTree split not applicable");
        return Ok(());
    }

    // 查找一个示例文件
    let test_name = "example.txt";
    if let Some(inode) = htree::find_entry(inode_ref, test_name)? {
        println!("Found '{}' at inode {}", test_name, inode);
    } else {
        println!("'{}' not found", test_name);
    }

    Ok(())
}

/// 完整的工作流程示例
pub fn complete_workflow_example<D: BlockDevice>(
    dir_inode: &mut InodeRef<D>,
    sb: &mut Superblock,
) -> Result<()> {
    println!("=== HTree Split Workflow Example ===");

    // 步骤 1: 检查目录状态
    println!("\n1. Checking directory state...");
    inspect_htree_example(dir_inode)?;

    // 步骤 2: 添加文件直到触发分裂
    println!("\n2. Adding files (will trigger split when needed)...");
    for i in 0..200 {
        let filename = format!("test_{:03}.dat", i);
        let child_inode = 2000 + i;

        match write::add_entry(
            dir_inode,
            sb,
            &filename,
            child_inode,
            write::EXT4_DE_REG_FILE,
        ) {
            Ok(()) => {
                if i % 50 == 0 {
                    println!("  Progress: {} files", i);
                }
            }
            Err(e) => {
                println!("  Error at file {}: {:?}", i, e);
                // 可能遇到了未实现的递归分裂
                if e.kind() == lwext4_core::ErrorKind::NoSpace {
                    println!("  Hit index block limit (recursive split not implemented)");
                    break;
                }
                return Err(e);
            }
        }
    }

    println!("\n3. Workflow complete!");
    Ok(())
}

/// 分裂算法内部细节示例
pub fn split_algorithm_details() {
    println!("=== HTree Split Algorithm Details ===\n");

    println!("Leaf Block Split Algorithm:");
    println!("1. Read all directory entries from the full block");
    println!("2. Calculate hash for each entry name");
    println!("3. Sort entries by hash value");
    println!("4. Find split point at ~50% capacity");
    println!("5. Handle hash collisions (keep same-hash entries together)");
    println!("6. Allocate new physical block");
    println!("7. Write first half to old block, second half to new block");
    println!("8. Update inode size");
    println!("9. Return (new_logical_block, split_hash)\n");

    println!("Index Block Split Algorithm:");
    println!("Non-root split:");
    println!("  1. Allocate new index block");
    println!("  2. Copy right half of entries to new block");
    println!("  3. Update counts in both blocks");
    println!("  4. Return split hash\n");

    println!("Root split (tree growth):");
    println!("  1. Allocate new child block");
    println!("  2. Move all entries to child");
    println!("  3. Root keeps single entry pointing to child");
    println!("  4. Increment indirect_levels");
    println!("  5. Tree height increases by 1\n");
}

fn main() {
    println!("HTree Split功能使用示例\n");
    println!("注意：这些示例需要完整的文件系统支持才能实际运行。");
    println!("当前展示的是 API 使用模式和算法流程。\n");

    // 显示算法细节
    split_algorithm_details();

    println!("\n要运行完整示例，需要：");
    println!("1. 一个已挂载的 ext4 文件系统");
    println!("2. 一个启用 HTree 索引的目录");
    println!("3. 完整的 inode 管理功能");
    println!("\n这些功能将在后续版本中实现。");
}
