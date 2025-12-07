//! Inode 操作模块
//!
//! 这个模块提供 ext4 inode 的读取、验证和操作功能。

mod read;

pub use read::*;
