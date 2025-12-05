//! 该模块是ext4文件系统实现的主入口，定义了对外暴露的接口和核心组件。

// 禁用标准库，适用于嵌入式或内核环境
#![no_std]
// 启用链接相关的特性
#![feature(linkage)]
// 启用C语言可变参数和size_t类型特性
#![feature(c_variadic, c_size_t)]
// 启用关联类型默认值特性
#![feature(associated_type_defaults)]

// 引入内存分配库
extern crate alloc;

// 引入日志宏
#[macro_use]
extern crate log;

// 内部实现：模拟libc的必要功能（如内存分配、打印）
mod ulibc;

// 对外暴露的FFI（Foreign Function Interface）绑定
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