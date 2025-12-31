//! Extent 树写操作
//!
//! 对应 lwext4 的 ext4_extent.c 中的写操作部分
//!
//! ## 功能
//!
//! - ✅ Extent 树初始化 (`tree_init`)
//! - ✅ Extent 插入 (简化版本 - 仅支持深度 0)
//! - ✅ Extent 节点分裂 (ExtentWriter)
//! - ✅ Extent 块获取/分配 (`get_blocks`)
//!   - ✅ 查找现有映射
//!   - ✅ 分配新块（集成 balloc）
//!   - ✅ 自动插入新 extent
//!   - ✅ 失败时自动回滚
//! - ✅ Extent 移除 (`remove_space`)
//!   - ✅ 完全删除 extent
//!   - ✅ 部分删除（截断开头或结尾）
//!   - ✅ 中间删除（分裂 extent）
//!   - ✅ 自动释放物理块
//! - ⚠️ Extent 合并（部分实现）
//!
//! ## 依赖
//!
//! - Transaction 系统（用于保证原子性）
//! - balloc 模块（用于分配和释放物理块）
//!
//! ## 当前限制
//!
//! - `insert_extent_simple` 和 `remove_space` 仅支持深度为 0 的 extent 树
//! - 多层 extent 树支持需要使用 `ExtentWriter`

use crate::{
    balloc::{self, BlockAllocator},
    block::{Block, BlockDevice},
    consts::*,
    error::{Error, ErrorKind, Result},
    fs::InodeRef,
    superblock::Superblock,
    transaction::SimpleTransaction,
    types::{ext4_extent, ext4_extent_header, ext4_extent_idx},
};
use alloc::vec::Vec;

//=============================================================================
// Extent 树初始化
//=============================================================================

/// 初始化 extent 树
///
/// 对应 lwext4 的 `ext4_extent_tree_init()`
///
/// 在 inode 中初始化一个空的 extent 树，用于新创建的文件。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
///
/// # 实现细节
///
/// 1. 获取 inode 中的 extent header（在 inode.blocks 数组中）
/// 2. 设置 header 的各个字段：
///    - depth = 0（根节点即叶子）
///    - entries_count = 0（空树）
///    - generation = 0
///    - magic = 0xF30A
/// 3. 计算 max_entries（基于 inode.blocks 的大小）
/// 4. 标记 inode 为 dirty
///
/// # 示例
///
/// ```rust,ignore
/// use lwext4_core::extent::tree_init;
///
/// // 为新创建的 inode 初始化 extent 树
/// tree_init(&mut inode_ref)?;
/// ```
pub fn tree_init<D: BlockDevice>(inode_ref: &mut InodeRef<D>) -> Result<()> {
    // Extent 树魔数
    const EXT4_EXTENT_MAGIC: u16 = 0xF30A;

    // 在 inode 中直接修改 extent header
    inode_ref.with_inode_mut(|inode| {
        // inode.blocks 是 15 个 u32，总共 60 字节
        // 前面是 ext4_extent_header，后面是 extent 或 extent_idx 数组
        let header_ptr = inode.blocks.as_mut_ptr() as *mut ext4_extent_header;
        let header = unsafe { &mut *header_ptr };

        // 设置 header 字段
        header.depth = 0u16.to_le();       // 根节点即叶子
        header.entries = 0u16.to_le();     // 空树
        header.max = 0u16.to_le();         // 稍后计算
        header.magic = EXT4_EXTENT_MAGIC.to_le(); // 0xF30A
        header.generation = 0u32.to_le();

        // 计算 max_entries
        // inode.blocks 是 60 字节，减去 header (12 字节)，剩下可以存放 extent
        // 每个 ext4_extent 是 12 字节
        const INODE_BLOCKS_SIZE: usize = 60; // 15 * 4
        const HEADER_SIZE: usize = core::mem::size_of::<ext4_extent_header>();
        const EXTENT_SIZE: usize = core::mem::size_of::<ext4_extent>();

        let max_entries = (INODE_BLOCKS_SIZE - HEADER_SIZE) / EXTENT_SIZE;
        header.max = (max_entries as u16).to_le();
    })?;

    // 标记 inode 为 dirty
    inode_ref.mark_dirty();

    Ok(())
}

//=============================================================================
// Extent 块获取和分配
//=============================================================================

/// 查找下一个已分配的逻辑块
///
/// 对应 lwext4 的 `ext4_ext_next_allocated_block()`
///
/// 用于确定可以分配多少块而不会覆盖已有的 extent。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `logical_block` - 当前逻辑块号
///
/// # 返回
///
/// 下一个已分配的逻辑块号，如果没有则返回 u32::MAX
/// issue: 这个函数只处理了深度为0的情况，但是却被get_blocks调用，可能是一个bug
fn find_next_allocated_block<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    logical_block: u32,
) -> Result<u32> {
    // 读取 extent 树根节点
    let (root_data, depth) = inode_ref.with_inode(|inode| {
        let root_data = unsafe {
            core::slice::from_raw_parts(
                inode.blocks.as_ptr() as *const u8,
                60, // 15 * 4
            ).to_vec()
        };

        let header = unsafe {
            *(root_data.as_ptr() as *const ext4_extent_header)
        };

        (root_data, u16::from_le(header.depth))
    })?;

    // 如果深度为 0，直接在根节点查找
    if depth == 0 {
        let header = unsafe { *(root_data.as_ptr() as *const ext4_extent_header) };
        let entries = u16::from_le(header.entries);
        let header_size = core::mem::size_of::<ext4_extent_header>();
        let extent_size = core::mem::size_of::<ext4_extent>();

        let mut next_block = u32::MAX;

        for i in 0..entries as usize {
            let offset = header_size + i * extent_size;
            if offset + extent_size > root_data.len() {
                break;
            }

            let extent = unsafe {
                *(root_data.as_ptr().add(offset) as *const ext4_extent)
            };

            let ee_block = u32::from_le(extent.block);

            // 找到第一个大于 logical_block 的 extent
            if ee_block > logical_block && ee_block < next_block {
                next_block = ee_block;
            }
        }

        return Ok(next_block);
    }

    // TODO: 支持多层树
    Ok(u32::MAX)
}

/// 计算块分配目标
///
/// 对应 lwext4 的 `ext4_ext_find_goal()`
///
/// 根据当前文件的 extent 分布，智能选择一个物理块作为分配目标，作为该inode该逻辑块对应的物理块
/// 这有助于减少文件碎片化。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `logical_block` - 要分配的逻辑块号
///
/// # 返回
///
/// 建议的物理块起始地址（goal）
///
/// # 策略
///
/// 1. 如果存在相邻的 extent，尝试在其后继续分配
/// 2. 否则，使用 inode 所在块组的默认位置
fn find_goal<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    logical_block: u32,
) -> Result<u64> {
    // 尝试查找最接近的 extent
    let extent_opt = find_extent_for_block(inode_ref, logical_block)?;

    if let Some(extent) = extent_opt {
        let ee_block = u32::from_le(extent.block);
        let ee_start_lo = u32::from_le(extent.start_lo);
        let ee_start_hi = u16::from_le(extent.start_hi);
        let ee_start = (ee_start_hi as u64) << 32 | (ee_start_lo as u64);

        // 如果逻辑块在当前 extent 之后，预测物理块也应该在其后
        if logical_block > ee_block {
            return Ok(ee_start + (logical_block - ee_block) as u64);
        } else {
            // 如果在之前，尝试在其前面分配（反向写）
            return Ok(ee_start.saturating_sub((ee_block - logical_block) as u64));
        }
    }

    // 如果没有找到相邻 extent，使用 inode 所在块组的默认位置
    // 这是最保守的 fallback 策略
    Ok(0) // 0 表示让 balloc 自己选择
}

