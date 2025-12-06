# Rust 惯用 API 设计分析

**日期**: 2025-12-06
**问题**: lwext4_core 的 C 风格设计 vs Rust 惯用设计

---

## 问题观察

### 1. 当前异常现象

#### 异常 1: types.rs 中所有结构体堆在一起
```rust
// lwext4_core/src/types.rs (200+ 行)
pub struct ext4_sblock { ... }
pub struct ext4_inode { ... }
pub struct ext4_blockdev { ... }
pub struct ext4_fs { ... }
pub struct ext4_bcache { ... }
// ... 10+ 个结构体
```

**Rust 惯例** ❌:
```
src/
├── sblock.rs          // pub struct Superblock { ... }
├── inode.rs           // pub struct Inode { ... }
├── blockdev.rs        // pub struct BlockDevice { ... }
└── fs.rs              // pub struct Filesystem { ... }
```

#### 异常 2: 大量独立的 pub 函数
```rust
// lwext4_core/src/inode.rs
pub fn ext4_inode_get_size(sb: *const Ext4Superblock, inode: *const Ext4Inode) -> u64 { ... }
pub fn ext4_inode_set_size(inode: *mut Ext4Inode, size: u64) { ... }
pub fn ext4_inode_get_mode(sb: *const Ext4Superblock, inode: *const Ext4Inode) -> u32 { ... }
pub fn ext4_inode_set_mode(sb: *mut Ext4Superblock, inode: *mut Ext4Inode, mode: u32) { ... }
// ... 10+ 个函数
```

**Rust 惯例** ❌:
```rust
impl Ext4Inode {
    pub fn size(&self, sb: &Ext4Superblock) -> u64 { ... }
    pub fn set_size(&mut self, size: u64) { ... }
    pub fn mode(&self, sb: &Ext4Superblock) -> u32 { ... }
    pub fn set_mode(&mut self, sb: &mut Ext4Superblock, mode: u32) { ... }
}
```

---

## 核心问题

### ✅ 你的理解 100% 正确！

**这些"异常"确实是为了保证 lwext4_core 的泛用性（C 兼容性）才存在的！**

---

## 设计原因分析

### 为什么是 C 风格？

#### 原因 1: "源码级 C 兼容性"设计原则

**lwext4_core 的核心目标**:
- ✅ 看起来像 C lwext4
- ✅ 可以作为 C FFI 的替代品
- ✅ 方便从 C 代码移植

**C 语言的特点**:
```c
// C 风格（lwext4 原始代码）
// 1. 所有类型定义在头文件
// ext4_types.h
struct ext4_inode { ... };
struct ext4_sblock { ... };

// 2. 函数都是独立的
// ext4_inode.c
uint64_t ext4_inode_get_size(struct ext4_sblock *sb, struct ext4_inode *inode);
void ext4_inode_set_size(struct ext4_inode *inode, uint64_t size);
```

**lwext4_core 的对应**:
```rust
// Rust 模拟 C 风格
// types.rs (对应 ext4_types.h)
pub struct ext4_inode { ... }
pub struct ext4_sblock { ... }

// inode.rs (对应 ext4_inode.c)
pub fn ext4_inode_get_size(sb: *const Ext4Superblock, inode: *const Ext4Inode) -> u64;
pub fn ext4_inode_set_size(inode: *mut Ext4Inode, size: u64);
```

**好处**:
- ✅ 移植 C 代码时几乎是 1:1 对应
- ✅ C 程序员容易理解
- ✅ 可以直接用于 FFI

#### 原因 2: 支持双模式（FFI + Rust）

**当前架构**:
```
lwext4_arce
├── use-ffi 模式 → C lwext4 (通过 bindgen)
└── use-rust 模式 → lwext4_core (纯 Rust，C 风格)
```

**如果 lwext4_core 用 Rust 风格**:
```rust
// ❌ 问题：不能用于 FFI
impl Ext4Inode {
    pub fn get_size(&self, sb: &Ext4Superblock) -> u64 { ... }
}

// FFI 需要 C 函数：
#[no_mangle]
pub extern "C" fn ext4_inode_get_size(...) -> u64 {
    // ❌ 如何适配？需要额外的包装层
}
```

**C 风格的优势**:
```rust
// ✅ 可以直接用于 FFI
pub fn ext4_inode_get_size(sb: *const Ext4Superblock, inode: *const Ext4Inode) -> u64 {
    // 纯 Rust 实现
}

// 如果需要 FFI，直接导出即可
#[no_mangle]
pub extern "C" fn ext4_inode_get_size(...) -> u64 {
    ext4_inode_get_size(...)  // ✅ 直接调用
}
```

