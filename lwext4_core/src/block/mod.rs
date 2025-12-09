//! 块设备抽象
//!
//! 提供块设备接口和块级 I/O 操作。

mod device;
mod io;
mod handle;
mod lock;

pub use device::{BlockDevice, BlockDev};
pub use handle::Block;
pub use lock::{DeviceLock, NoLock};