/// 获取或分配物理块
///
/// 对应 lwext4 的 `ext4_extent_get_blocks()`
///
/// 给定逻辑块号，返回对应的物理块号。如果逻辑块尚未映射，
/// 根据 `create` 参数决定是否分配新的物理块。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `logical_block` - 逻辑块号
/// * `max_blocks` - 最多获取/分配的块数
/// * `create` - 如果为 true，在块不存在时分配新块
///
/// # 返回
///
/// * `Ok((physical_block, allocated_count))` - 物理块号和实际分配的块数
///   - 如果 `physical_block` 为 0，表示块不存在且未创建
/// * `Err(_)` - 发生错误
///
/// # 实现状态
///
/// - ✅ 查找现有 extent
/// - ✅ 返回已映射的物理块
/// - ⏳ 块分配（需要集成 balloc）
/// - ⏳ 未初始化 extent 处理
///
/// # 示例
///
/// ```rust,ignore
/// // 查找逻辑块 100 对应的物理块
/// let (phys_block, count) = get_blocks(&mut inode_ref, 100, 1, false)?;
/// if phys_block == 0 {
///     println!("Block not allocated");
/// }
///
/// // 分配新块
/// let (phys_block, count) = get_blocks(&mut inode_ref, 100, 10, true)?;
/// ```
pub fn get_blocks<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    sb: &mut Superblock,
    allocator: &mut BlockAllocator,
    logical_block: u32,
    max_blocks: u32,
    create: bool,
) -> Result<(u64, u32)> {
    // 1. 查找包含此逻辑块的 extent
    let extent_opt = find_extent_for_block(inode_ref, logical_block)?;

    if let Some(extent) = extent_opt {
        // 提取 extent 信息
        let ee_block = u32::from_le(extent.block);
        let ee_len = u16::from_le(extent.len);
        let ee_start_lo = u32::from_le(extent.start_lo);
        let ee_start_hi = u16::from_le(extent.start_hi);

        // 计算物理块起始地址
        let ee_start = (ee_start_hi as u64) << 32 | (ee_start_lo as u64);

        // 检查逻辑块是否在这个 extent 范围内
        if logical_block >= ee_block && logical_block < ee_block + ee_len as u32 {
            // 计算物理块号
            let offset = logical_block - ee_block;
            let physical_block = ee_start + offset as u64;

            // 计算剩余块数
            let remaining = ee_len as u32 - offset;
            let allocated = remaining.min(max_blocks);

            return Ok((physical_block, allocated));
        }
    }

    // 2. 没有找到包含此逻辑块的 extent
    if !create {
        // 不创建，返回 0
        return Ok((0, 0));
    }

    // 3. 分配新块并使用 ExtentWriter 插入
    //
    // 与 lwext4 一致：使用完整的 extent 插入逻辑（支持自动 split/grow/merge）
    // 而不是简化版的 insert_extent_simple

    // 3.1 计算可以分配多少块（不能超过下一个已分配的 extent）
    // find_next_allocated_block无法处理深度大于0的情况， 对于深度大于0的树， 永远返回i32::MAX 虽然应该不会导致bug， 但是需要进一步优化
    let next_allocated = find_next_allocated_block(inode_ref, logical_block)?;
    let mut allocated_count = if next_allocated > logical_block {
        (next_allocated - logical_block).min(max_blocks)
    } else {
        max_blocks
    };

    // 3.2 计算分配目标（goal）
    let goal = find_goal(inode_ref, logical_block)?;

    // 3.3 分配物理块（支持批量分配）
    let (physical_block, actual_allocated) = balloc::alloc_blocks(
        inode_ref.bdev(),
        sb,
        goal,
        allocated_count,
    )?;
    allocated_count = actual_allocated;

    // 3.4 插入新 extent（支持自动 split/grow）
    // 与 lwext4 的 ext4_ext_insert_extent 行为一致
    //
    // 逻辑：
    // 1. 检查根节点是否满
    // 2. 如果满了，先 grow_tree_depth 增加树深度
    // 3. 然后插入extent (使用通用的 insert_extent_any_depth)

    let insert_result = insert_extent_with_auto_split(
        inode_ref,
        sb,
        allocator,
        logical_block,
        physical_block,
        allocated_count,
    );

    match insert_result {
        Ok(_) => {
            // 成功插入，返回分配的块
            Ok((physical_block, allocated_count))
        }
        Err(e) => {
            // 插入失败，释放已分配的块
            let _ = balloc::free_blocks(
                inode_ref.bdev(),
                sb,
                physical_block,
                allocated_count,
            );
            Err(e)
        }
    }
}

/// 插入 extent 并自动处理 split/grow（无事务版本）
///
/// 这个函数实现了与 lwext4 的 ext4_ext_insert_extent 类似的逻辑，
/// 但不需要事务系统支持。
///
/// # 功能
///
/// 1. 检查根节点是否满
/// 2. 如果满了，调用 grow_tree_depth 增加树深度
/// 3. 插入 extent 到适当的位置
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `sb` - Superblock
/// * `allocator` - 块分配器
/// * `logical_block` - 逻辑块号
/// * `physical_block` - 物理块号
/// * `length` - extent 长度（块数）
fn insert_extent_with_auto_split<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    sb: &mut Superblock,
    allocator: &mut BlockAllocator,
    logical_block: u32,
    physical_block: u64,
    length: u32,
) -> Result<()> {
    // 1. 检查根节点是否满
    let (is_full, depth, entries, max) = inode_ref.with_inode(|inode| -> (bool, u16, u16, u16) {
        let header_ptr = inode.blocks.as_ptr() as *const ext4_extent_header;
        let header = unsafe { &*header_ptr };

        let entries = u16::from_le(header.entries);
        let max = u16::from_le(header.max);
        let depth = u16::from_le(header.depth);

        (entries >= max, depth, entries, max)
    })?;

    log::debug!(
        "[EXTENT_INSERT] logical={}, physical=0x{:x}, len={}, is_full={}, depth={}, entries={}/{}",
        logical_block, physical_block, length, is_full, depth, entries, max
    );

    // 2. 根据当前状态决定插入策略
    if is_full {
        // 根节点满了，需要增加树深度
        log::debug!("[EXTENT_INSERT] Root is FULL, calling grow_tree_depth (depth {} -> {})", depth, depth + 1);
        let new_block = super::grow_tree_depth(inode_ref, sb, allocator)?;

        // 关键修复：grow 后需要根据新深度确定如何插入
        // - 如果原 depth = 0，grow 后 depth = 1，new_block 是叶子节点（depth=0）
        // - 如果原 depth >= 1，grow 后 depth >= 2，new_block 是索引节点，需要继续遍历
        let new_depth = depth + 1;

        // 根据新深度确定目标叶子块
        let leaf_block: u64 = (match new_depth {
            1 => {
                // depth 0->1: new_block 就是叶子节点
                log::debug!("[EXTENT_INSERT] After grow (0->1), new_block 0x{:x} is leaf", new_block);
                Ok(new_block)
            }
            2 => {
                // depth 1->2: new_block 是索引节点，读取其第一个 index 指向的叶子块
                log::debug!("[EXTENT_INSERT] After grow (1->2), new_block 0x{:x} is index node", new_block);

                Block::get(inode_ref.bdev(), new_block)
                    .and_then(|mut idx_block| {
                        // with_data 返回 Result<Result<u64>>, 需要展开外层并返回内层
                        match idx_block.with_data(|data| -> Result<u64> {
                            let header = unsafe {
                                *(data.as_ptr() as *const ext4_extent_header)
                            };

                            let depth_check = u16::from_le(header.depth);
                            if depth_check != 1 {
                                log::error!("[EXTENT_INSERT] Expected depth=1 in new index block, got {}", depth_check);
                                return Err(Error::new(
                                    ErrorKind::Corrupted,
                                    "Expected depth=1 in new index block after grow",
                                ));
                            }

                            // 读取第一个 index
                            let header_size = core::mem::size_of::<ext4_extent_header>();
                            let idx_ptr = unsafe {
                                (data.as_ptr() as *const u8).add(header_size) as *const ext4_extent_idx
                            };
                            let idx = unsafe { &*idx_ptr };

                            let leaf = super::helpers::ext4_idx_pblock(idx);
                            log::debug!("[EXTENT_INSERT] Read first index from 0x{:x}: leaf=0x{:x}", new_block, leaf);
                            Ok(leaf)
                        }) {
                            Ok(inner_result) => inner_result,  // 返回内层 Result<u64>
                            Err(e) => Err(e),
                        }
                    })
            }
            _ => {
                // depth > 2: 需要递归遍历，暂不支持
                // issue: depth>2 递归支持
                log::error!("[EXTENT_INSERT] Tree depth {} not supported after grow", new_depth);
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "Tree depth > 2 not supported after grow",
                ))
            }
        })?;

        log::debug!("[EXTENT_INSERT] After grow, inserting to leaf block 0x{:x}", leaf_block);
        insert_extent_to_leaf_direct(inode_ref, sb, allocator, leaf_block, logical_block, physical_block, length)?;
    } else if depth == 0 {
        // 深度为 0 且未满，直接插入到根节点（inode.blocks）
        log::debug!("[EXTENT_INSERT] Depth=0 and not full, using insert_extent_simple");
        let extent = ext4_extent {
            block: logical_block.to_le(),
            len: (length as u16).to_le(),
            start_hi: ((physical_block >> 32) as u16).to_le(),
            start_lo: (physical_block as u32).to_le(),
        };

        insert_extent_simple(inode_ref, &extent)?;
    } else {
        // 深度 > 0 且未满，需要插入到叶子节点
        log::debug!("[EXTENT_INSERT] Depth={} and not full, inserting to leaf", depth);

        // 读取 leaf_block（使用独立的 Block::get 避免 inode 缓存问题）
        let leaf_block = read_first_leaf_block(inode_ref)?;
        log::debug!("[EXTENT_INSERT] Read leaf_block from inode: 0x{:x}", leaf_block);

        insert_extent_to_leaf_direct(inode_ref, sb, allocator, leaf_block, logical_block, physical_block, length)?;
    }

    Ok(())
}