---

## 是否应该创建纯 Rust 风格适配层？

### ✅ 你的建议非常好！

**是的，应该在 lwext4_core 之上创建一层纯 Rust 风格的适配层！**

---

## 推荐方案：双层架构

### 方案 A: 新增 lwext4_safe crate（推荐）⭐

```
项目结构:
lwext4-rust/
├── lwext4_core/          ← 保持 C 风格（底层，通用）
├── lwext4_safe/          ← 新增：纯 Rust 风格（上层，易用）
└── lwext4_arce/          ← arceos 适配层
```

#### lwext4_core (底层，C 风格)

**职责**:
- ✅ 提供 C 兼容的 API
- ✅ 支持 FFI 使用
- ✅ 作为实现基础

**风格**:
```rust
// lwext4_core/src/types.rs
pub struct ext4_inode { ... }
pub struct ext4_sblock { ... }

// lwext4_core/src/inode.rs
pub fn ext4_inode_get_size(sb: *const Ext4Superblock, inode: *const Ext4Inode) -> u64 { ... }
pub fn ext4_inode_set_size(inode: *mut Ext4Inode, size: u64) { ... }
```

**特点**:
- C 风格命名
- 使用原始指针
- 独立函数

#### lwext4_safe (上层，Rust 风格) ⭐

**职责**:
- ✅ 提供 Rust 惯用 API
- ✅ 类型安全
- ✅ 易于使用

**目录结构**:
```
lwext4_safe/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── inode.rs           // Inode 类型及方法
    ├── superblock.rs      // Superblock 类型及方法
    ├── blockdev.rs        // BlockDevice trait
    ├── filesystem.rs      // Filesystem 高层 API
    └── error.rs           // Rust 风格错误处理
```

**API 设计**:
```rust
// lwext4_safe/src/inode.rs
use lwext4_core;

/// Rust 风格的 Inode 包装
pub struct Inode {
    inner: lwext4_core::ext4_inode,
}

impl Inode {
    /// 获取 inode 大小（需要 superblock 信息）
    pub fn size(&self, sb: &Superblock) -> u64 {
        unsafe {
            lwext4_core::ext4_inode_get_size(
                &sb.inner as *const _,
                &self.inner as *const _
            )
        }
    }

    /// 设置 inode 大小
    pub fn set_size(&mut self, size: u64) {
        unsafe {
            lwext4_core::ext4_inode_set_size(
                &mut self.inner as *mut _,
                size
            )
        }
    }

    /// 获取 inode 模式
    pub fn mode(&self, sb: &Superblock) -> u32 {
        unsafe {
            lwext4_core::ext4_inode_get_mode(
                &sb.inner as *const _,
                &self.inner as *const _
            )
        }
    }

    /// 检查是否是目录
    pub fn is_dir(&self, sb: &Superblock) -> bool {
        self.mode(sb) & 0o040000 != 0
    }

    /// 检查是否是普通文件
    pub fn is_file(&self, sb: &Superblock) -> bool {
        self.mode(sb) & 0o100000 != 0
    }
}

// lwext4_safe/src/superblock.rs
pub struct Superblock {
    inner: lwext4_core::ext4_sblock,
}

impl Superblock {
    pub fn magic(&self) -> u16 {
        u16::from_le(self.inner.magic)
    }

    pub fn block_size(&self) -> u32 {
        1024 << self.inner.log_block_size
    }

    pub fn inodes_per_group(&self) -> u32 {
        u32::from_le(self.inner.inodes_per_group)
    }
}

// lwext4_safe/src/filesystem.rs
use std::path::Path;

pub struct Filesystem<D: BlockDevice> {
    inner: lwext4_core::ext4_fs,
    blockdev: D,
}

impl<D: BlockDevice> Filesystem<D> {
    /// 打开文件系统
    pub fn open(device: D) -> Result<Self, Error> {
        // 使用 lwext4_core 的 C 风格函数
        // 但对外提供 Rust 风格 API
        todo!()
    }

    /// 读取 inode
    pub fn inode(&mut self, ino: u32) -> Result<Inode, Error> {
        todo!()
    }

    /// 读取文件内容
    pub fn read_file(&mut self, path: &Path) -> Result<Vec<u8>, Error> {
        todo!()
    }

    /// 列出目录
    pub fn read_dir(&mut self, path: &Path) -> Result<Vec<DirEntry>, Error> {
        todo!()
    }
}
```

