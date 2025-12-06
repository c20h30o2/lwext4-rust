// 禁用标准库，适用于嵌入式或内核环境
#![no_std]

// FFI相关特性（仅use-ffi时启用）
#![cfg_attr(feature = "use-ffi", feature(linkage))]
#![cfg_attr(feature = "use-ffi", feature(c_variadic, c_size_t))]

// 启用关联类型默认值特性
#![feature(associated_type_defaults)]

//! 该模块是ext4文件系统实现的主入口，定义了对外暴露的接口和核心组件。

// 引入内存分配库
extern crate alloc;

// 引入日志宏
#[macro_use]
extern crate log;

// 内部实现：模拟libc的必要功能（如内存分配、打印）
#[cfg(feature = "use-ffi")]
mod ulibc;

// 对外暴露的FFI（Foreign Function Interface）绑定
#[cfg(feature = "use-ffi")]
pub mod ffi {
    // 允许非大写全局变量（C风格）
    #![allow(non_upper_case_globals)]
    // 允许非驼峰式类型名（C风格）
    #![allow(non_camel_case_types)]
    // 允许非蛇形函数名（C风格）
    #![allow(non_snake_case)]

    // 包含自动生成的C绑定（由bindgen生成）
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

// 使用纯 Rust 实现时，从 lwext4_core 导入所有接口
#[cfg(feature = "use-rust")]
pub mod ffi {
    // 重新导出 lwext4_core 的所有内容
    // lwext4_core 已经使用 C 风格命名（ext4_fs, ext4_sblock 等）
    // 并提供了 Rust 风格的类型别名（Ext4Filesystem, Ext4Superblock 等）
    pub use lwext4_core::*;

    // 以下类型暂时用占位，后续实现
    pub type ext4_blockdev_iface = u8;  // 占位
    pub type ext4_bcache = u8;  // 占位
    pub type ext4_dir_search_result = u8;  // 占位
}

// 块设备抽象模块
mod blockdev;
// 错误处理模块
mod error;
// 文件系统核心逻辑模块
mod fs;
// inode（索引节点）相关模块
mod inode;
// 工具函数模块
mod util;

// 对外暴露块设备相关类型
pub use blockdev::{BlockDevice, EXT4_DEV_BSIZE};
// 对外暴露错误处理类型
pub use error::{Ext4Error, Ext4Result};
// 对外暴露文件系统相关类型和方法
pub use fs::*;
// 对外暴露inode相关类型
pub use inode::*;