/// 读取 inode 中第一个索引的 leaf_block
///
/// 注意：使用 with_inode_mut 而非 with_inode 来读取，确保能看到最新的修改
/// 即使我们不修改 inode，使用 mut 访问也能保证读到最新的 Block 缓存数据
fn read_first_leaf_block<D: BlockDevice>(inode_ref: &mut InodeRef<D>) -> Result<u64> {
    // 使用 with_inode_mut 而不是 with_inode 来读取
    // 这确保了我们能读到 grow_tree_depth 中 with_inode_mut 的最新修改
    inode_ref.with_inode_mut(|inode| -> Result<u64> {
        // 读取 extent header
        let header_ptr = inode.blocks.as_ptr() as *const ext4_extent_header;
        let header = unsafe { &*header_ptr };

        let depth = u16::from_le(header.depth);
        if depth == 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "read_first_leaf_block called on depth-0 tree",
            ));
        }

        // 读取第一个索引
        let header_size = core::mem::size_of::<ext4_extent_header>();
        let idx_ptr = unsafe {
            // 关键修复：inode.blocks 是 [u32; 15]，需要先转为 *const u8 再按字节偏移
            (inode.blocks.as_ptr() as *const u8).add(header_size) as *const ext4_extent_idx
        };
        let idx = unsafe { &*idx_ptr };

        // 使用辅助函数而不是手动组合
        let leaf_block = super::helpers::ext4_idx_pblock(idx);

        log::debug!(
            "[READ_LEAF_BLOCK] depth={}, leaf_lo=0x{:x}, leaf_hi=0x{:x}, leaf_block=0x{:x}",
            depth, u32::from_le(idx.leaf_lo), u16::from_le(idx.leaf_hi), leaf_block
        );

        // 打印整个 inode.blocks 的前 28 字节（header 12 + index 12 + 额外 4）
        let data_slice = unsafe {
            core::slice::from_raw_parts(inode.blocks.as_ptr() as *const u8, 28)
        };
        log::debug!("[READ_LEAF_BLOCK] inode.blocks[0..28]: {:02x?}", data_slice);

        Ok(leaf_block)
    })?
}

/// 直接插入 extent 到指定的叶子块（支持分裂）
///
/// 这个函数直接使用给定的 leaf_block，而不是从 inode 读取索引。
/// 当叶子满时，会自动构建路径并执行分裂操作。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `sb` - Superblock 引用
/// * `allocator` - 块分配器（用于分裂时分配新块）
/// * `leaf_block` - 叶子块地址
/// * `logical_block` - 要插入的逻辑块号
/// * `physical_block` - 要插入的物理块号
/// * `length` - extent 长度
fn insert_extent_to_leaf_direct<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    sb: &mut Superblock,
    allocator: &mut BlockAllocator,
    leaf_block: u64,
    logical_block: u32,
    physical_block: u64,
    length: u32,
) -> Result<()> {
    log::debug!(
        "[EXTENT_LEAF_DIRECT] Inserting to leaf block 0x{:x}: logical={}, physical=0x{:x}, len={}",
        leaf_block, logical_block, physical_block, length
    );

    // 首先尝试直接插入
    let insert_result = try_insert_to_leaf_block(
        inode_ref.bdev(),
        leaf_block,
        logical_block,
        physical_block,
        length,
    );

    match insert_result {
        Ok(()) => {
            log::debug!("[EXTENT_LEAF_DIRECT] Insert succeeded without split");
            Ok(())
        }
        Err(e) if e.kind() == ErrorKind::NoSpace => {
            log::debug!("[EXTENT_LEAF_DIRECT] Leaf is full, need to split");

            // 构建 ExtentPath 用于分裂
            let mut path = build_extent_path_for_leaf(inode_ref, leaf_block)?;

            // 执行分裂（在 path 的最后一个节点，即叶子节点）
            let leaf_at = path.nodes.len() - 1;
            log::debug!(
                "[EXTENT_LEAF_DIRECT] Calling split_extent_node at depth={}, leaf_at={}",
                path.nodes[leaf_at].depth, leaf_at
            );

            super::split_extent_node(
                inode_ref,
                sb,
                allocator,
                &mut path,
                leaf_at,
                logical_block,
            )?;

            log::debug!("[EXTENT_LEAF_DIRECT] Split succeeded, retrying insert");

            // 分裂后，需要重新确定应该插入到哪个叶子节点
            // 可能是原来的 leaf_block，也可能是新分裂出来的块
            let new_leaf_block = determine_target_leaf_after_split(
                inode_ref,
                &path,
                logical_block,
            )?;

            log::debug!(
                "[EXTENT_LEAF_DIRECT] Target leaf after split: 0x{:x}",
                new_leaf_block
            );

            // 重试插入（分裂后必定有空间）
            try_insert_to_leaf_block(
                inode_ref.bdev(),
                new_leaf_block,
                logical_block,
                physical_block,
                length,
            )?;

            log::debug!("[EXTENT_LEAF_DIRECT] Retry insert succeeded");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// 尝试插入 extent 到叶子块（不处理分裂）
///
/// 这是一个辅助函数，仅执行插入操作。如果块满，返回 NoSpace 错误。
fn try_insert_to_leaf_block<D: BlockDevice>(
    bdev: &mut crate::block::BlockDev<D>,
    leaf_block: u64,
    logical_block: u32,
    physical_block: u64,
    length: u32,
) -> Result<()> {
    let mut block = Block::get(bdev, leaf_block)?;
    block.with_data_mut(|data| {
        let header = unsafe {
            &mut *(data.as_mut_ptr() as *mut ext4_extent_header)
        };

        if !header.is_valid() {
            return Err(Error::new(
                ErrorKind::Corrupted,
                "Invalid extent header in leaf block",
            ));
        }

        let entries_count = u16::from_le(header.entries);
        let max_entries = u16::from_le(header.max);

        if entries_count >= max_entries {
            return Err(Error::new(
                ErrorKind::NoSpace,
                "Leaf block is full",
            ));
        }

        // 插入 extent 到叶子块
        let header_size = core::mem::size_of::<ext4_extent_header>();
        let extent_size = core::mem::size_of::<ext4_extent>();

        // 找到插入位置（保持排序）
        let mut insert_pos = entries_count as usize;
        for i in 0..entries_count as usize {
            let offset = header_size + i * extent_size;
            let existing_extent = unsafe {
                &*(data[offset..].as_ptr() as *const ext4_extent)
            };

            if u32::from_le(existing_extent.block) > logical_block {
                insert_pos = i;
                break;
            }
        }

        // 移动后面的 extent 为新 extent 腾出空间
        if insert_pos < entries_count as usize {
            let src_offset = header_size + insert_pos * extent_size;
            let dst_offset = header_size + (insert_pos + 1) * extent_size;
            let move_count = (entries_count as usize - insert_pos) * extent_size;

            unsafe {
                core::ptr::copy(
                    data[src_offset..].as_ptr(),
                    data[dst_offset..].as_mut_ptr(),
                    move_count,
                );
            }
        }

        // 写入新 extent
        let new_extent_offset = header_size + insert_pos * extent_size;
        let new_extent = unsafe {
            &mut *(data[new_extent_offset..].as_mut_ptr() as *mut ext4_extent)
        };

        new_extent.block = logical_block.to_le();
        new_extent.len = (length as u16).to_le();
        new_extent.start_lo = (physical_block as u32).to_le();
        new_extent.start_hi = ((physical_block >> 32) as u16).to_le();

        log::debug!(
            "[EXTENT_INSERT] Writing extent at pos {}: logical={}, physical=0x{:x}, len={}",
            insert_pos, logical_block, physical_block, length
        );

        // 更新 header
        header.entries = (entries_count + 1).to_le();

        log::debug!(
            "[EXTENT_INSERT] Updated header: entries {} -> {}",
            entries_count, entries_count + 1
        );

        Ok(())
    })??;

    Ok(())
}

/// 构建从根到指定叶子块的 ExtentPath
///
/// 用于分裂操作前构建路径信息
fn build_extent_path_for_leaf<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    leaf_block: u64,
) -> Result<ExtentPath> {
    // 读取根节点信息
    let (root_header, max_depth) = inode_ref.with_inode(|inode| {
        let header_ptr = inode.blocks.as_ptr() as *const ext4_extent_header;
        let header = unsafe { &*header_ptr };
        let depth = u16::from_le(header.depth);
        (header.clone(), depth)
    })?;

    let mut path = ExtentPath::new(max_depth);

    // 添加根节点
    path.push(ExtentPathNode {
        block_addr: 0, // 根节点在 inode 中
        depth: max_depth,
        header: root_header,
        index_pos: 0,
        node_type: ExtentNodeType::Root,
    });

    // 如果深度为 0，根节点就是叶子节点
    if max_depth == 0 {
        return Ok(path);
    }

    // 对于深度 > 0，需要添加中间节点和叶子节点
    // 这里我们简化处理：只支持深度 1（一层索引 + 一层叶子）
    if max_depth == 1 {
        // 读取叶子节点 header
        let mut block = Block::get(inode_ref.bdev(), leaf_block)?;
        let leaf_header = block.with_data(|data| {
            let header = unsafe {
                *(data.as_ptr() as *const ext4_extent_header)
            };
            header.clone()
        })?;

        // 添加叶子节点
        path.push(ExtentPathNode {
            block_addr: leaf_block,
            depth: 0, // 叶子节点深度为 0
            header: leaf_header,
            index_pos: 0,
            node_type: ExtentNodeType::Leaf,
        });

        return Ok(path);
    }

    // 对于深度 > 1，需要遍历索引树
    // TODO: 完整实现任意深度支持
    Err(Error::new(
        ErrorKind::Unsupported,
        "build_extent_path_for_leaf: depth > 1 not yet supported",
    ))
}

/// 分裂后确定目标叶子块
///
/// 根据 logical_block，决定应该插入到原叶子还是新分裂的叶子
fn determine_target_leaf_after_split<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    path: &ExtentPath,
    logical_block: u32,
) -> Result<u64> {
    // 对于深度 1 的简单情况，从根节点的索引中找到目标叶子
    let depth = path.nodes[0].header.depth();

    log::debug!(
        "[DETERMINE_TARGET] Starting: depth={}, logical_block={}",
        depth, logical_block
    );

    if depth == 1 {
        // 读取根节点的索引数组
        let (indices, _) = super::split::read_indices_from_inode(inode_ref)?;

        log::debug!(
            "[DETERMINE_TARGET] Read {} indices from inode",
            indices.len()
        );

        // 找到最后一个 first_block <= logical_block 的索引
        let mut target_idx: Option<&ext4_extent_idx> = None;
        for (i, idx) in indices.iter().enumerate() {
            let idx_block = u32::from_le(idx.block);
            let leaf_block = super::helpers::ext4_idx_pblock(idx);

            log::debug!(
                "[DETERMINE_TARGET] Index {}: idx_block={}, leaf_block=0x{:x}",
                i, idx_block, leaf_block
            );

            if logical_block >= idx_block {
                target_idx = Some(idx);
            } else {
                break;
            }
        }

        if let Some(idx) = target_idx {
            // 使用辅助函数而不是手动组合
            let leaf_block = super::helpers::ext4_idx_pblock(idx);

            log::debug!(
                "[DETERMINE_TARGET] Selected target: leaf_block=0x{:x}",
                leaf_block
            );

            return Ok(leaf_block);
        }

        log::error!("[DETERMINE_TARGET] No matching index found!");
        return Err(Error::new(
            ErrorKind::Corrupted,
            "No matching index found after split",
        ));
    }

    // TODO: 支持更深的树
    Err(Error::new(
        ErrorKind::Unsupported,
        "determine_target_leaf_after_split: depth > 1 not yet supported",
    ))
}

/// 插入 extent 到叶子节点（支持任意深度）
///
/// 这个函数遍历 extent 树找到合适的叶子节点，然后插入 extent。
///
/// 注意：这个函数已废弃，请使用 insert_extent_to_leaf_direct
#[allow(dead_code)]
fn insert_extent_to_leaf<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    sb: &mut Superblock,
    allocator: &mut BlockAllocator,
    logical_block: u32,
    physical_block: u64,
    length: u32,
) -> Result<()> {
    // 查找包含 logical_block 的叶子节点
    let (leaf_block, depth) = inode_ref.with_inode(|inode| -> Result<(u64, u16)> {
        let header_ptr = inode.blocks.as_ptr() as *const ext4_extent_header;
        let header = unsafe { &*header_ptr };
        let depth = u16::from_le(header.depth);

        if depth == 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "insert_extent_to_leaf called on depth-0 tree",
            ));
        }

        // 对于深度 > 0，需要遍历索引节点
        // 简化实现：仅支持深度 1
        if depth > 1 {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "insert_extent_to_leaf: depth > 1 not yet fully supported",
            ));
        }

        // 读取第一个索引（深度 1 时通常只有一个索引指向叶子节点）
        let header_size = core::mem::size_of::<ext4_extent_header>();
        let idx_ptr = unsafe {
            // 关键修复：inode.blocks 是 [u32; 15]，需要先转为 *const u8 再按字节偏移
            (inode.blocks.as_ptr() as *const u8).add(header_size) as *const ext4_extent_idx
        };
        let idx = unsafe { &*idx_ptr };

        let leaf_lo = u32::from_le(idx.leaf_lo);
        let leaf_hi = u16::from_le(idx.leaf_hi);
        let leaf_block = (leaf_hi as u64) << 32 | (leaf_lo as u64);

        log::debug!(
            "[EXTENT_LEAF] Read index: leaf_lo=0x{:x}, leaf_hi=0x{:x}, leaf_block=0x{:x}, depth={}",
            leaf_lo, leaf_hi, leaf_block, depth
        );

        Ok((leaf_block, depth))
    })??;

    // 使用统一的 insert_extent_to_leaf_direct（支持分裂）
    insert_extent_to_leaf_direct(
        inode_ref,
        sb,
        allocator,
        leaf_block,
        logical_block,
        physical_block,
        length,
    )
}