**使用示例**:
```rust
// 用户代码（纯 Rust 风格）
use lwext4_safe::{Filesystem, BlockDevice};

let device = MyBlockDevice::new("rootfs.img")?;
let mut fs = Filesystem::open(device)?;

// Rust 风格 API
let root = fs.inode(2)?;
assert!(root.is_dir(&fs.superblock()));

let entries = fs.read_dir("/bin")?;
for entry in entries {
    println!("{}: {} bytes", entry.name(), entry.size());
}

let data = fs.read_file("/etc/passwd")?;
println!("passwd: {}", String::from_utf8_lossy(&data));
```

---

## 方案对比

### 方案 A: 双层架构（lwext4_core + lwext4_safe）⭐ 推荐

**优点**:
- ✅ lwext4_core 保持 C 兼容性（FFI、移植、参考 C 代码）
- ✅ lwext4_safe 提供 Rust 体验（类型安全、易用）
- ✅ 各取所需：底层灵活，上层友好
- ✅ 清晰的职责分离

**缺点**:
- ⚠️ 需要维护额外的 crate
- ⚠️ 增加一点点开销（包装层）

**适用场景**:
- ✅ 需要同时支持 FFI 和 Rust 使用
- ✅ 有 C 代码移植需求
- ✅ 希望提供友好的 Rust API

### 方案 B: 在 lwext4_core 内提供双 API

**实现**:
```rust
// lwext4_core/src/inode.rs

// C 风格（保留）
pub fn ext4_inode_get_size(sb: *const Ext4Superblock, inode: *const Ext4Inode) -> u64 { ... }

// Rust 风格（新增）
impl Ext4Inode {
    pub fn size(&self, sb: &Ext4Superblock) -> u64 {
        unsafe { ext4_inode_get_size(sb, self) }
    }
}
```

**优点**:
- ✅ 单个 crate
- ✅ 两种风格都支持

**缺点**:
- ❌ 代码混乱（两种风格混在一起）
- ❌ 增加 lwext4_core 的复杂度
- ❌ 违反单一职责原则

### 方案 C: 完全改为 Rust 风格

**缺点**:
- ❌ 失去 C 兼容性
- ❌ use-ffi 模式需要额外适配
- ❌ 无法参考 C 代码
- ❌ 违反设计原则

**结论**: ❌ 不推荐

---

## 推荐的项目结构

```
lwext4-rust/
├── lwext4_core/                  # C 风格底层实现
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs              # 所有 C 风格类型（保持）
│       ├── consts.rs
│       ├── inode.rs              # C 风格独立函数
│       ├── block.rs
│       ├── dir.rs
│       └── fs.rs
│
├── lwext4_safe/                  # Rust 风格安全封装（新增）⭐
│   ├── Cargo.toml
│   │   [dependencies]
│   │   lwext4_core = { path = "../lwext4_core" }
│   └── src/
│       ├── lib.rs
│       ├── inode.rs              # pub struct Inode + impl
│       ├── superblock.rs         # pub struct Superblock + impl
│       ├── blockdev.rs           # pub trait BlockDevice
│       ├── filesystem.rs         # pub struct Filesystem<D>
│       ├── error.rs              # pub enum Error + Result<T>
│       ├── file.rs               # pub struct File + Read/Write traits
│       └── dir.rs                # pub struct DirEntry + Iterator
│
└── lwext4_arce/                  # arceos 适配层
    ├── Cargo.toml
    │   [dependencies]
    │   lwext4_core = { path = "../lwext4_core" }  # 底层
    │   # 或
    │   lwext4_safe = { path = "../lwext4_safe" }  # 高层
    └── src/
        └── ...
```

---

## 实现优先级

### 阶段 1: 完成 lwext4_core 功能实现（当前）⏰

**目标**: 让 lwext4_core 的占位符函数都有真实实现

**优先级**: P0（最高）

**原因**:
- lwext4_safe 依赖 lwext4_core 的功能
- 先确保底层可用

### 阶段 2: 创建 lwext4_safe（中期）📋

**时机**: lwext4_core 基本功能完成后（只读功能实现）

**步骤**:
1. 创建 lwext4_safe crate
2. 设计 Rust 风格 API
3. 实现类型包装和方法
4. 编写示例和文档

### 阶段 3: 完善和优化（长期）🚀

**内容**:
- 添加更多便利方法
- 性能优化
- 完善文档

---

## lwext4_safe API 设计草案

