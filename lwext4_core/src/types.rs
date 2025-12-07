//! ext4 核心数据结构
//!
//! 设计原则：
//! 1. 命名完全遵循C（结构体名、字段名、函数名）
//! 2. 底层使用纯Rust实现（Vec、Result、Option等）
//! 3. 结构对应C的定义，但实现方式不同

// 允许C风格命名（这是有意为之，便于对照C代码实现）
#![allow(non_camel_case_types)]

use core::ptr;
use alloc::vec::Vec;
use crate::consts::*;

/// Superblock 结构
///
/// 对应C定义: struct ext4_sblock (ext4_types.h)
#[derive(Debug, Clone, Copy)]
pub struct ext4_sblock {
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

    // 更多字段（简化版 - 只保留关键字段）
    pub uuid: [u8; 16],              // 104: 128位UUID
    pub volume_name: [u8; 16],       // 120: 卷名称
    pub last_mounted: [u8; 64],      // 136: 最后挂载路径
    pub blocks_count_hi: u32,        // 200: 总块数（高32位）
    pub r_blocks_count_hi: u32,      // 204: 保留块数（高32位）
    pub free_blocks_count_hi: u32,   // 208: 空闲块数（高32位）

    // 填充到 1024 字节
    pub reserved: [u8; 812],         // 212-1023: 保留
}

impl Default for ext4_sblock {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

/// Inode 结构
///
/// 对应C定义: struct ext4_inode (ext4_types.h:373-419)
#[derive(Debug, Clone, Copy)]
pub struct ext4_inode {
    pub mode: u16,                   // 0: 文件模式
    pub uid: u16,                    // 2: 所有者 uid（低16位）
    pub size_lo: u32,                // 4: 文件大小（低32位）
    pub access_time: u32,            // 8: 访问时间（C字段名）
    pub change_inode_time: u32,      // 12: inode改变时间（C字段名）
    pub modification_time: u32,      // 16: 修改时间（C字段名）
    pub deletion_time: u32,          // 20: 删除时间（C字段名）
    pub gid: u16,                    // 24: 组 gid（低16位）
    pub links_count: u16,            // 26: 硬链接数
    pub blocks_count_lo: u32,        // 28: 块数（低32位）
    pub flags: u32,                  // 32: 标志
    pub osd1: u32,                   // 36: OS 相关1
    pub blocks: [u32; EXT4_INODE_BLOCKS], // 40: 块指针数组（C中是blocks，复数）
    pub generation: u32,             // 100: 文件版本
    pub file_acl_lo: u32,            // 104: 文件 ACL（低32位）
    pub size_hi: u32,                // 108: 文件大小（高32位）
    pub obso_faddr: u32,             // 112: 废弃的fragment地址

    // osd2 union - 简化处理，只保留关键字段
    pub blocks_high: u16,            // 116: 块数高16位
    pub file_acl_high: u16,          // 118: ACL高16位
    pub uid_high: u16,               // 120: uid高16位
    pub gid_high: u16,               // 122: gid高16位
    pub checksum_lo: u16,            // 124: 校验和低16位
    pub reserved2: u16,              // 126: 保留