/// 简单插入 extent（仅支持深度 0 的树）
///
/// 这是一个简化的 extent 插入实现，仅支持在 inode 的根节点（深度=0）中插入 extent。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `extent` - 要插入的 extent
///
/// # 返回
///
/// 成功返回 ()，失败返回错误
///
/// # 限制
///
/// - 仅支持深度为 0 的 extent 树
/// - 不支持节点分裂
/// - 不支持 extent 合并
pub(crate) fn insert_extent_simple<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    extent: &ext4_extent,
) -> Result<()> {
    inode_ref.with_inode_mut(|inode| {
        // 获取 extent header
        let header_ptr = inode.blocks.as_mut_ptr() as *mut ext4_extent_header;
        let header = unsafe { &mut *header_ptr };

        // 检查深度
        let depth = u16::from_le(header.depth);
        if depth != 0 {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "insert_extent_simple only supports depth=0 trees",
            ));
        }

        // 检查是否有空间
        let entries = u16::from_le(header.entries);
        let max_entries = u16::from_le(header.max);

        if entries >= max_entries {
            return Err(Error::new(
                ErrorKind::NoSpace,
                "Extent root node is full (split not yet implemented)",
            ));
        }

        // 计算插入位置
        let header_size = core::mem::size_of::<ext4_extent_header>();
        let extent_size = core::mem::size_of::<ext4_extent>();
        let new_block = u32::from_le(extent.block);

        // 找到正确的插入位置（保持逻辑块号升序）
        let mut insert_pos = entries as usize;
        for i in 0..entries as usize {
            let offset = header_size + i * extent_size;
            let existing_extent = unsafe {
                *(inode.blocks.as_ptr().add(offset / 4) as *const ext4_extent)
            };
            let existing_block = u32::from_le(existing_extent.block);

            if new_block < existing_block {
                insert_pos = i;
                break;
            }
        }

        // 如果需要，移动后面的 extent 腾出空间
        if insert_pos < entries as usize {
            let src_offset = header_size + insert_pos * extent_size;
            let dst_offset = src_offset + extent_size;
            let move_count = (entries as usize - insert_pos) * extent_size;

            unsafe {
                let src = inode.blocks.as_ptr().add(src_offset / 4) as *const u8;
                let dst = inode.blocks.as_mut_ptr().add(dst_offset / 4) as *mut u8;
                core::ptr::copy(src, dst, move_count);
            }
        }

        // 插入新 extent
        let insert_offset = header_size + insert_pos * extent_size;
        unsafe {
            let dst = inode.blocks.as_mut_ptr().add(insert_offset / 4) as *mut ext4_extent;
            core::ptr::write(dst, *extent);
        }

        // 更新 entries 计数
        header.entries = (entries + 1).to_le();

        Ok(())
    })?;

    // 标记 inode 为脏
    inode_ref.mark_dirty();

    Ok(())
}

