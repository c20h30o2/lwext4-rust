# lwext4_core 设计原则（修订版）

**核心理念**: 源代码层面遵循C的lwext4，底层实现使用纯Rust

---

## 三大原则

### 原则1：命名完全遵循C ✅

**目的**: 便于对照C代码实现Rust版本

**要求**:
- 结构体名使用C的命名：`ext4_dir_en`（不是`Ext4DirEntry`）
- 字段名使用C的命名：`entry_len`（不是`entry_length`）
- 函数名使用C的命名：`ext4_fs_get_inode_ref`
- 常量名使用C的命名：`EXT4_DE_DIR`

**示例**:
```rust
// ✅ 正确：使用C命名
pub struct ext4_dir_en {
    pub inode: u32,
    pub entry_len: u16,    // C的字段名
    pub name_len: u8,      // C的字段名
}

pub fn ext4_fs_mount(...) { }  // C的函数名

// ❌ 错误：使用Rust惯用命名
pub struct Ext4DirEntry {      // 不要这样
    pub entry_length: u16,     // 不要这样
}
```

---

### 原则2：底层使用纯Rust实现 ✅

**目的**: 利用Rust的安全性和表达力

**允许**:
- ✅ 使用`Vec<u8>`代替柔性数组
- ✅ 使用`Option<T>`代替NULL指针
- ✅ 使用`Result<T, E>`代替错误码
- ✅ 使用普通结构体代替union
- ✅ 使用方法代替直接字段访问
- ✅ 使用Rust的所有权和借用

**避免**:
- ❌ 不需要`#[repr(C)]`（除非确实需要二进制兼容）
- ❌ 不需要`union`关键字
- ❌ 不需要零长度数组
- ❌ 尽量避免`unsafe`

**示例**:
```rust
// C的union用普通结构体+方法实现
pub struct ext4_dir_en_internal {
    value: u8,  // C中两个字段共用一个字节
}

impl ext4_dir_en_internal {
    // 提供C中union的两种访问方式
    pub fn name_length_high(&self) -> u8 { self.value }
    pub fn inode_type(&self) -> u8 { self.value }
}

// C的柔性数组用Vec实现
pub struct ext4_dir_en {
    pub inode: u32,
    pub entry_len: u16,
    pub name_len: u8,
    pub in_: ext4_dir_en_internal,
    name_data: Vec<u8>,  // 柔性数组用Vec
}

// C的错误码用Result
pub fn ext4_fs_mount(...) -> Result<(), Ext4Error> {
    // 不返回i32错误码
}
```

---

### 原则3：结构对应C的定义 ✅

**目的**: 保持概念上的一致性，便于实现

**要求**:
- 每个C结构体对应一个Rust结构体
- 字段数量和语义对应
- 但实现方式可以不同

**C代码对照表**:

| C文件 | 定义内容 | lwext4_core对应 |
|-------|---------|----------------|
| ext4_types.h | 基础类型 | src/types.rs |
| ext4_fs.h | 文件系统类型 | src/fs.rs |
| ext4_dir.h | 目录类型 | src/dir.rs |
| ext4_inode.h | inode类型 | src/inode.rs |
| ext4_blockdev.h | 块设备类型 | src/blockdev.rs |

---

## 具体设计示例

### 示例1: Union的Rust实现

**C定义** (`ext4_types.h`):
```c
union ext4_dir_en_internal {
    uint8_t name_length_high;
    uint8_t inode_type;
};
```

**lwext4_core实现**（不使用union关键字）:
```rust
/// 目录项内部字段
///
/// 对应C的union ext4_dir_en_internal
/// 在C中，两个字段占用同一个字节
/// 在Rust中，我们用一个字节+访问方法实现
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
```

### 示例2: 柔性数组的Rust实现

**C定义**:
```c
struct ext4_dir_en {
    uint32_t inode;
    uint16_t entry_len;
    uint8_t name_len;
    union ext4_dir_en_internal in;
    uint8_t name[];  // 柔性数组成员
};
```

