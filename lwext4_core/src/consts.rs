//! ext4 常量定义

/// 块设备物理块大小（512 字节）
pub const EXT4_DEV_BSIZE: usize = 512;

/// Superblock 位置（从设备开始的字节偏移）
pub const EXT4_SUPERBLOCK_OFFSET: u64 = 1024;

/// Superblock 大小
pub const EXT4_SUPERBLOCK_SIZE: usize = 1024;

/// ext4 魔数
pub const EXT4_SUPERBLOCK_MAGIC: u16 = 0xEF53;

/// Inode 结构中的块指针数量（12个直接块 + 1个间接块 + 1个二级间接块 + 1个三级间接块）
pub const EXT4_INODE_BLOCKS: usize = 15;

/// 直接块数量
pub const EXT4_INODE_DIRECT_BLOCKS: usize = 12;

/// Inode flags: 使用 extent 树
pub const EXT4_INODE_FLAG_EXTENTS: u32 = 0x80000;

/// 目录项类型常量
pub const EXT4_DE_UNKNOWN: u8 = 0;
pub const EXT4_DE_REG_FILE: u8 = 1;
pub const EXT4_DE_DIR: u8 = 2;
pub const EXT4_DE_CHRDEV: u8 = 3;
pub const EXT4_DE_BLKDEV: u8 = 4;
pub const EXT4_DE_FIFO: u8 = 5;
pub const EXT4_DE_SOCK: u8 = 6;
pub const EXT4_DE_SYMLINK: u8 = 7;

/// 错误码（兼容 C errno）
pub const EOK: i32 = 0;
pub const EINVAL: i32 = 22;
pub const EIO: i32 = 5;
pub const ENOMEM: i32 = 12;
pub const ENOENT: i32 = 2;
pub const ENOSPC: i32 = 28;
pub const ENOTSUP: i32 = 95;
pub const EISDIR: i32 = 21;
pub const ENOTEMPTY: i32 = 39;

/// Inode 模式位
pub const EXT4_INODE_MODE_FIFO: u16 = 0x1000;
pub const EXT4_INODE_MODE_CHARDEV: u16 = 0x2000;
pub const EXT4_INODE_MODE_DIRECTORY: u16 = 0x4000;
pub const EXT4_INODE_MODE_BLOCKDEV: u16 = 0x6000;
pub const EXT4_INODE_MODE_FILE: u16 = 0x8000;
pub const EXT4_INODE_MODE_SOFTLINK: u16 = 0xA000;
pub const EXT4_INODE_MODE_SOCKET: u16 = 0xC000;
pub const EXT4_INODE_MODE_TYPE_MASK: u16 = 0xF000;