    // 扩展字段
    pub extra_isize: u16,            // 128: 额外inode大小
    pub checksum_hi: u16,            // 130: 校验和高16位
    pub ctime_extra: u32,            // 132: 额外change时间
    pub mtime_extra: u32,            // 136: 额外modification时间
    pub atime_extra: u32,            // 140: 额外access时间
    pub crtime: u32,                 // 144: 创建时间
    pub crtime_extra: u32,           // 148: 额外创建时间
    pub version_hi: u32,             // 152: 版本高32位
}

impl Default for ext4_inode {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

/// Inode 引用
///
/// 对应C定义: struct ext4_inode_ref (ext4_fs.h)
pub struct ext4_inode_ref {
    pub index: u32,                  // inode 编号
    pub inode: *mut ext4_inode,      // inode 指针
    pub fs: *mut ext4_fs,            // 文件系统指针
    pub dirty: bool,                 // 是否已修改
    pub block_group: u32,            // 所属块组
}

impl ext4_inode_ref {
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

/// 文件系统结构
///
/// 对应C定义: struct ext4_fs (ext4_fs.h:56-70)
pub struct ext4_fs {
    pub read_only: bool,             // 只读模式
    pub bdev: *mut ext4_blockdev,    // 块设备指针
    pub sb: ext4_sblock,             // Superblock
    pub inode_block_limits: [u64; 4], // inode 块限制
    pub inode_blocks_per_level: [u64; 4], // 每级 inode 块数
    pub block_size: u32,             // 块大小（字节）
    pub inode_size: u32,             // inode 大小
    pub inodes_per_group: u32,       // 每组 inode 数
    pub blocks_per_group: u32,       // 每组块数
    pub block_group_count: u32,      // 块组总数
}

impl ext4_fs {
    pub fn new() -> Self {
        Self {
            read_only: false,
            bdev: ptr::null_mut(),
            sb: ext4_sblock::default(),
            inode_block_limits: [0; 4],
            inode_blocks_per_level: [0; 4],
            block_size: 0,
            inode_size: 0,
            inodes_per_group: 0,
            blocks_per_group: 0,
            block_group_count: 0,
        }
    }
}

/// 缓冲区结构
///
/// 对应C定义: struct ext4_buf (ext4_bcache.h)
pub struct ext4_buf {
    pub flags: i32,                  // 标志位
    pub lba: u64,                    // 逻辑块地址
    pub data: *mut u8,               // 数据指针
    pub lru_prio: u32,               // LRU优先级
    pub lru_id: u32,                 // LRU ID
    pub refctr: u32,                 // 引用计数
    pub bc: *mut u8,                 // 块缓存指针
    pub on_dirty_list: bool,         // 是否在脏列表中
}

/// 块缓存条目
///
/// 对应C定义: struct ext4_block (ext4_bcache.h:55-64)
pub struct ext4_block {
    pub lb_id: u64,                  // 逻辑块ID
    pub buf: *mut ext4_buf,          // 缓冲区指针
    pub data: *mut u8,               // 数据指针
}

impl ext4_block {
    pub fn new() -> Self {
        Self {
            lb_id: 0,
            buf: ptr::null_mut(),
            data: ptr::null_mut(),
        }
    }
}

/// 块设备接口结构
///
/// 对应C定义: struct ext4_blockdev_iface (ext4_blockdev.h:49-103)
pub struct ext4_blockdev_iface {
    // 函数指针（使用Option包装，允许为空）
    pub open: Option<unsafe extern "C" fn(*mut ext4_blockdev) -> i32>,
    pub bread: Option<unsafe extern "C" fn(*mut ext4_blockdev, *mut core::ffi::c_void, u64, u32) -> i32>,
    pub bwrite: Option<unsafe extern "C" fn(*mut ext4_blockdev, *const core::ffi::c_void, u64, u32) -> i32>,
    pub close: Option<unsafe extern "C" fn(*mut ext4_blockdev) -> i32>,
    pub lock: Option<unsafe extern "C" fn(*mut ext4_blockdev) -> i32>,
    pub unlock: Option<unsafe extern "C" fn(*mut ext4_blockdev) -> i32>,