**lwext4_core实现方案A**（使用Vec）:
```rust
/// 目录项结构
pub struct ext4_dir_en {
    pub inode: u32,
    pub entry_len: u16,
    pub name_len: u8,
    pub in_: ext4_dir_en_internal,
    /// 目录项名称（对应C的柔性数组name[]）
    name_data: Vec<u8>,
}

impl ext4_dir_en {
    pub fn new(inode: u32, name: &[u8], inode_type: u8) -> Self {
        Self {
            inode,
            entry_len: 0,  // 稍后计算
            name_len: name.len() as u8,
            in_: ext4_dir_en_internal::new(),
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
}
```

**lwext4_core实现方案B**（如果需要从磁盘读取）:
```rust
/// 目录项结构（磁盘格式）
///
/// 注意：这个结构体对应磁盘上的布局，不包含name数据
/// name数据紧跟在结构体后面
#[repr(C)]
pub struct ext4_dir_en_header {
    pub inode: u32,
    pub entry_len: u16,
    pub name_len: u8,
    pub in_: ext4_dir_en_internal,
}

/// 目录项（内存格式）
///
/// 包含完整的name数据
pub struct ext4_dir_en {
    pub header: ext4_dir_en_header,
    name_data: Vec<u8>,
}

impl ext4_dir_en {
    /// 从磁盘数据解析
    pub fn from_disk(data: &[u8]) -> Result<Self, Ext4Error> {
        // 读取头部
        let header: ext4_dir_en_header = /* 解析 */;

        // 读取name数据
        let name_start = core::mem::size_of::<ext4_dir_en_header>();
        let name_data = data[name_start..name_start + header.name_len as usize].to_vec();

        Ok(Self { header, name_data })
    }

    pub fn inode(&self) -> u32 { self.header.inode }
    pub fn entry_len(&self) -> u16 { self.header.entry_len }
    pub fn name_len(&self) -> u8 { self.header.name_len }
    pub fn name(&self) -> &[u8] { &self.name_data }
}
```

### 示例3: 函数签名的Rust化

**C函数**:
```c
int ext4_fs_get_inode_ref(
    struct ext4_fs *fs,
    struct ext4_inode_ref *inode_ref,
    uint32_t index
);
```

**lwext4_core实现**（Rust风格，但保持C函数名）:
```rust
/// 获取inode引用
///
/// 对应C函数: ext4_fs_get_inode_ref
///
/// 参数:
/// - fs: 文件系统对象
/// - index: inode索引
///
/// 返回:
/// - Ok(ext4_inode_ref): 成功
/// - Err(Ext4Error): 失败
pub fn ext4_fs_get_inode_ref(
    fs: &mut ext4_fs,
    index: u32,
) -> Result<ext4_inode_ref, Ext4Error> {
    // Rust实现
    // 不需要输出参数，直接返回Result
}

// 也可以提供方法形式
impl ext4_fs {
    pub fn get_inode_ref(&mut self, index: u32) -> Result<ext4_inode_ref, Ext4Error> {
        ext4_fs_get_inode_ref(self, index)
    }
}
```

---

## 命名规范详细说明

### 结构体命名

**规则**: 完全使用C的名称（小写+下划线）

```rust
// ✅ 正确
pub struct ext4_fs { }
pub struct ext4_inode { }
pub struct ext4_dir_en { }
pub struct ext4_blockdev { }

// ❌ 错误（不要用Rust惯用的大驼峰）
pub struct Ext4Fs { }
pub struct Ext4Inode { }
```

**可选**: 提供Rust风格的type alias
```rust
// 主定义用C风格
pub struct ext4_fs { ... }

// 可选的Rust风格别名
pub type Ext4Fs = ext4_fs;
pub type Ext4Filesystem = ext4_fs;
```

### 字段命名

**规则**: 使用C的字段名

```rust
pub struct ext4_dir_en {
    pub inode: u32,
    pub entry_len: u16,     // ✅ C的字段名
    pub name_len: u8,       // ✅ C的字段名
    pub in_: ext4_dir_en_internal,  // ✅ C的字段名（in是关键字，用in_）
}

// ❌ 不要
pub struct ext4_dir_en {
    pub entry_length: u16,  // ❌ Rust风格，不要
    pub name_length: u8,    // ❌ Rust风格，不要
}
```

### 函数命名

**规则**: 保持C的函数名

```rust
// ✅ 正确：C函数名
pub fn ext4_fs_mount(...) { }
pub fn ext4_fs_get_inode_ref(...) { }
pub fn ext4_dir_find_entry(...) { }

// ❌ 错误：Rust风格
pub fn mount_filesystem(...) { }  // 不要
pub fn get_inode_reference(...) { }  // 不要
```