/// 查找包含指定逻辑块的 extent
///
/// 内部辅助函数，用于在 extent 树中查找包含指定逻辑块的 extent
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `logical_block` - 要查找的逻辑块号
///
/// # 返回
///
/// * `Some(extent)` - 找到包含此逻辑块的 extent
/// * `None` - 未找到
fn find_extent_for_block<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    logical_block: u32,
) -> Result<Option<ext4_extent>> {
    // 读取 inode 中的 extent 树根节点
    let root_data = inode_ref.with_inode(|inode| {
        let root_data = unsafe {
            core::slice::from_raw_parts(
                inode.blocks.as_ptr() as *const u8,
                60, // 15 * 4
            ).to_vec()
        };
        root_data
    })?;

    // 解析根节点 header
    let header = unsafe {
        *(root_data.as_ptr() as *const ext4_extent_header)
    };

    let depth = u16::from_le(header.depth);

    // 根据深度选择查找方式
    if depth == 0 {
        // 叶子节点：直接在根节点中查找
        return find_extent_in_leaf(&root_data, logical_block);
    }

    // 多层树：需要遍历索引节点
    find_extent_in_multilevel_tree(inode_ref, &root_data, &header, logical_block)
}

/// 在多层 extent 树中查找 extent
///
/// 递归遍历索引节点，直到找到包含目标逻辑块的叶子节点
fn find_extent_in_multilevel_tree<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    node_data: &[u8],
    header: &ext4_extent_header,
    logical_block: u32,
) -> Result<Option<ext4_extent>> {
    // 如果已经是叶子节点，直接查找
    if header.is_leaf() {
        return find_extent_in_leaf(node_data, logical_block);
    }

    // 索引节点：查找指向目标块的索引
    let entries = u16::from_le(header.entries);
    let header_size = core::mem::size_of::<ext4_extent_header>();
    let idx_size = core::mem::size_of::<ext4_extent_idx>();

    let mut target_idx: Option<ext4_extent_idx> = None;

    for i in 0..entries as usize {
        let offset = header_size + i * idx_size;
        if offset + idx_size > node_data.len() {
            return Err(Error::new(
                ErrorKind::Corrupted,
                "Extent index node data too short",
            ));
        }

        let idx = unsafe {
            core::ptr::read_unaligned(
                node_data[offset..].as_ptr() as *const ext4_extent_idx
            )
        };

        let idx_block = u32::from_le(idx.block);

        // 找到最后一个 logical_block >= idx.block 的索引
        if logical_block >= idx_block {
            target_idx = Some(idx);
        } else {
            break;
        }
    }

    if let Some(idx) = target_idx {
        // 读取子节点
        let child_block = {
            let leaf_lo = u32::from_le(idx.leaf_lo);
            let leaf_hi = u16::from_le(idx.leaf_hi);
            (leaf_hi as u64) << 32 | (leaf_lo as u64)
        };

        let mut block = Block::get(inode_ref.bdev(), child_block)?;

        // 复制子节点数据
        let child_data = block.with_data(|data| {
            let mut buf = Vec::with_capacity(data.len());
            buf.extend_from_slice(data);
            buf
        })?;

        drop(block);

        // 解析子节点 header
        let child_header = unsafe {
            *(child_data.as_ptr() as *const ext4_extent_header)
        };

        if !child_header.is_valid() {
            return Err(Error::new(
                ErrorKind::Corrupted,
                "Invalid extent header in child node",
            ));
        }

        // 递归查找
        find_extent_in_multilevel_tree(inode_ref, &child_data, &child_header, logical_block)
    } else {
        Ok(None)
    }
}

/// 在叶子节点中查找 extent
fn find_extent_in_leaf(node_data: &[u8], logical_block: u32) -> Result<Option<ext4_extent>> {
    let header = unsafe { *(node_data.as_ptr() as *const ext4_extent_header) };
    let entries = u16::from_le(header.entries);

    let header_size = core::mem::size_of::<ext4_extent_header>();
    let extent_size = core::mem::size_of::<ext4_extent>();

    for i in 0..entries as usize {
        let offset = header_size + i * extent_size;
        if offset + extent_size > node_data.len() {
            break;
        }

        let extent = unsafe {
            *(node_data.as_ptr().add(offset) as *const ext4_extent)
        };

        let ee_block = u32::from_le(extent.block);
        let ee_len = u16::from_le(extent.len);

        // 检查逻辑块是否在这个 extent 范围内
        if logical_block >= ee_block && logical_block < ee_block + ee_len as u32 {
            return Ok(Some(extent));
        }
    }

    Ok(None)
}

/// Extent 路径节点
///
/// 表示从根到叶子的路径上的一个节点
///
/// 对应 lwext4 的 `struct ext4_extent_path`
#[derive(Debug)]
pub struct ExtentPathNode {
    /// 节点所在的物理块地址
    pub block_addr: u64,

    /// 节点深度（0 = 叶子）
    pub depth: u16,

    /// Extent header
    pub header: ext4_extent_header,

    /// 当前索引位置（在索引节点中）
    pub index_pos: usize,

    /// 节点类型
    pub node_type: ExtentNodeType,
}

/// Extent 节点类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtentNodeType {
    /// 根节点（在 inode 中）
    Root,

    /// 索引节点
    Index,

    /// 叶子节点
    Leaf,
}

/// Extent 路径
///
/// 表示从 inode 根节点到目标 extent 的完整路径
///
/// 对应 lwext4 的 `struct ext4_extent_path` 数组
#[derive(Debug)]
pub struct ExtentPath {
    /// 路径上的所有节点（从根到叶）
    pub nodes: Vec<ExtentPathNode>,

    /// 最大深度
    pub max_depth: u16,
}

impl ExtentPath {
    /// 创建新的 extent 路径
    pub fn new(max_depth: u16) -> Self {
        Self {
            nodes: Vec::with_capacity(max_depth as usize + 1),
            max_depth,
        }
    }

    /// 获取当前深度
    pub fn depth(&self) -> u16 {
        if self.nodes.is_empty() {
            0
        } else {
            self.nodes.len() as u16 - 1
        }
    }

    /// 获取叶子节点
    pub fn leaf(&self) -> Option<&ExtentPathNode> {
        self.nodes.last()
    }

    /// 获取叶子节点（可变）
    pub fn leaf_mut(&mut self) -> Option<&mut ExtentPathNode> {
        self.nodes.last_mut()
    }

    /// 添加节点到路径
    pub fn push(&mut self, node: ExtentPathNode) {
        self.nodes.push(node);
    }
}

/// Extent 写操作器
///
/// 提供 extent 树的修改操作
pub struct ExtentWriter<'a, D: BlockDevice> {
    trans: &'a mut SimpleTransaction<'a, D>,
    block_size: u32,
}

impl<'a, D: BlockDevice> ExtentWriter<'a, D> {
    /// 创建新的 extent 写操作器
    pub fn new(trans: &'a mut SimpleTransaction<'a, D>, block_size: u32) -> Self {
        Self { trans, block_size }
    }

    /// 查找 extent 路径
    ///
    /// 从 inode 根节点开始，查找到包含指定逻辑块的叶子节点的路径
    ///
    /// 对应 lwext4 的 `ext4_find_extent`
    ///
    /// # 参数
    ///
    /// * `inode_ref` - Inode 引用
    /// * `logical_block` - 目标逻辑块号
    ///
    /// # 返回
    ///
    /// Extent 路径
    pub fn find_extent_path(
        &mut self,
        inode_ref: &mut InodeRef<D>,
        logical_block: u32,
    ) -> Result<ExtentPath> {
        // 读取 inode 中的 extent 根节点
        let root_data = inode_ref.with_inode(|inode| {
            let root_data = unsafe {
                core::slice::from_raw_parts(
                    inode.blocks.as_ptr() as *const u8,
                    60, // 15 * 4 = 60 bytes
                )
            };
            let mut buf = alloc::vec![0u8; 60];
            buf.copy_from_slice(root_data);
            buf
        })?;

        // 解析根节点 header
        let root_header = unsafe {
            core::ptr::read_unaligned(root_data.as_ptr() as *const ext4_extent_header)
        };

        if !root_header.is_valid() {
            return Err(Error::new(
                ErrorKind::Corrupted,
                "Invalid extent header in inode",
            ));
        }

        let max_depth = root_header.depth();
        let mut path = ExtentPath::new(max_depth);

        // 添加根节点到路径
        path.push(ExtentPathNode {
            block_addr: 0, // 根节点在 inode 中，没有独立块地址
            depth: max_depth,
            header: root_header,
            index_pos: 0,
            node_type: ExtentNodeType::Root,
        });

        // 如果根节点就是叶子，直接返回
        if root_header.is_leaf() {
            return Ok(path);
        }

        // 递归查找路径
        let mut current_data = root_data;
        let mut current_depth = max_depth;

        while current_depth > 0 {
            // 在当前索引节点中查找
            let next_block = self.find_index_in_node(&current_data, logical_block)?;

            // 读取子节点
            let mut child_block = self.trans.get_block(next_block)?;
            current_data = child_block.with_data(|data| {
                let mut buf = alloc::vec![0u8; data.len()];
                buf.copy_from_slice(data);
                buf
            })?;

            drop(child_block);

            // 解析子节点 header
            let child_header = unsafe {
                core::ptr::read_unaligned(current_data.as_ptr() as *const ext4_extent_header)
            };

            if !child_header.is_valid() {
                return Err(Error::new(
                    ErrorKind::Corrupted,
                    "Invalid extent header in child node",
                ));
            }

            current_depth -= 1;

            let node_type = if child_header.is_leaf() {
                ExtentNodeType::Leaf
            } else {
                ExtentNodeType::Index
            };

            // 添加到路径
            path.push(ExtentPathNode {
                block_addr: next_block,
                depth: current_depth,
                header: child_header,
                index_pos: 0,
                node_type,
            });

            if child_header.is_leaf() {
                break;
            }
        }

        Ok(path)
    }

