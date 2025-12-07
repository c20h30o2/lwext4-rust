//! Extent 树操作模块
//!
//! 这个模块提供 ext4 extent 树的解析和块映射功能。
//!
//! Extent 是现代 ext4 文件系统中用于表示文件数据块位置的机制，
//! 相比传统的间接块方式更高效。

mod tree;

pub use tree::*;
