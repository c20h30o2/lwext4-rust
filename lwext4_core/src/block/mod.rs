//! 块设备抽象
//!
//! 提供块设备接口和块级 I/O 操作。

mod device;
mod io;

pub use device::{BlockDevice, BlockDev};