    /// 在索引节点中查找目标块
    fn find_index_in_node(&self, node_data: &[u8], logical_block: u32) -> Result<u64> {
        let header = unsafe {
            core::ptr::read_unaligned(node_data.as_ptr() as *const ext4_extent_header)
        };

        let entries = header.entries_count() as usize;
        let header_size = core::mem::size_of::<ext4_extent_header>();
        let idx_size = core::mem::size_of::<ext4_extent_idx>();

        // 找到最后一个 logical_block >= idx.first_block 的索引
        let mut target_idx: Option<ext4_extent_idx> = None;

        for i in 0..entries {
            let offset = header_size + i * idx_size;
            if offset + idx_size > node_data.len() {
                return Err(Error::new(
                    ErrorKind::Corrupted,
                    "Extent index node data too short",
                ));
            }

            let idx = unsafe {
                core::ptr::read_unaligned(
                    node_data[offset..].as_ptr() as *const ext4_extent_idx
                )
            };

            let idx_block = idx.logical_block();

            if logical_block >= idx_block {
                target_idx = Some(idx);
            } else {
                break;
            }
        }

        if let Some(idx) = target_idx {
            Ok(idx.leaf_block())
        } else {
            Err(Error::new(
                ErrorKind::NotFound,
                "No matching index found",
            ))
        }
    }

    /// 插入新的 extent（支持自动分裂）
    ///
    /// 对应 lwext4 的 `ext4_ext_insert_extent`
    ///
    /// # 参数
    ///
    /// * `inode_ref` - Inode 引用
    /// * `sb` - Superblock 引用
    /// * `allocator` - 块分配器
    /// * `logical_block` - 逻辑块起始位置
    /// * `physical_block` - 物理块起始位置
    /// * `length` - extent 长度（块数）
    ///
    /// # 返回
    ///
    /// 成功返回 Ok(())
    ///
    /// # 功能
    ///
    /// 此函数会：
    /// 1. 查找插入位置
    /// 2. 如果节点满，自动进行分裂或增加树深度
    /// 3. 检查是否可以与现有 extent 合并（TODO）
    /// 4. 插入新 extent
    ///
    /// # 注意
    ///
    /// ⚠️ **当前限制**：
    /// - 仅支持根节点和第一层叶子节点
    /// - 不支持深度 > 1 的树
    /// - 不支持 extent 合并优化
    pub fn insert_extent(
        &mut self,
        inode_ref: &mut InodeRef<D>,
        sb: &mut crate::superblock::Superblock,
        allocator: &mut crate::balloc::BlockAllocator,
        logical_block: u32,
        physical_block: u64,
        length: u32,
    ) -> Result<()> {
        // 1. 查找路径到应该包含此 extent 的叶子节点
        let mut path = self.find_extent_path(inode_ref, logical_block)?;

        // 2. 获取叶子节点
        let leaf = path.leaf().ok_or_else(|| {
            Error::new(ErrorKind::Corrupted, "Extent path has no leaf node")
        })?;

        // 检查节点是否有空间
        let entries_count = leaf.header.entries_count();
        let max_entries = leaf.header.max_entries();

        if entries_count >= max_entries {
            // 节点满了，需要分裂
            if leaf.node_type == ExtentNodeType::Root {
                // 根节点满了，需要增加树深度
                self.grow_tree_depth(inode_ref, sb, allocator)?;

                // 重新查找路径（树结构已改变）
                path = self.find_extent_path(inode_ref, logical_block)?;
            } else {
                // 叶子节点满了，分裂它
                let leaf_at = path.nodes.len() - 1;
                self.split_extent_node(
                    inode_ref,
                    sb,
                    allocator,
                    &mut path,
                    leaf_at,
                    logical_block,
                )?;

                // 重新查找路径（树结构已改变）
                path = self.find_extent_path(inode_ref, logical_block)?;
            }
        }

        // 3. 尝试与现有 extent 合并（简化版本）
        // TODO: 实现完整的合并逻辑

        // 4. 重新获取叶子节点（可能已改变）
        let leaf = path.leaf().ok_or_else(|| {
            Error::new(ErrorKind::Corrupted, "Extent path has no leaf node after split")
        })?;

        // 5. 在 inode 或块中插入新 extent
        if leaf.node_type == ExtentNodeType::Root {
            // 插入到 inode 的 extent 根节点
            self.insert_extent_to_inode(inode_ref, logical_block, physical_block, length)?;
        } else {
            // 插入到独立的 extent 块
            self.insert_extent_to_block(
                leaf.block_addr,
                logical_block,
                physical_block,
                length,
            )?;
        }

        Ok(())
    }

    /// 插入 extent 到 inode 中的根节点
    fn insert_extent_to_inode(
        &mut self,
        inode_ref: &mut InodeRef<D>,
        logical_block: u32,
        physical_block: u64,
        length: u32,
    ) -> Result<()> {
        inode_ref.with_inode_mut(|inode| {
            // inode.blocks 中前 60 字节是 extent 根节点
            let extent_data = unsafe {
                core::slice::from_raw_parts_mut(
                    inode.blocks.as_mut_ptr() as *mut u8,
                    60,
                )
            };

            // 解析 header
            let header = unsafe {
                &mut *(extent_data.as_mut_ptr() as *mut ext4_extent_header)
            };

            if !header.is_valid() {
                return Err(Error::new(
                    ErrorKind::Corrupted,
                    "Invalid extent header in inode",
                ));
            }

            let entries_count = header.entries_count();
            let max_entries = header.max_entries();

            if entries_count >= max_entries {
                return Err(Error::new(
                    ErrorKind::NoSpace,
                    "Inode extent root is full",
                ));
            }

            // 计算插入位置
            let header_size = core::mem::size_of::<ext4_extent_header>();
            let extent_size = core::mem::size_of::<ext4_extent>();

            // 找到插入位置（保持 extent 按逻辑块号排序）
            let mut insert_pos = entries_count as usize;
            for i in 0..entries_count as usize {
                let offset = header_size + i * extent_size;
                let existing_extent = unsafe {
                    &*(extent_data[offset..].as_ptr() as *const ext4_extent)
                };

                if existing_extent.logical_block() > logical_block {
                    insert_pos = i;
                    break;
                }
            }

            // 如果需要，移动后面的 extent 为新 extent 腾出空间
            if insert_pos < entries_count as usize {
                let src_offset = header_size + insert_pos * extent_size;
                let dst_offset = header_size + (insert_pos + 1) * extent_size;
                let move_count = (entries_count as usize - insert_pos) * extent_size;

                unsafe {
                    core::ptr::copy(
                        extent_data[src_offset..].as_ptr(),
                        extent_data[dst_offset..].as_mut_ptr(),
                        move_count,
                    );
                }
            }

            // 写入新 extent
            let new_extent_offset = header_size + insert_pos * extent_size;
            let new_extent = unsafe {
                &mut *(extent_data[new_extent_offset..].as_mut_ptr() as *mut ext4_extent)
            };

            new_extent.block = logical_block.to_le();
            new_extent.len = (length as u16).to_le();
            new_extent.start_lo = (physical_block as u32).to_le();
            new_extent.start_hi = ((physical_block >> 32) as u16).to_le();

            // 更新 header 中的 entry 计数
            header.entries = (entries_count + 1).to_le();

            Ok(())
        })?
    }