### 常量命名

**规则**: 使用C的常量名

```rust
// ✅ 正确
pub const EXT4_DEV_BSIZE: usize = 512;
pub const EXT4_DE_DIR: u8 = 2;
pub const EXT4_INODE_BLOCKS: usize = 15;

// ❌ 错误
pub const DEVICE_BLOCK_SIZE: usize = 512;  // 不要
```

---

## 实现建议

### 1. 文件组织

```
lwext4_core/src/
├── lib.rs              - 导出所有内容
├── consts.rs           - 常量定义
├── error.rs            - 错误类型
├── types.rs            - 基础类型（ext4_inode等）
├── fs.rs               - 文件系统（ext4_fs等）
├── dir.rs              - 目录（ext4_dir_en等）
├── blockdev.rs         - 块设备
├── superblock.rs       - 超级块
└── inode.rs            - inode操作函数
```

### 2. 逐步实现策略

**阶段1**: 定义所有结构体（命名遵循C）
- 字段全部用C的名称
- 用纯Rust类型（Vec, Option等）

**阶段2**: 实现基础方法
- 构造函数
- 访问器方法
- 辅助方法

**阶段3**: 实现C风格的全局函数
- 保持C函数名
- 但用Rust的Result等

**阶段4**: 添加Rust风格的便利方法（可选）
- 在impl块中添加

### 3. 文档注释要求

每个结构体和函数都要注明对应的C定义

```rust
/// 目录项结构
///
/// 对应C定义: struct ext4_dir_en (ext4_types.h:825-833)
///
/// C中的柔性数组成员name[]在Rust中用Vec<u8>实现
pub struct ext4_dir_en {
    // ...
}

/// 获取inode引用
///
/// 对应C函数: ext4_fs_get_inode_ref (ext4_inode.h:45)
///
/// 与C版本的区别:
/// - C: 通过输出参数返回inode_ref，返回错误码
/// - Rust: 直接返回Result<ext4_inode_ref, Ext4Error>
pub fn ext4_fs_get_inode_ref(...) -> Result<...> {
    // ...
}
```

---

## 与lwext4_arce的交互

### lwext4_arce的适配非常简单

因为lwext4_core已经提供了：
- C风格的结构体名（ext4_dir_en）
- C风格的字段名（entry_len, name_len）
- 但是Rust的方法（name(), get_inode_type()）

```rust
// lwext4_arce/src/inode/dir.rs

use lwext4_core::ext4_dir_en;  // 直接使用

pub struct RawDirEntry {
    inner: ext4_dir_en,  // ✅ 类型名称一致
}

impl RawDirEntry {
    pub fn name(&self, sb: &ext4_sblock) -> &[u8] {
        // 调用lwext4_core的Rust方法
        let is_old = revision_tuple(sb) < (0, 5);
        if is_old {
            let len = self.inner.full_name_len(true);
            &self.inner.name()[..len]
        } else {
            self.inner.name()
        }
    }

    pub fn entry_len(&self) -> u16 {
        self.inner.entry_len  // ✅ 字段名一致，直接访问
    }
}
```

---

## 总结

### 三个"一致"

1. **命名一致**: 结构体名、字段名、函数名都用C的
2. **概念一致**: 每个C定义对应一个Rust定义
3. **功能一致**: 最终实现完整的ext4功能

### 三个"不同"

1. **实现不同**: 底层用纯Rust（Vec, Result等）
2. **安全不同**: 避免unsafe，利用Rust类型系统
3. **接口不同**: 提供Rust风格的方法（除了C函数名）

### 为什么这样设计？

✅ **便于实现**: 对照C代码一一实现
✅ **便于维护**: 清楚知道每个定义对应C的哪个
✅ **便于扩展**: 将来完整实现ext4功能
✅ **保持安全**: 使用Rust的安全特性
✅ **易于适配**: lwext4_arce的适配工作量小

---

## 下一步

按照这个设计原则：
1. 定义结构体（C命名，Rust实现）
2. 实现方法（便利的Rust方法）
3. 提供C风格函数（保持函数名）
4. lwext4_arce直接使用（适配简单）

要开始实施吗？
