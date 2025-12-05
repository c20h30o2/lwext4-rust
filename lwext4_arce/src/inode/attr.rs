//! 该模块实现inode属性（元数据）的读写操作，如权限、大小、时间戳等。

use core::time::Duration;

use crate::{SystemHal, ffi::*, util::get_block_size};

use super::{InodeRef, InodeType};

/// 文件系统节点的元数据（属性）
#[derive(Clone, Debug, Default)]
pub struct FileAttr {
    /// 包含文件的设备ID
    pub device: u64,
    /// inode编号
    pub ino: u32,
    /// 硬链接数量
    pub nlink: u64,
    /// 权限模式（如0o644）
    pub mode: u32,
    /// 节点类型（文件/目录/链接等）
    pub node_type: InodeType,
    /// 所有者用户ID
    pub uid: u32,
    /// 所有者组ID
    pub gid: u32,
    /// 文件大小（字节）
    pub size: u64,
    /// 文件系统I/O块大小
    pub block_size: u64,
    /// 分配的512B块数量
    pub blocks: u64,

    /// 最后访问时间
    pub atime: Duration,
    /// 最后修改时间
    pub mtime: Duration,
    /// 最后状态修改时间
    pub ctime: Duration,
}

/// 将Duration转换为ext4存储的时间格式（秒+纳秒/扩展秒）
fn encode_time(dur: &Duration) -> (u32, u32) {
    let sec = dur.as_secs();
    let nsec = dur.subsec_nanos();
    let time = u32::to_le(sec as u32); // 秒部分（小端存储）
    // 纳秒左移2位 + 秒的高位（超过32位的部分）
    let extra = u32::to_le((nsec << 2) | (sec >> 32) as u32);
    (time, extra)
}

/// 将ext4存储的时间格式转换为Duration
fn decode_time(time: u32, extra: u32) -> Duration {
    let sec = u32::from_le(time); // 秒部分（从小端读取）
    let extra = u32::from_le(extra);
    let epoch = extra & 3; // 秒的高位（超过32位的部分）
    let nsec = extra >> 2; // 纳秒部分

    Duration::new(sec as u64 + ((epoch as u64) << 32), nsec)
}

impl<Hal: SystemHal> InodeRef<Hal> {
    /// 获取inode的类型（从模式字段解析）
    pub fn inode_type(&self) -> InodeType {
        ((self.mode() >> 12) as u8).into() // 模式字段高4位表示类型
    }

    /// 检查inode是否为目录
    pub fn is_dir(&self) -> bool {
        self.inode_type() == InodeType::Directory
    }

    /// 获取文件大小
    pub fn size(&self) -> u64 {
        unsafe {
            // 调用C函数获取大小
            ext4_inode_get_size(self.superblock() as *const _ as _, self.inner.inode)
        }
    }

    /// 获取权限模式
    pub fn mode(&self) -> u32 {
        unsafe {
            // 调用C函数获取模式
            ext4_inode_get_mode(self.superblock() as *const _ as _, self.inner.inode)
        }
    }

    /// 设置权限模式（仅保留低9位，即0o777范围）
    pub fn set_mode(&mut self, mode: u32) {
        unsafe {
            ext4_inode_set_mode(self.superblock_mut(), self.inner.inode, mode);
            self.mark_dirty(); // 标记为脏
        }
    }

    /// 获取硬链接计数
    pub fn nlink(&self) -> u16 {
        u16::from_le(self.raw_inode().links_count) // 从小端读取
    }

    /// 获取所有者用户ID
    pub fn uid(&self) -> u16 {
        u16::from_le(self.raw_inode().uid)
    }

    /// 获取所有者组ID
    pub fn gid(&self) -> u16 {
        u16::from_le(self.raw_inode().gid)
    }

    /// 设置所有者用户ID和组ID
    pub fn set_owner(&mut self, uid: u16, gid: u16) {
        let inode = self.raw_inode_mut();
        inode.uid = u16::to_le(uid); // 转换为小端存储
        inode.gid = u16::to_le(gid);
        self.mark_dirty();
    }

    /// 设置最后访问时间
    pub fn set_atime(&mut self, dur: &Duration) {
        let (time, extra) = encode_time(dur);
        let inode = self.raw_inode_mut();
        inode.access_time = time;
        inode.atime_extra = extra;
        self.mark_dirty();
    }

    /// 设置最后修改时间
    pub fn set_mtime(&mut self, dur: &Duration) {
        let (time, extra) = encode_time(dur);
        let inode = self.raw_inode_mut();
        inode.modification_time = time;
        inode.mtime_extra = extra;
        self.mark_dirty();
    }

    /// 设置最后状态修改时间
    pub fn set_ctime(&mut self, dur: &Duration) {
        let (time, extra) = encode_time(dur);
        let inode = self.raw_inode_mut();
        inode.change_inode_time = time;
        inode.ctime_extra = extra;
        self.mark_dirty();
    }

    /// 根据系统时间更新最后访问时间
    pub fn update_atime(&mut self) {
        if let Some(dur) = Hal::now() {
            self.set_atime(&dur);
        }
    }

    /// 根据系统时间更新最后修改时间
    pub fn update_mtime(&mut self) {
        if let Some(dur) = Hal::now() {
            self.set_mtime(&dur);
        }
    }

    /// 根据系统时间更新最后状态修改时间
    pub fn update_ctime(&mut self) {
        if let Some(dur) = Hal::now() {
            self.set_ctime(&dur);
        }
    }

    /// 读取inode的属性到FileAttr结构体
    pub fn get_attr(&self, attr: &mut FileAttr) {
        attr.device = 0; // 未实现设备ID
        attr.ino = u32::from_le(self.inner.index);
        attr.nlink = self.nlink() as _;
        attr.mode = self.mode();
        attr.node_type = self.inode_type();
        attr.uid = self.uid() as _;
        attr.gid = self.gid() as _;
        attr.size = self.size();
        attr.block_size = get_block_size(self.superblock()) as _;
        attr.blocks = unsafe {
            // 调用C函数获取块计数
            ext4_inode_get_blocks_count(self.superblock() as *const _ as _, self.inner.inode)
        };

        // 解析时间戳
        let inode = self.raw_inode();
        attr.atime = decode_time(inode.access_time, inode.atime_extra);
        attr.mtime = decode_time(inode.modification_time, inode.mtime_extra);
        attr.ctime = decode_time(inode.change_inode_time, inode.ctime_extra);
    }
}