    /// 插入 extent 到独立的 extent 块
    fn insert_extent_to_block(
        &mut self,
        block_addr: u64,
        logical_block: u32,
        physical_block: u64,
        length: u32,
    ) -> Result<()> {
        {
            let mut block = self.trans.get_block(block_addr)?;

            block.with_data_mut(|data| {
                // 解析 header
                let header = unsafe {
                    &mut *(data.as_mut_ptr() as *mut ext4_extent_header)
                };

                if !header.is_valid() {
                    return Err(Error::new(
                        ErrorKind::Corrupted,
                        "Invalid extent header in block",
                    ));
                }

                let entries_count = header.entries_count();
                let max_entries = header.max_entries();

                if entries_count >= max_entries {
                    return Err(Error::new(
                        ErrorKind::NoSpace,
                        "Extent block is full",
                    ));
                }

                // 计算插入位置
                let header_size = core::mem::size_of::<ext4_extent_header>();
                let extent_size = core::mem::size_of::<ext4_extent>();

                // 找到插入位置（保持 extent 按逻辑块号排序）
                let mut insert_pos = entries_count as usize;
                for i in 0..entries_count as usize {
                    let offset = header_size + i * extent_size;
                    let existing_extent = unsafe {
                        &*(data[offset..].as_ptr() as *const ext4_extent)
                    };

                    if existing_extent.logical_block() > logical_block {
                        insert_pos = i;
                        break;
                    }
                }

                // 如果需要，移动后面的 extent 为新 extent 腾出空间
                if insert_pos < entries_count as usize {
                    let src_offset = header_size + insert_pos * extent_size;
                    let dst_offset = header_size + (insert_pos + 1) * extent_size;
                    let move_count = (entries_count as usize - insert_pos) * extent_size;

                    unsafe {
                        core::ptr::copy(
                            data[src_offset..].as_ptr(),
                            data[dst_offset..].as_mut_ptr(),
                            move_count,
                        );
                    }
                }

                // 写入新 extent
                let new_extent_offset = header_size + insert_pos * extent_size;
                let new_extent = unsafe {
                    &mut *(data[new_extent_offset..].as_mut_ptr() as *mut ext4_extent)
                };

                new_extent.block = logical_block.to_le();
                new_extent.len = (length as u16).to_le();
                new_extent.start_lo = (physical_block as u32).to_le();
                new_extent.start_hi = ((physical_block >> 32) as u16).to_le();

                // 更新 header 中的 entry 计数
                header.entries = (entries_count + 1).to_le();

                Ok(())
            })??;
        } // block 在这里被 drop，释放借用

        // 标记块为脏
        self.trans.mark_dirty(block_addr)?;

        Ok(())
    }

    // ========================================================================
    // 节点分裂操作（占位实现）
    // ========================================================================

    /// 分裂 extent 节点
    ///
    /// 对应 lwext4 的 `ext4_ext_split()`
    ///
    /// 当节点满时，将其分裂成两个节点：
    /// 1. 分配新的 extent 块
    /// 2. 将当前节点的一半条目移动到新节点
    /// 3. 在父节点中插入新的索引条目
    ///
    /// # 参数
    ///
    /// * `inode_ref` - Inode 引用
    /// * `sb` - Superblock 引用
    /// * `allocator` - 块分配器
    /// * `path` - Extent 路径（包含需要分裂的节点）
    /// * `at` - 需要分裂的节点在路径中的索引
    /// * `logical_block` - 触发分裂的逻辑块号
    ///
    /// # 返回
    ///
    /// 成功返回 `Ok(())`
    ///
    /// # 注意
    ///
    /// ⚠️ **当前限制**：
    /// - 不支持递归分裂（当父节点也满时）
    /// - 不支持根节点分裂（需要先调用 grow_tree_depth）
    pub fn split_extent_node(
        &mut self,
        inode_ref: &mut InodeRef<D>,
        sb: &mut crate::superblock::Superblock,
        allocator: &mut crate::balloc::BlockAllocator,
        path: &mut ExtentPath,
        at: usize,
        logical_block: u32,
    ) -> Result<()> {
        crate::extent::split_extent_node(
            inode_ref,
            sb,
            allocator,
            path,
            at,
            logical_block,
        )
    }

    /// 合并相邻的 extent（占位实现）
    ///
    /// ⚠️ **尚未实现** - 总是返回 `Unsupported` 错误
    ///
    /// 对应 lwext4 的 `ext4_ext_try_to_merge()`
    ///
    /// # 未来实现需求
    ///
    /// Extent 合并需要检查：
    /// 1. 两个 extent 在逻辑上是否连续
    /// 2. 两个 extent 在物理上是否连续
    /// 3. 合并后的长度是否超过最大值（32768 块）
    /// 4. 两个 extent 的初始化状态是否相同
    ///
    /// # 参数
    ///
    /// * `path` - Extent 路径
    /// * `new_extent` - 新插入的 extent
    ///
    /// # 返回
    ///
    /// `Err(Unsupported)` - 功能未实现
    pub fn try_merge_extent(
        &mut self,
        _path: &mut ExtentPath,
        _new_extent: &ext4_extent,
    ) -> Result<bool> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "Extent merging not yet implemented",
        ))
    }

    /// 增加 extent 树的深度
    ///
    /// 对应 lwext4 的 `ext4_ext_grow_indepth()`
    ///
    /// 当根节点需要分裂时，增加树的深度：
    /// 1. 分配新的 extent 块
    /// 2. 将当前根节点的内容复制到新分配的块
    /// 3. 在 inode 中创建新的索引根节点，指向新分配的块
    /// 4. 树深度加 1
    ///
    /// # 参数
    ///
    /// * `inode_ref` - Inode 引用
    /// * `sb` - Superblock 引用
    /// * `allocator` - 块分配器
    ///
    /// # 返回
    ///
    /// 成功返回新分配的块地址
    ///
    /// # 示例
    ///
    /// ```text
    /// Before (depth=0):
    /// Root (in inode)
    ///   [E1, E2, E3, E4]
    ///
    /// After (depth=1):
    /// Root (in inode)
    ///   [Index -> Block 1000]
    ///           ↓
    ///      Block 1000
    ///        [E1, E2, E3, E4]
    /// ```
    pub fn grow_tree_depth(
        &mut self,
        inode_ref: &mut InodeRef<D>,
        sb: &mut crate::superblock::Superblock,
        allocator: &mut crate::balloc::BlockAllocator,
    ) -> Result<u64> {
        crate::extent::grow_tree_depth(inode_ref, sb, allocator)
    }
}

//=============================================================================
// Extent 空间移除（删除/截断）
//=============================================================================

/// 移除 extent 空间（删除/截断文件）
///
/// 对应 lwext4 的 `ext4_extent_remove_space()`
///
/// 删除指定范围内的所有 extent，释放对应的物理块。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `sb` - Superblock 引用
/// * `from` - 起始逻辑块号
/// * `to` - 结束逻辑块号（包含）
///
/// # 返回
///
/// 成功返回 ()，失败返回错误
///
/// # 实现状态
///
/// - ✅ 支持深度 0 的 extent 树
/// - ✅ 完全删除 extent
/// - ✅ 部分删除 extent（截断开头或结尾）
/// - ✅ 分裂 extent（删除中间部分）
/// - ⏳ 多层 extent 树（待完善）
///
/// # 示例
///
/// ```rust,ignore
/// // 删除逻辑块 10-19（共 10 个块）
/// remove_space(&mut inode_ref, &mut sb, 10, 19)?;
///
/// // 截断文件到 100 个块
/// remove_space(&mut inode_ref, &mut sb, 100, u32::MAX)?;
/// ```
pub fn remove_space<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    sb: &mut Superblock,
    from: u32,
    to: u32,
) -> Result<()> {
    // 读取 extent 树深度
    let depth = inode_ref.with_inode(|inode| {
        let header_ptr = inode.blocks.as_ptr() as *const ext4_extent_header;
        let header = unsafe { &*header_ptr };
        u16::from_le(header.depth)
    })?;

    // 使用多层树支持的实现
    // 注意：这需要一个 allocator，但当前签名没有
    // 为了保持向后兼容，我们创建一个临时的 allocator
    let mut allocator = balloc::BlockAllocator::new();

    if depth == 0 {
        // 深度 0 使用优化的简化版本
        remove_space_simple(inode_ref, sb, from, to)?;
    } else {
        // 多层树使用完整实现
        crate::extent::remove_space_multilevel(
            inode_ref,
            sb,
            &mut allocator,
            from,
            to,
        )?;
    }

    Ok(())
}