    // 数据字段
    pub ph_bsize: u32,               // 物理块大小
    pub ph_bcnt: u64,                // 物理块数量
    pub ph_bbuf: *mut u8,            // 物理块缓冲区
    pub ph_refctr: u32,              // 引用计数
    pub bread_ctr: u32,              // 读计数
    pub bwrite_ctr: u32,             // 写计数
    pub p_user: *mut core::ffi::c_void,  // 用户数据指针
}

impl ext4_blockdev_iface {
    pub fn new() -> Self {
        Self {
            open: None,
            bread: None,
            bwrite: None,
            close: None,
            lock: None,
            unlock: None,
            ph_bsize: 0,
            ph_bcnt: 0,
            ph_bbuf: ptr::null_mut(),
            ph_refctr: 0,
            bread_ctr: 0,
            bwrite_ctr: 0,
            p_user: ptr::null_mut(),
        }
    }
}

/// 块缓存结构
///
/// 对应C定义: struct ext4_bcache (ext4_bcache.h)
#[repr(C)]
pub struct ext4_bcache {
    pub cnt: u32,                    // 块缓存中的项目数量
    pub itemsize: u32,               // 块缓存中每个项目的大小
    pub lru_ctr: u32,                // 最近使用计数器
    pub ref_blocks: u32,             // 当前引用的数据块
    pub max_ref_blocks: u32,         // 最大引用的数据块
    pub bdev: *mut ext4_blockdev,   // 绑定到此块缓存的块设备
    // 其他字段暂时省略（如dirty_list等）
}

impl ext4_bcache {
    pub fn new() -> Self {
        Self {
            cnt: 0,
            itemsize: 0,
            lru_ctr: 0,
            ref_blocks: 0,
            max_ref_blocks: 0,
            bdev: ptr::null_mut(),
        }
    }
}

/// 块设备结构
///
/// 对应C定义: struct ext4_blockdev (ext4_blockdev.h:106-132)
pub struct ext4_blockdev {
    pub bdif: *mut ext4_blockdev_iface,  // 块设备接口
    pub part_offset: u64,            // 分区偏移（多分区模式）
    pub part_size: u64,              // 分区大小（多分区模式）
    pub bc: *mut ext4_bcache,        // 块缓存
    pub lg_bsize: u32,               // 逻辑块大小
    pub lg_bcnt: u64,                // 逻辑块数量
    pub cache_write_back: u32,       // 缓存回写模式引用计数
    pub fs: *mut ext4_fs,            // 所属文件系统
    pub journal: *mut u8,            // 日志（暂用u8）
    pub ph_bsize: u32,               // 物理块大小（通常 512）
    pub ph_bcnt: u64,                // 物理块数量
}

impl ext4_blockdev {
    pub fn new() -> Self {
        Self {
            bdif: ptr::null_mut(),
            part_offset: 0,
            part_size: 0,
            bc: ptr::null_mut(),
            lg_bsize: 0,
            lg_bcnt: 0,
            cache_write_back: 0,
            fs: ptr::null_mut(),
            journal: ptr::null_mut(),
            ph_bsize: EXT4_DEV_BSIZE as u32,
            ph_bcnt: 0,
        }
    }
}

/// 目录项内部字段
///
/// 对应C定义: union ext4_dir_en_internal (ext4_types.h)
/// C中是union，两个字段占用同一个字节
/// Rust实现：用一个字节+访问方法
pub struct ext4_dir_en_internal {
    /// 这个字节的两种解释：
    /// - 旧版本ext4: 存储name_length_high
    /// - 新版本ext4: 存储inode_type
    value: u8,
}

impl ext4_dir_en_internal {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    /// 作为name_length_high访问（旧版本）
    pub fn name_length_high(&self) -> u8 {
        self.value
    }

    /// 作为inode_type访问（新版本）
    pub fn inode_type(&self) -> u8 {
        self.value
    }

    /// 设置name_length_high
    pub fn set_name_length_high(&mut self, val: u8) {
        self.value = val;
    }

