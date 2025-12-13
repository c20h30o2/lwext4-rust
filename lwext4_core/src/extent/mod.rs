//! Extent 树操作模块
//!
//! 这个模块提供 ext4 extent 树的解析和块映射功能。
//!
//! Extent 是现代 ext4 文件系统中用于表示文件数据块位置的机制，
//! 相比传统的间接块方式更高效。
//!
//! ## 子模块
//!
//! - `tree` - Extent 树读取操作（✅ 完全实现）
//! - `write` - Extent 树写入操作（✅ 核心功能完整）
//!
//! ## 主要功能
//!
//! ### 读取操作
//! - `find_extent()` - 查找逻辑块对应的 extent
//! - extent 树遍历和解析
//!
//! ### 写入操作
//! - `tree_init()` - 初始化 extent 树
//! - `get_blocks()` - 获取/分配物理块（支持自动分配）
//! - `remove_space()` - 删除/截断文件（释放物理块）
//! - `ExtentWriter` - 高级 extent 写入器（支持节点分裂）
//!
//! ## 实现状态
//!
//! - ✅ 小文件支持（深度 0 的 extent 树）
//! - ✅ 文件创建、写入、截断、删除
//! - ✅ 块分配和回收
//! - ⚠️ 大文件支持（多层树需要使用 ExtentWriter）

mod tree;
mod write;

pub use tree::*;
pub use write::{
    get_blocks, remove_space, tree_init, ExtentPath, ExtentPathNode, ExtentNodeType,
    ExtentWriter,
};