/// 简单的空间移除（仅支持深度 0）
///
/// 内部辅助函数，处理深度为 0 的 extent 树的空间移除。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `sb` - Superblock 引用
/// * `from` - 起始逻辑块号
/// * `to` - 结束逻辑块号
///
/// # 实现逻辑
///
/// 1. 遍历所有 extent
/// 2. 对于每个与删除范围重叠的 extent：
///    - 如果完全在范围内：删除整个 extent
///    - 如果部分重叠：截断 extent
///    - 如果删除范围在 extent 中间：分裂 extent
/// 3. 释放对应的物理块
/// 4. 更新 extent 数组
fn remove_space_simple<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    sb: &mut Superblock,
    from: u32,
    to: u32,
) -> Result<()> {
    // 收集需要删除/修改的 extent 信息
    let modifications = inode_ref.with_inode(|inode| {
        let mut mods = Vec::new();
        let header_ptr = inode.blocks.as_ptr() as *const ext4_extent_header;
        let header = unsafe { &*header_ptr };
        let entries = u16::from_le(header.entries);

        let header_size = core::mem::size_of::<ext4_extent_header>();
        let extent_size = core::mem::size_of::<ext4_extent>();

        // 遍历所有 extent，找出需要修改的
        for i in 0..entries as usize {
            let offset = header_size + i * extent_size;
            let extent = unsafe {
                *(inode.blocks.as_ptr().add(offset / 4) as *const ext4_extent)
            };

            let ee_block = u32::from_le(extent.block);
            let ee_len = u16::from_le(extent.len);
            let ee_end = ee_block + ee_len as u32 - 1;

            // 检查是否与删除范围重叠
            if ee_end < from || ee_block > to {
                // 不重叠，保留
                continue;
            }

            let ee_start_lo = u32::from_le(extent.start_lo);
            let ee_start_hi = u16::from_le(extent.start_hi);
            let ee_start = (ee_start_hi as u64) << 32 | (ee_start_lo as u64);

            mods.push(ExtentModification {
                index: i,
                ee_block,
                ee_len: ee_len as u32,
                ee_start,
            });
        }

        mods
    })?;

    // 应用修改（从后往前，避免索引问题）
    for modification in modifications.iter().rev() {
        apply_extent_removal(
            inode_ref,
            sb,
            modification.index,
            modification.ee_block,
            modification.ee_len,
            modification.ee_start,
            from,
            to,
        )?;
    }

    Ok(())
}

/// Extent 修改信息
struct ExtentModification {
    index: usize,
    ee_block: u32,
    ee_len: u32,
    ee_start: u64,
}

/// 应用 extent 移除
///
/// 根据删除范围，修改或删除指定的 extent，并释放对应的物理块。
///
/// # 参数
///
/// * `inode_ref` - Inode 引用
/// * `sb` - Superblock 引用
/// * `extent_idx` - Extent 在数组中的索引
/// * `ee_block` - Extent 的起始逻辑块
/// * `ee_len` - Extent 的长度
/// * `ee_start` - Extent 的起始物理块
/// * `from` - 删除范围的起始逻辑块
/// * `to` - 删除范围的结束逻辑块
fn apply_extent_removal<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    sb: &mut Superblock,
    extent_idx: usize,
    ee_block: u32,
    ee_len: u32,
    ee_start: u64,
    from: u32,
    to: u32,
) -> Result<()> {
    let ee_end = ee_block + ee_len - 1;

    // 情况 1: 删除范围完全包含 extent
    if from <= ee_block && to >= ee_end {
        // 删除整个 extent
        // 1. 释放物理块
        balloc::free_blocks(inode_ref.bdev(), sb, ee_start, ee_len)?;

        // 2. 从数组中移除 extent
        remove_extent_at_index(inode_ref, extent_idx)?;
    }
    // 情况 2: 删除范围在 extent 开头
    else if from <= ee_block && to < ee_end && to >= ee_block {
        // 截断开头
        let removed_len = (to - ee_block + 1) as u32;
        let new_len = ee_len - removed_len;
        let new_block = to + 1;
        let new_start = ee_start + removed_len as u64;

        // 1. 释放被删除的块
        balloc::free_blocks(inode_ref.bdev(), sb, ee_start, removed_len)?;

        // 2. 更新 extent
        update_extent_at_index(inode_ref, extent_idx, new_block, new_len, new_start)?;
    }
    // 情况 3: 删除范围在 extent 结尾
    else if from > ee_block && to >= ee_end && from <= ee_end {
        // 截断结尾
        let removed_len = (ee_end - from + 1) as u32;
        let new_len = ee_len - removed_len;
        let removed_start = ee_start + (from - ee_block) as u64;

        // 1. 释放被删除的块
        balloc::free_blocks(inode_ref.bdev(), sb, removed_start, removed_len)?;

        // 2. 更新 extent
        update_extent_at_index(inode_ref, extent_idx, ee_block, new_len, ee_start)?;
    }
    // 情况 4: 删除范围在 extent 中间（需要分裂）
    else if from > ee_block && to < ee_end {
        // 分裂成两个 extent
        let left_len = (from - ee_block) as u32;
        let middle_len = (to - from + 1) as u32;
        let right_len = (ee_end - to) as u32;

        let middle_start = ee_start + left_len as u64;
        let right_block = to + 1;
        let right_start = ee_start + (left_len + middle_len) as u64;

        // 1. 释放中间的块
        balloc::free_blocks(inode_ref.bdev(), sb, middle_start, middle_len)?;

        // 2. 更新左边的 extent
        update_extent_at_index(inode_ref, extent_idx, ee_block, left_len, ee_start)?;

        // 3. 插入右边的新 extent
        let right_extent = ext4_extent {
            block: right_block.to_le(),
            len: (right_len as u16).to_le(),
            start_hi: ((right_start >> 32) as u16).to_le(),
            start_lo: (right_start as u32).to_le(),
        };

        insert_extent_simple(inode_ref, &right_extent)?;
    }

    Ok(())
}

/// 移除指定索引处的 extent
///
/// 从 inode 的 extent 数组中移除指定索引的 extent，
/// 并将后续 extent 前移。
fn remove_extent_at_index<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    index: usize,
) -> Result<()> {
    inode_ref.with_inode_mut(|inode| {
        let header_ptr = inode.blocks.as_mut_ptr() as *mut ext4_extent_header;
        let header = unsafe { &mut *header_ptr };

        let entries = u16::from_le(header.entries);
        if index >= entries as usize {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid extent index in remove",
            ));
        }

        let header_size = core::mem::size_of::<ext4_extent_header>();
        let extent_size = core::mem::size_of::<ext4_extent>();

        // 移动后续 extent
        if index < entries as usize - 1 {
            let src_offset = header_size + (index + 1) * extent_size;
            let dst_offset = header_size + index * extent_size;
            let move_count = (entries as usize - index - 1) * extent_size;

            unsafe {
                let src = inode.blocks.as_ptr().add(src_offset / 4) as *const u8;
                let dst = inode.blocks.as_mut_ptr().add(dst_offset / 4) as *mut u8;
                core::ptr::copy(src, dst, move_count);
            }
        }

        // 更新 entries 计数
        header.entries = (entries - 1).to_le();

        Ok(())
    })?;

    inode_ref.mark_dirty();
    Ok(())
}

/// 更新指定索引处的 extent
///
/// 修改 inode extent 数组中指定索引的 extent 的值。
fn update_extent_at_index<D: BlockDevice>(
    inode_ref: &mut InodeRef<D>,
    index: usize,
    new_block: u32,
    new_len: u32,
    new_start: u64,
) -> Result<()> {
    inode_ref.with_inode_mut(|inode| {
        let header_ptr = inode.blocks.as_ptr() as *const ext4_extent_header;
        let header = unsafe { &*header_ptr };

        let entries = u16::from_le(header.entries);
        if index >= entries as usize {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid extent index in update",
            ));
        }

        let header_size = core::mem::size_of::<ext4_extent_header>();
        let extent_size = core::mem::size_of::<ext4_extent>();
        let offset = header_size + index * extent_size;

        let new_extent = ext4_extent {
            block: new_block.to_le(),
            len: (new_len as u16).to_le(),
            start_hi: ((new_start >> 32) as u16).to_le(),
            start_lo: (new_start as u32).to_le(),
        };

        unsafe {
            let dst = inode.blocks.as_mut_ptr().add(offset / 4) as *mut ext4_extent;
            core::ptr::write(dst, new_extent);
        }

        Ok(())
    })?;

    inode_ref.mark_dirty();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extent_path_creation() {
        let path = ExtentPath::new(2);
        assert_eq!(path.max_depth, 2);
        assert_eq!(path.depth(), 0);
    }

    #[test]
    fn test_extent_node_type() {
        let node_type = ExtentNodeType::Leaf;
        assert_eq!(node_type, ExtentNodeType::Leaf);
        assert_ne!(node_type, ExtentNodeType::Index);
    }
}