### 核心类型

```rust
// lwext4_safe/src/lib.rs

// 重新导出核心类型
pub use filesystem::Filesystem;
pub use inode::Inode;
pub use superblock::Superblock;
pub use blockdev::BlockDevice;
pub use error::{Error, Result};

// 便利类型
pub use file::File;
pub use dir::{DirEntry, ReadDir};
```

### 文件系统操作

```rust
// lwext4_safe/src/filesystem.rs

impl<D: BlockDevice> Filesystem<D> {
    // 打开文件系统
    pub fn open(device: D) -> Result<Self>;

    // 元数据
    pub fn superblock(&self) -> &Superblock;
    pub fn block_size(&self) -> u32;
    pub fn total_blocks(&self) -> u64;
    pub fn free_blocks(&self) -> u64;

    // Inode 操作
    pub fn inode(&mut self, ino: u32) -> Result<Inode>;
    pub fn root_inode(&mut self) -> Result<Inode>;

    // 路径操作
    pub fn lookup(&mut self, path: &Path) -> Result<Inode>;
    pub fn open_file(&mut self, path: &Path) -> Result<File>;
    pub fn read_dir(&mut self, path: &Path) -> Result<ReadDir>;

    // 文件操作（便利方法）
    pub fn read(&mut self, path: &Path) -> Result<Vec<u8>>;
    pub fn write(&mut self, path: &Path, data: &[u8]) -> Result<()>;
    pub fn create(&mut self, path: &Path) -> Result<File>;
    pub fn remove(&mut self, path: &Path) -> Result<()>;
    pub fn mkdir(&mut self, path: &Path) -> Result<()>;
}
```

### 文件操作

```rust
// lwext4_safe/src/file.rs

pub struct File<'fs, D: BlockDevice> {
    fs: &'fs mut Filesystem<D>,
    inode: Inode,
    pos: u64,
}

impl<'fs, D: BlockDevice> File<'fs, D> {
    pub fn size(&self) -> u64;
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    pub fn write(&mut self, buf: &[u8]) -> Result<usize>;
    pub fn seek(&mut self, pos: u64) -> Result<u64>;
}

// 实现标准 trait
impl<'fs, D: BlockDevice> std::io::Read for File<'fs, D> { ... }
impl<'fs, D: BlockDevice> std::io::Write for File<'fs, D> { ... }
impl<'fs, D: BlockDevice> std::io::Seek for File<'fs, D> { ... }
```

### 目录操作

```rust
// lwext4_safe/src/dir.rs

pub struct ReadDir<'fs, D: BlockDevice> {
    fs: &'fs mut Filesystem<D>,
    // ...
}

impl<'fs, D: BlockDevice> Iterator for ReadDir<'fs, D> {
    type Item = Result<DirEntry>;
    fn next(&mut self) -> Option<Self::Item>;
}

pub struct DirEntry {
    name: String,
    ino: u32,
    file_type: FileType,
}

impl DirEntry {
    pub fn name(&self) -> &str;
    pub fn ino(&self) -> u32;
    pub fn file_type(&self) -> FileType;
}

pub enum FileType {
    File,
    Directory,
    Symlink,
    // ...
}
```

---

## 总结

### ✅ 你的观察完全正确

1. **types.rs 堆所有结构体** ← C 风格，为了兼容性
2. **大量独立 pub 函数** ← C 风格，为了兼容性
3. **这些是为了泛用性** ← 是的！

### ✅ 你的建议非常好

**应该创建纯 Rust 风格适配层**：

```
推荐架构:
┌──────────────────────────────────────┐
│  lwext4_safe (Rust 风格，易用)        │
│  - impl Inode { fn size() }          │
│  - pub struct Filesystem<D>          │
│  - Result<T, Error>                  │
└──────────────────────────────────────┘
               ↓ 使用
┌──────────────────────────────────────┐
│  lwext4_core (C 风格，通用)           │
│  - pub fn ext4_inode_get_size()      │
│  - 原始指针                          │
│  - i32 错误码                        │
└──────────────────────────────────────┘
```

### 📋 实施计划

1. **当前阶段** ⏰: 完成 lwext4_core 功能实现（保持 C 风格）
2. **中期阶段** 📋: 创建 lwext4_safe（Rust 风格封装）
3. **长期阶段** 🚀: 完善和优化

**两层各司其职**:
- lwext4_core: 底层、通用、C 兼容
- lwext4_safe: 上层、易用、Rust 惯用

这是最佳设计！🎉