    /// 设置inode_type
    pub fn set_inode_type(&mut self, val: u8) {
        self.value = val;
    }
}

/// 目录项结构
///
/// 对应C定义: struct ext4_dir_en (ext4_types.h:825-833)
/// C中的柔性数组成员name[]在Rust中用Vec<u8>实现
pub struct ext4_dir_en {
    pub inode: u32,                  // inode 编号
    pub entry_len: u16,              // 记录长度（C字段名）
    pub name_len: u8,                // 名称长度（C字段名）
    pub in_: ext4_dir_en_internal,   // union字段（C字段名）
    name_data: Vec<u8>,              // 目录项名称（对应C的柔性数组name[]）
}

impl ext4_dir_en {
    /// 创建新的目录项
    pub fn new(inode: u32, name: &[u8], inode_type: u8) -> Self {
        let mut in_ = ext4_dir_en_internal::new();
        in_.set_inode_type(inode_type);

        Self {
            inode,
            entry_len: 0,  // 稍后计算
            name_len: name.len() as u8,
            in_,
            name_data: name.to_vec(),
        }
    }

    /// 获取名称
    pub fn name(&self) -> &[u8] {
        &self.name_data
    }

    /// 获取完整名称长度（处理旧版本的高8位）
    pub fn full_name_len(&self, old_version: bool) -> usize {
        let mut len = self.name_len as usize;
        if old_version {
            len |= (self.in_.name_length_high() as usize) << 8;
        }
        len
    }

    /// 获取inode类型
    pub fn get_inode_type(&self) -> u8 {
        self.in_.inode_type()
    }
}

/// 目录迭代器
///
/// 对应C定义: struct ext4_dir_iter (ext4_dir.h:57-62)
pub struct ext4_dir_iter {
    pub inode_ref: *mut ext4_inode_ref, // inode引用指针
    pub curr_blk: ext4_block,        // 当前块
    pub curr_off: u64,               // 当前偏移量（C字段名）
    pub curr: *mut ext4_dir_en,      // 当前目录项指针
}

impl ext4_dir_iter {
    pub fn new() -> Self {
        Self {
            inode_ref: ptr::null_mut(),
            curr_blk: ext4_block::new(),
            curr_off: 0,
            curr: ptr::null_mut(),
        }
    }
}

/// 目录搜索结果
///
/// 对应C定义: struct ext4_dir_search_result (ext4_dir.h)
pub struct ext4_dir_search_result {
    pub block: ext4_block,          // 块
    pub dentry: *mut ext4_dir_en,   // 目录项指针
}

impl ext4_dir_search_result {
    pub fn new() -> Self {
        Self {
            block: ext4_block::new(),
            dentry: ptr::null_mut(),
        }
    }
}

// ===== Type Aliases =====
// 提供Rust风格的别名，方便使用

/// Rust风格别名：Superblock
pub type Ext4Superblock = ext4_sblock;

/// Rust风格别名：Inode
pub type Ext4Inode = ext4_inode;

/// Rust风格别名：Inode引用
pub type Ext4InodeRef = ext4_inode_ref;

/// Rust风格别名：文件系统
pub type Ext4Filesystem = ext4_fs;

/// Rust风格别名：块设备
pub type Ext4BlockDevice = ext4_blockdev;

/// Rust风格别名：块设备接口
pub type Ext4BlockDeviceIface = ext4_blockdev_iface;

/// Rust风格别名：块缓存
pub type Ext4BlockCache = ext4_bcache;

/// Rust风格别名：缓冲区
pub type Ext4Buf = ext4_buf;

/// Rust风格别名：块
pub type Ext4Block = ext4_block;

/// Rust风格别名：目录项
pub type Ext4DirEntry = ext4_dir_en;

/// Rust风格别名：目录项内部字段
pub type Ext4DirEntryInternal = ext4_dir_en_internal;

/// Rust风格别名：目录迭代器
pub type Ext4DirIterator = ext4_dir_iter;

/// Rust风格别名：目录搜索结果
pub type Ext4DirSearchResult = ext4_dir_search_result;

//=============================================================================
// Rust 风格重构定义（新增）
//=============================================================================

use crate::traits::BlockDevice;
use alloc::boxed::Box;

/// 块设备结构（Rust 风格）
///
/// 对应 C 的 `ext4_blockdev`，但使用泛型和 Rust 特性
pub struct Ext4BlockDev<D: BlockDevice> {
    /// 底层块设备
    device: D,

