//! 文件系统高级 API
//!
//! 这个模块提供完整的 ext4 文件系统操作接口。

mod filesystem;
mod file;
mod metadata;

pub use filesystem::Ext4FileSystem;
pub use file::File;
pub use metadata::{FileMetadata, FileType};
