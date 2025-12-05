//! ext4 核心数据结构

use core::ptr;
use crate::consts::*;

/// Superblock 结构（简化版，只包含关键字段）
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4Superblock {
    pub inodes_count: u32,           // 0: 总 inode 数
    pub blocks_count_lo: u32,        // 4: 总块数（低32位）
    pub r_blocks_count_lo: u32,      // 8: 保留块数（低32位）
    pub free_blocks_count_lo: u32,   // 12: 空闲块数（低32位）
    pub free_inodes_count: u32,      // 16: 空闲 inode 数
    pub first_data_block: u32,       // 20: 第一个数据块
    pub log_block_size: u32,         // 24: 块大小（2^(10+log_block_size)）
    pub log_cluster_size: u32,       // 28: 簇大小
    pub blocks_per_group: u32,       // 32: 每组块数
    pub clusters_per_group: u32,     // 36: 每组簇数
    pub inodes_per_group: u32,       // 40: 每组 inode 数
    pub mtime: u32,                  // 44: 挂载时间
    pub wtime: u32,                  // 48: 写入时间
    pub mnt_count: u16,              // 52: 挂载次数
    pub max_mnt_count: u16,          // 54: 最大挂载次数
    pub magic: u16,                  // 56: 魔数 (0xEF53)
    pub state: u16,                  // 58: 文件系统状态
    pub errors: u16,                 // 60: 错误处理方式
    pub minor_rev_level: u16,        // 62: 次版本号
    pub lastcheck: u32,              // 64: 最后检查时间
    pub checkinterval: u32,          // 68: 检查间隔
    pub creator_os: u32,             // 72: 创建者操作系统
    pub rev_level: u32,              // 76: 版本级别
    pub def_resuid: u16,             // 80: 默认保留 uid
    pub def_resgid: u16,             // 82: 默认保留 gid

    // 扩展字段
    pub first_ino: u32,              // 84: 第一个非保留 inode
    pub inode_size: u16,             // 88: inode 大小
    pub block_group_nr: u16,         // 90: 本超级块所在的块组号
    pub feature_compat: u32,         // 92: 兼容特性
    pub feature_incompat: u32,       // 96: 不兼容特性
    pub feature_ro_compat: u32,      // 100: 只读兼容特性

    // 更多字段暂时省略，需要时添加
    pub reserved: [u8; 924],         // 填充到 1024 字节
}

impl Default for Ext4Superblock {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

/// Inode 结构（简化版）
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4Inode {
    pub mode: u16,                   // 0: 文件模式
    pub uid: u16,                    // 2: 所有者 uid（低16位）
    pub size_lo: u32,                // 4: 文件大小（低32位）
    pub atime: u32,                  // 8: 访问时间
    pub ctime: u32,                  // 12: 创建时间
    pub mtime: u32,                  // 16: 修改时间
    pub dtime: u32,                  // 20: 删除时间
    pub gid: u16,                    // 24: 组 gid（低16位）
    pub links_count: u16,            // 26: 硬链接数
    pub blocks_count_lo: u32,        // 28: 块数（低32位）
    pub flags: u32,                  // 32: 标志
    pub osd1: u32,                   // 36: OS 相关1
    pub block: [u32; EXT4_INODE_BLOCKS], // 40: 块指针数组（60字节）
    pub generation: u32,             // 100: 文件版本
    pub file_acl_lo: u32,            // 104: 文件 ACL（低32位）
    pub size_hi: u32,                // 108: 文件大小（高32位）

    // 更多字段暂时省略
    pub reserved: [u8; 28],          // 填充到标准 inode 大小
}

impl Default for Ext4Inode {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

/// Inode 引用（包含 inode 及其元数据）
pub struct Ext4InodeRef {
    pub index: u32,                  // inode 编号
    pub inode: *mut Ext4Inode,       // inode 指针
    pub fs: *mut Ext4Filesystem,     // 文件系统指针
    pub dirty: bool,                 // 是否已修改
    pub block_group: u32,            // 所属块组
}

impl Ext4InodeRef {
    pub fn new() -> Self {
        Self {
            index: 0,
            inode: ptr::null_mut(),
            fs: ptr::null_mut(),
            dirty: false,
            block_group: 0,
        }
    }
}

/// 块设备接口（trait，由调用者实现）
pub trait BlockDevice {
    fn read_blocks(&mut self, block_id: u64, buf: &mut [u8]) -> crate::Ext4Result<usize>;
    fn write_blocks(&mut self, block_id: u64, buf: &[u8]) -> crate::Ext4Result<usize>;
    fn num_blocks(&self) -> crate::Ext4Result<u64>;
}

/// 文件系统结构
pub struct Ext4Filesystem {
    pub sb: Ext4Superblock,          // Superblock
    pub block_size: u32,             // 块大小（字节）
    pub inode_size: u16,             // inode 大小
    pub inodes_per_group: u32,       // 每组 inode 数
    pub blocks_per_group: u32,       // 每组块数
    pub block_group_count: u32,      // 块组总数
}

impl Ext4Filesystem {
    pub fn new() -> Self {
        Self {
            sb: Ext4Superblock::default(),
            block_size: 0,
            inode_size: 0,
            inodes_per_group: 0,
            blocks_per_group: 0,
            block_group_count: 0,
        }
    }
}

/// 块设备结构（简化版）
pub struct Ext4BlockDevice {
    pub lg_bsize: u32,               // 逻辑块大小
    pub lg_bcnt: u64,                // 逻辑块数量
    pub ph_bsize: u32,               // 物理块大小（通常 512）
    pub ph_bcnt: u64,                // 物理块数量
}

impl Ext4BlockDevice {
    pub fn new() -> Self {
        Self {
            lg_bsize: 0,
            lg_bcnt: 0,
            ph_bsize: EXT4_DEV_BSIZE as u32,
            ph_bcnt: 0,
        }
    }
}

/// 目录项结构
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Ext4DirEntry {
    pub inode: u32,                  // inode 编号
    pub rec_len: u16,                // 记录长度
    pub name_len: u8,                // 名称长度
    pub inode_type: u8,              // inode 类型
    // name 字段动态长度，不在此定义
}

/// 目录迭代器
pub struct Ext4DirIterator {
    pub curr_offset: u64,            // 当前偏移量
    pub curr_inode: u32,             // 当前目录 inode
}

impl Ext4DirIterator {
    pub fn new(inode: u32) -> Self {
        Self {
            curr_offset: 0,
            curr_inode: inode,
        }
    }
}
