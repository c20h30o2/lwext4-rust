//! lwext4_core: Pure Rust ext4 filesystem implementation
//!
//! 这是一个纯 Rust 实现的 ext4 文件系统库，旨在提供：
//! - **零 unsafe 代码**（除必要的结构体定义）
//! - **Rust 惯用风格**的 API
//! - **完整的类型安全**
//! - **可选的 C API 兼容层**
//!
//! # 示例
//!
//! ```rust,ignore
//! use lwext4_core::{BlockDevice, block::BlockDev, Result};
//!
//! // 实现 BlockDevice trait
//! struct MyDevice {
//!     // ...
//! }
//!
//! impl BlockDevice for MyDevice {
//!     // 实现必要的方法
//!     // ...
//! }
//!
//! fn main() -> Result<()> {
//!     let device = MyDevice::new();
//!     let mut block_dev = BlockDev::new(device);
//!
//!     // 读取块
//!     let mut buf = vec![0u8; 4096];
//!     block_dev.read_block(0, &mut buf)?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # 模块结构
//!
//! - [`error`] - 错误类型定义
//! - [`block`] - 块设备抽象和 I/O 操作
//! - [`c_api`] - C API 兼容层（可选）

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

extern crate alloc;

// ===== 核心模块 =====

/// 错误处理
pub mod error;

/// 块设备抽象
pub mod block;

// 暂时注释掉未实现的模块
// pub mod fs;
// pub mod inode;
// pub mod dir;

// ===== C API 兼容层（可选）=====

/// C API 兼容层
///
/// 提供与 lwext4 C 库兼容的函数接口。
#[cfg(feature = "c-api")]
pub mod c_api;

// ===== 公共导出 =====

// 错误处理
pub use error::{Error, ErrorKind, Result};

// 块设备
pub use block::{BlockDevice, BlockDev};

// C API（当启用时）
#[cfg(feature = "c-api")]
pub use c_api::block::{
    ext4_blocks_get_direct, ext4_blocks_set_direct, ext4_block_readbytes,
    ext4_block_writebytes, ext4_block_cache_flush,
};
