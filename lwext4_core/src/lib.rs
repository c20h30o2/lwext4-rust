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
pub mod superblock;
pub mod inode;
pub mod block;
pub mod dir;
pub mod fs;

// 重新导出常用类型
pub use consts::*;
pub use error::{Ext4Error, Ext4Result};
pub use types::*;

// 重新导出核心API
pub use fs::{ext4_fs_init, ext4_fs_fini};
pub use block::{ext4_block_init, ext4_block_fini, ext4_block_readbytes, ext4_block_writebytes};
pub use inode::{ext4_fs_get_inode_ref, ext4_fs_put_inode_ref};
pub use dir::{ext4_dir_find_entry, ext4_dir_iterator_init, ext4_dir_iterator_next};