    /// 分区偏移（字节）
    part_offset: u64,

    /// 分区大小（字节）
    part_size: u64,

    /// 逻辑块大小（字节）
    lg_bsize: u32,

    /// 逻辑块数量
    lg_bcnt: u64,

    /// 物理块大小（字节）
    ph_bsize: u32,

    /// 物理块数量
    ph_bcnt: u64,

    /// 块缓存（可选）
    cache: Option<Box<Ext4BCache>>,

    /// 缓存回写模式计数
    cache_write_back: u32,

    /// 读操作计数（统计）
    bread_ctr: u64,

    /// 写操作计数（统计）
    bwrite_ctr: u64,
}

impl<D: BlockDevice> Ext4BlockDev<D> {
    /// 创建新的块设备
    pub fn new(device: D) -> Self {
        let ph_bsize = device.physical_block_size();
        let lg_bsize = device.block_size();
        let total_blocks = device.total_blocks();

        Self {
            device,
            part_offset: 0,
            part_size: 0,
            lg_bsize,
            lg_bcnt: total_blocks,
            ph_bsize,
            ph_bcnt: total_blocks,
            cache: None,
            cache_write_back: 0,
            bread_ctr: 0,
            bwrite_ctr: 0,
        }
    }

    /// 获取底层设备的可变引用
    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    /// 获取底层设备的引用
    pub fn device(&self) -> &D {
        &self.device
    }

    /// 获取逻辑块大小
    pub fn lg_bsize(&self) -> u32 {
        self.lg_bsize
    }

    /// 获取物理块大小
    pub fn ph_bsize(&self) -> u32 {
        self.ph_bsize
    }

    /// 获取逻辑块数量
    pub fn lg_bcnt(&self) -> u64 {
        self.lg_bcnt
    }

    /// 获取物理块数量
    pub fn ph_bcnt(&self) -> u64 {
        self.ph_bcnt
    }

    /// 获取分区偏移
    pub fn part_offset(&self) -> u64 {
        self.part_offset
    }

    /// 设置分区偏移
    pub fn set_part_offset(&mut self, offset: u64) {
        self.part_offset = offset;
    }

    /// 获取读操作计数
    pub fn bread_ctr(&self) -> u64 {
        self.bread_ctr
    }

    /// 获取写操作计数
    pub fn bwrite_ctr(&self) -> u64 {
        self.bwrite_ctr
    }

    /// 增加读操作计数
    pub(crate) fn inc_bread_ctr(&mut self) {
        self.bread_ctr += 1;
    }

    /// 增加写操作计数
    pub(crate) fn inc_bwrite_ctr(&mut self) {
        self.bwrite_ctr += 1;
    }
}

/// 块缓存结构（Rust 风格）- 暂未实现
///
/// 对应 C 的 `ext4_bcache`，使用 Rust 惯用数据结构
/// TODO: 实现实际的缓存逻辑（LRU、HashMap 等）
pub struct Ext4BCache {
    /// 缓存项数量
    cnt: u32,

    /// 每个缓存项大小
    itemsize: u32,

    /// LRU 计数器
    lru_ctr: u32,

    /// 当前引用的块数
    ref_blocks: u32,

    /// 最大引用块数
    max_ref_blocks: u32,

    // TODO: 实际的缓存数据结构（HashMap, LRU list 等）
}

impl Ext4BCache {
    /// 创建新的块缓存
    pub fn new(cnt: u32, itemsize: u32) -> Self {
        Self {
            cnt,
            itemsize,
            lru_ctr: 0,
            ref_blocks: 0,
            max_ref_blocks: 0,
        }
    }
}
