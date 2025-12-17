# xattr 模块实现条件评估

## 概述

本文档评估当前 lwext4_core 实现 xattr (扩展属性) 模块所需的依赖和条件。

## xattr 功能说明

Extended Attributes (扩展属性) 允许在文件和目录上存储额外的元数据，格式为 name-value 对。
ext4 支持两种存储位置：
1. **Inode 内部** - 存储在 inode 的 extra space 中
2. **独立块** - 存储在独立的 xattr 块中（通过 inode->file_acl 指向）

## 依赖模块分析

### ✅ 已完成的依赖

| 模块 | 状态 | 说明 |
|------|------|------|
| types | ✅ | 完整的 ext4_inode, ext4_sblock 定义 |
| inode | ✅ | inode 读写操作 |
| block | ✅ | 块设备抽象和 I/O |
| superblock | ✅ | superblock 操作 |
| block_group | ✅ | 块组描述符操作 |
| balloc | ✅ | 块分配/释放功能 |
| transaction | ✅ | 事务支持 |
| cache | ✅ | 块缓存系统 |
| checksum (crc32c) | ✅ | 使用 crc32c crate |

### ⚠️ 需要补充的功能

#### 1. Inode ACL 字段访问器 (高优先级)

**缺失功能：**
```rust
// 需要在 lwext4_core/src/inode/read.rs 的 Inode impl 中添加
pub fn get_file_acl(&self, sb: &Superblock) -> u64 {
    let acl_lo = u32::from_le(self.inner.file_acl_lo) as u64;
    if sb.inner().creator_os == EXT4_SUPERBLOCK_OS_LINUX.to_le() {
        let acl_hi = u16::from_le(self.inner.file_acl_high) as u64;
        acl_lo | (acl_hi << 32)
    } else {
        acl_lo
    }
}
```

**当前状态：**
- ✅ `set_file_acl()` 已实现 (lwext4_core/src/inode/write.rs:296)
- ❌ `get_file_acl()` 缺失

#### 2. Inode Extra Size 访问器 (高优先级)

**缺失功能：**
```rust
// 需要在 lwext4_core/src/inode/read.rs 的 Inode impl 中添加
pub fn get_extra_isize(&self, sb: &Superblock) -> u16 {
    if sb.inode_size() <= EXT4_GOOD_OLD_INODE_SIZE as u16 {
        0
    } else {
        u16::from_le(self.inner.extra_isize)
    }
}
```

**当前状态：**
- ✅ `set_extra_isize()` 已实现 (lwext4_core/src/inode/write.rs)
- ❌ `get_extra_isize()` 缺失

#### 3. 块校验和计算 (中优先级)

**需要功能：**
```rust
// 计算 xattr 块的 CRC32C 校验和
pub fn xattr_block_checksum(
    inode_ref: &InodeRef,
    block_num: u64,
    header: &XattrHeader,
    block_data: &[u8],
) -> u32
```

**当前状态：**
- ✅ crc32c 已在其他模块使用 (inode, dir, balloc, superblock)
- ✅ 可以直接复用现有 crc32c 逻辑

#### 4. Transaction 块操作 (中优先级)

**需要功能：**
```rust
// 获取事务块（用于修改）
pub fn trans_block_get(bdev: &mut BlockDev, block_num: u64) -> Result<Block>

// 标记块为脏
pub fn trans_set_block_dirty(block: &mut Block)
```

**当前状态：**
- ✅ transaction 模块已实现
- ⚠️ 需要确认是否有 trans_block_get 和 trans_set_block_dirty 接口

## xattr 核心数据结构

### 需要定义的结构 (在 types.rs 中)

```rust
#[repr(C)]
pub struct ext4_xattr_header {
    pub h_magic: u32,      // 魔数 0xEA020000
    pub h_refcount: u32,   // 引用计数
    pub h_blocks: u32,     // 块数（通常为1）
    pub h_hash: u32,       // 哈希值
    pub h_checksum: u32,   // CRC32C 校验和
    pub h_reserved: [u32; 3],
}

#[repr(C)]
pub struct ext4_xattr_ibody_header {
    pub h_magic: u32,      // 魔数
}

#[repr(C)]
pub struct ext4_xattr_entry {
    pub e_name_len: u8,     // 名称长度
    pub e_name_index: u8,   // 命名空间索引
    pub e_value_offs: u16,  // 值偏移
    pub e_value_block: u32, // 值所在块（未使用）
    pub e_value_size: u32,  // 值大小
    pub e_hash: u32,        // 哈希值
}
```

## xattr 实现范围

### 核心功能

1. **xattr_list** - 列出所有扩展属性
2. **xattr_get** - 获取指定属性值
3. **xattr_set** - 设置/更新属性
4. **xattr_remove** - 删除属性

### 辅助功能

1. **命名空间前缀解析** (user., system., trusted., security.)
2. **inode 内部 xattr 操作**
3. **xattr 块操作**
4. **哈希计算和校验和验证**
5. **块引用计数管理** (COW 机制)

## 实现复杂度评估

### 低复杂度部分 (1-2天)
- ✅ 数据结构定义
- ✅ 命名空间前缀表
- ✅ 基本的读取操作 (list, get)

### 中复杂度部分 (3-4天)
- ⚠️ inode 内部 xattr 插入/删除
- ⚠️ xattr 块分配和初始化
- ⚠️ 空间管理和碎片整理

### 高复杂度部分 (5-7天)
- ❌ 块共享和 COW (copy-on-write)
- ❌ 哈希值和校验和计算
- ❌ 错误恢复和一致性检查

## 实现优先级建议

### 第一阶段：补充缺失的基础功能
1. 实现 `Inode::get_file_acl()`
2. 实现 `Inode::get_extra_isize()`
3. 确认 transaction 块操作接口

### 第二阶段：实现 xattr 基础结构
1. 添加 xattr 数据结构到 types.rs
2. 实现命名空间前缀解析
3. 实现 xattr 块校验和计算

### 第三阶段：实现只读操作
1. 实现 `xattr_list()` - 列出属性
2. 实现 `xattr_get()` - 读取属性值
3. 添加 xattr 验证逻辑

### 第四阶段：实现写操作
1. 实现 `xattr_set()` - 设置属性
2. 实现 `xattr_remove()` - 删除属性
3. 实现块引用计数管理

## 兼容性考虑

### 特性支持
- ✅ 支持 `metadata_csum` 特性的校验和
- ✅ 支持大 inode (> 128 字节)
- ✅ 支持 xattr 块共享 (引用计数)

### 命名空间
- ✅ user.* (用户属性)
- ✅ system.* (系统属性)
- ✅ security.* (安全标签)
- ✅ trusted.* (可信属性)
- ⚠️ ACL 属性 (system.posix_acl_*)

## 结论

**当前状态：基本具备实现条件 (85% 就绪)**

### 缺失项
1. 2个 inode 访问器函数 (15分钟工作量)
2. transaction 块操作确认 (30分钟)

### 建议
**可以开始实现 xattr 模块**，但建议：
1. 先补充缺失的 inode 访问器
2. 先实现只读功能 (list, get)
3. 再实现写功能 (set, remove)

### 预计工作量
- 补充基础功能：**0.5 天**
- 实现只读 xattr：**2-3 天**
- 实现完整 xattr：**7-10 天**

---

生成时间：2025-12-17
