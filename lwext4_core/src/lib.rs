//! lwext4-core: Pure Rust implementation of ext4 filesystem
//!
//! This crate provides a minimal ext4 implementation compatible with lwext4 API.

#![no_std]
#![allow(dead_code)]

extern crate alloc;

// 公共模块
pub mod consts;
pub mod types;
pub mod error;
pub mod traits;
pub mod superblock;
pub mod inode;
pub mod block;
pub mod dir;
pub mod fs;

// 重新导出常用类型
pub use consts::*;
pub use error::{Ext4Error, Ext4Result};
pub use traits::BlockDevice;
pub use types::*;

// 重新导出所有API函数
pub use fs::*;
pub use block::*;
pub use inode::*;
pub use dir::*;
pub use superblock::*;
