# lwext4-rust 测试策略

**日期**: 2025-12-06
**问题**: 如何在开发过程中有效测试 lwext4_core 的实现？

---

## 测试挑战

### 问题分析

1. **arceos 集成测试太重** ❌
   - 需要完整的 arceos 环境
   - 依赖太多
   - 调试困难

2. **lwext4_arce 层测试需要准备** ⚠️
   - 需要 ext4 镜像文件
   - 需要实现块设备驱动

3. **lwext4_core 层测试太细** ⚠️
   - 需要手动构造数据结构
   - 需要提供磁盘数据访问

---

## 推荐测试方案 ⭐

### 三层测试策略（由细到粗）

```
┌─────────────────────────────────────────┐
│ 层次 3: arceos 集成测试                 │  ← 最后阶段
│ - 真实 arceos 环境                      │
│ - 完整功能验证                          │
└─────────────────────────────────────────┘
               ↑
┌─────────────────────────────────────────┐
│ 层次 2: lwext4_arce 集成测试 ⭐         │  ← 主要测试层
│ - 真实 ext4 镜像                        │
│ - 内存块设备（简单实现）                │
│ - 完整调用链测试                        │
└─────────────────────────────────────────┘
               ↑
┌─────────────────────────────────────────┐
│ 层次 1: lwext4_core 单元测试            │  ← 开发阶段
│ - Mock 数据                             │
│ - 单个函数验证                          │
│ - 快速迭代                              │
└─────────────────────────────────────────┘
```

---

## 实施方案

### 方案 1: 准备测试镜像（一次性工作）

#### 创建小型测试镜像

```bash
# 在项目根目录创建 test-images 目录
mkdir -p test-images
cd test-images

# 1. 创建 10MB 空镜像
dd if=/dev/zero of=test.ext4 bs=1M count=10

# 2. 格式化为 ext4
mkfs.ext4 test.ext4

# 3. 挂载并创建测试文件
mkdir -p mnt
sudo mount test.ext4 mnt

# 4. 创建测试数据
sudo mkdir -p mnt/dir1 mnt/dir2
echo "Hello, world!" | sudo tee mnt/file1.txt
echo "Test data" | sudo tee mnt/dir1/file2.txt
sudo dd if=/dev/zero of=mnt/large.bin bs=1K count=100

# 5. 卸载
sudo umount mnt

# 6. 提交到仓库
git add test.ext4
```

**优点**:
- ✅ 一次创建，重复使用
- ✅ 包含真实 ext4 结构
- ✅ 大小可控（10MB）

#### 查看镜像信息

```bash
# 查看 superblock
dumpe2fs test-images/test.ext4 | head -50

# 查看 inode 信息
debugfs test-images/test.ext4 -R "stat <2>"  # 根目录 inode

# 十六进制查看
hexdump -C test-images/test.ext4 | head -100
```

---

### 方案 2: 实现简单的内存块设备 ⭐

#### 文件块设备（推荐）

```rust
// lwext4_arce/tests/common/mod.rs

use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use lwext4_arce::{BlockDevice, Ext4Result};

/// 基于文件的块设备（用于测试）
pub struct FileBlockDevice {
    file: File,
    block_size: usize,
}

impl FileBlockDevice {
    /// 打开测试镜像
    pub fn open(path: &str) -> std::io::Result<Self> {
        let file = File::options()
            .read(true)
            .write(true)
            .open(path)?;

        Ok(Self {
            file,
            block_size: 512,  // 默认块大小
        })
    }

    /// 从测试镜像创建（相对于项目根目录）
    pub fn from_test_image(name: &str) -> std::io::Result<Self> {
        let path = format!("test-images/{}", name);
        Self::open(&path)
    }
}

impl BlockDevice for FileBlockDevice {
    fn read_blocks(&mut self, block_id: u64, buf: &mut [u8]) -> Ext4Result<usize> {
        let offset = block_id * self.block_size as u64;
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| lwext4_arce::Ext4Error::new(EIO as _, &format!("seek failed: {}", e)))?;

        self.file.read(buf)
            .map_err(|e| lwext4_arce::Ext4Error::new(EIO as _, &format!("read failed: {}", e)))
    }

    fn write_blocks(&mut self, block_id: u64, buf: &[u8]) -> Ext4Result<usize> {
        let offset = block_id * self.block_size as u64;
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| lwext4_arce::Ext4Error::new(EIO as _, &format!("seek failed: {}", e)))?;

        self.file.write(buf)
            .map_err(|e| lwext4_arce::Ext4Error::new(EIO as _, &format!("write failed: {}", e)))
    }

    fn num_blocks(&self) -> Ext4Result<u64> {
        let size = self.file.metadata()
            .map_err(|e| lwext4_arce::Ext4Error::new(EIO as _, &format!("metadata failed: {}", e)))?
            .len();
        Ok(size / self.block_size as u64)
    }
}
```

**使用**:
```rust
#[test]
fn test_open_filesystem() {
    let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
    let fs = Ext4Filesystem::new(device, FsConfig::default()).unwrap();
    // 测试...
}
```

---

### 方案 3: lwext4_core 单元测试

#### 测试辅助工具

```rust
// lwext4_core/tests/common/mod.rs

use lwext4_core::*;

/// 测试辅助：创建最小的 superblock
pub fn create_test_superblock() -> ext4_sblock {
    let mut sb: ext4_sblock = unsafe { std::mem::zeroed() };

    // 设置必要字段
    sb.magic = 0xEF53u16.to_le();
    sb.log_block_size = 2;  // 4096 字节
    sb.inodes_per_group = 8192;
    sb.blocks_per_group = 32768;
    sb.inodes_count = 8192;
    sb.blocks_count_lo = 2560;  // 10MB / 4KB

    sb
}

/// 测试辅助：创建测试用 inode
pub fn create_test_inode(mode: u16, size: u64) -> ext4_inode {
    let mut inode: ext4_inode = unsafe { std::mem::zeroed() };

    inode.mode = mode.to_le();
    inode.size_lo = (size as u32).to_le();
    inode.size_hi = ((size >> 32) as u32).to_le();

    inode
}

/// 从真实镜像读取数据块
pub fn read_block_from_image(image_path: &str, block_id: u64) -> Vec<u8> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(image_path).unwrap();
    let mut buf = vec![0u8; 4096];

    file.seek(SeekFrom::Start(block_id * 4096)).unwrap();
    file.read_exact(&mut buf).unwrap();

    buf
}
```

#### 单元测试示例

```rust
// lwext4_core/tests/inode_tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_get_size() {
        let sb = create_test_superblock();
        let inode = create_test_inode(0o100644, 12345);

        let size = ext4_inode_get_size(&sb, &inode);
        assert_eq!(size, 12345);
    }

    #[test]
    fn test_inode_get_mode() {
        let sb = create_test_superblock();
        let inode = create_test_inode(0o100644, 0);

        let mode = ext4_inode_get_mode(&sb, &inode);
        assert_eq!(mode, 0o100644);
    }

    #[test]
    fn test_read_real_superblock() {
        // 从真实镜像读取 superblock
        let data = read_block_from_image("../test-images/test.ext4", 0);

        // superblock 在偏移 1024 处
        let sb_bytes = &data[1024..1024 + std::mem::size_of::<ext4_sblock>()];
        let sb: &ext4_sblock = unsafe {
            &*(sb_bytes.as_ptr() as *const ext4_sblock)
        };

        // 验证魔数
        assert_eq!(u16::from_le(sb.magic), 0xEF53);

        // 验证块大小
        let block_size = 1024 << sb.log_block_size;
        assert_eq!(block_size, 4096);
    }
}
```

---

### 方案 4: lwext4_arce 集成测试 ⭐ 推荐

#### 完整的集成测试

```rust
// lwext4_arce/tests/integration_test.rs

mod common;
use common::FileBlockDevice;
use lwext4_arce::*;

#[test]
fn test_open_filesystem() {
    let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
    let fs = Ext4Filesystem::<DummyHal, _>::new(device, FsConfig::default()).unwrap();

    // 成功打开即通过
}

#[test]
fn test_read_superblock() {
    let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
    let fs = Ext4Filesystem::<DummyHal, _>::new(device, FsConfig::default()).unwrap();

    // 访问 superblock（需要添加 getter）
    let magic = fs.superblock().magic();
    assert_eq!(magic, 0xEF53);
}

#[test]
fn test_read_root_inode() {
    let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
    let mut fs = Ext4Filesystem::<DummyHal, _>::new(device, FsConfig::default()).unwrap();

    // 读取根目录 inode (inode 2)
    let root = fs.inode_ref(2).unwrap();

    // 验证是目录
    let mode = root.mode();
    assert!(mode & 0o040000 != 0, "Root should be a directory");
}

#[test]
fn test_read_file() {
    let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
    let mut fs = Ext4Filesystem::<DummyHal, _>::new(device, FsConfig::default()).unwrap();

    // 假设 inode 12 是 file1.txt
    let file = fs.open_file("/file1.txt").unwrap();

    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();

    assert_eq!(buf, b"Hello, world!\n");
}

#[test]
fn test_list_directory() {
    let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
    let mut fs = Ext4Filesystem::<DummyHal, _>::new(device, FsConfig::default()).unwrap();

    let entries = fs.read_dir("/").unwrap();

    let names: Vec<_> = entries.iter().map(|e| e.name()).collect();
    assert!(names.contains(&"file1.txt"));
    assert!(names.contains(&"dir1"));
    assert!(names.contains(&"dir2"));
}
```

---

## 测试工作流

### 开发流程（推荐）⭐

#### 1. 实现函数
```rust
// lwext4_core/src/inode.rs
pub fn ext4_inode_get_size(sb: *const Ext4Superblock, inode: *const Ext4Inode) -> u64 {
    unsafe {
        let size_lo = u32::from_le((*inode).size_lo) as u64;
        let size_hi = u32::from_le((*inode).size_hi) as u64;
        (size_hi << 32) | size_lo
    }
}
```

#### 2. 写单元测试（lwext4_core）
```rust
// lwext4_core/tests/inode_tests.rs
#[test]
fn test_inode_get_size() {
    let sb = create_test_superblock();
    let inode = create_test_inode(0o100644, 12345);

    let size = ext4_inode_get_size(&sb, &inode);
    assert_eq!(size, 12345);
}
```

运行：
```bash
cd lwext4_core
cargo test test_inode_get_size
```

#### 3. 写集成测试（lwext4_arce）
```rust
// lwext4_arce/tests/integration_test.rs
#[test]
fn test_file_size() {
    let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
    let mut fs = Ext4Filesystem::<DummyHal, _>::new(device, FsConfig::default()).unwrap();

    let file = fs.open("/file1.txt").unwrap();
    assert_eq!(file.size(), 14);  // "Hello, world!\n"
}
```

运行：
```bash
cd lwext4_arce
cargo test test_file_size
```

#### 4. 迭代开发
- 失败 → 修改实现 → 重新测试
- 成功 → 继续下一个函数

---

## 测试项目结构

```
lwext4-rust/
├── test-images/                    # 测试镜像（提交到 git）
│   ├── test.ext4                   # 10MB 基础镜像
│   ├── small.ext4                  # 1MB 最小镜像
│   └── README.md                   # 镜像说明
│
├── lwext4_core/
│   ├── src/
│   │   └── ...
│   └── tests/
│       ├── common/
│       │   └── mod.rs              # 测试辅助工具
│       ├── inode_tests.rs          # inode 单元测试
│       ├── block_tests.rs          # block 单元测试
│       └── dir_tests.rs            # dir 单元测试
│
└── lwext4_arce/
    ├── src/
    │   └── ...
    └── tests/
        ├── common/
        │   └── mod.rs              # FileBlockDevice 实现
        └── integration_test.rs     # 集成测试
```

---

## 具体实施步骤

### 步骤 1: 准备测试环境（一次性）

```bash
# 1. 创建测试镜像
cd lwext4-rust
./scripts/create-test-images.sh  # 创建此脚本

# 2. 实现 FileBlockDevice
# 见上面的代码

# 3. 验证镜像可用
cargo test --package lwext4_arce test_open_filesystem
```

### 步骤 2: 实现第一个函数（示例：ext4_fs_init）

```rust
// lwext4_core/src/fs.rs
pub fn ext4_fs_init(
    fs: *mut Ext4Filesystem,
    bdev: *mut Ext4BlockDevice,
    read_only: bool,
) -> i32 {
    unsafe {
        // 1. 读取 superblock
        let mut sb_buf = [0u8; 1024];
        let rc = ext4_blocks_get_direct(
            bdev,
            sb_buf.as_mut_ptr() as *mut c_void,
            2,  // superblock 在块 2（偏移 1024）
            2,  // 读取 2 个 512 字节块
        );
        if rc != EOK {
            return rc;
        }

        // 2. 解析 superblock
        let sb = &*(sb_buf.as_ptr() as *const ext4_sblock);

        // 3. 验证魔数
        if u16::from_le(sb.magic) != 0xEF53 {
            return EINVAL as i32;
        }

        // 4. 填充 fs 结构
        (*fs).sb = *sb;
        (*fs).bdev = bdev;
        (*fs).read_only = read_only;

        EOK
    }
}
```

### 步骤 3: 测试

**单元测试**:
```rust
#[test]
fn test_fs_init() {
    // 使用 mock 数据测试
}
```

**集成测试**:
```rust
#[test]
fn test_fs_init_integration() {
    let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
    let fs = Ext4Filesystem::<DummyHal, _>::new(device, FsConfig::default()).unwrap();

    // fs_init 在 Ext4Filesystem::new 中被调用
    // 如果成功，说明 fs_init 工作正常
}
```

---

## 辅助脚本

### 创建测试镜像脚本

```bash
#!/bin/bash
# scripts/create-test-images.sh

set -e

IMAGES_DIR="test-images"
mkdir -p "$IMAGES_DIR"

echo "Creating test ext4 images..."

# 1. 基础测试镜像 (10MB)
echo "Creating test.ext4 (10MB)..."
dd if=/dev/zero of="$IMAGES_DIR/test.ext4" bs=1M count=10
mkfs.ext4 -F "$IMAGES_DIR/test.ext4"

mkdir -p "$IMAGES_DIR/mnt"
sudo mount "$IMAGES_DIR/test.ext4" "$IMAGES_DIR/mnt"

# 创建测试数据
sudo mkdir -p "$IMAGES_DIR/mnt/dir1" "$IMAGES_DIR/mnt/dir2"
echo "Hello, world!" | sudo tee "$IMAGES_DIR/mnt/file1.txt" > /dev/null
echo "Test data" | sudo tee "$IMAGES_DIR/mnt/dir1/file2.txt" > /dev/null
sudo dd if=/dev/zero of="$IMAGES_DIR/mnt/large.bin" bs=1K count=100 2>/dev/null

sudo umount "$IMAGES_DIR/mnt"

# 2. 最小测试镜像 (1MB)
echo "Creating small.ext4 (1MB)..."
dd if=/dev/zero of="$IMAGES_DIR/small.ext4" bs=1M count=1
mkfs.ext4 -F "$IMAGES_DIR/small.ext4"

# 清理
rmdir "$IMAGES_DIR/mnt"

echo "Test images created successfully!"
echo "Images:"
ls -lh "$IMAGES_DIR"/*.ext4
```

### 镜像信息查看脚本

```bash
#!/bin/bash
# scripts/inspect-image.sh

IMAGE="${1:-test-images/test.ext4}"

echo "=== Superblock info ==="
dumpe2fs "$IMAGE" 2>/dev/null | head -30

echo ""
echo "=== Directory listing ==="
debugfs "$IMAGE" -R "ls -l" 2>/dev/null

echo ""
echo "=== Root inode info ==="
debugfs "$IMAGE" -R "stat <2>" 2>/dev/null

echo ""
echo "=== Hex dump (first 2048 bytes) ==="
hexdump -C "$IMAGE" | head -128
```

---

## 总结

### ✅ 推荐方案（从简到繁）

#### 1. 快速开发：lwext4_core 单元测试
```rust
// 快速验证单个函数
#[test]
fn test_function() {
    let result = my_function(mock_data);
    assert_eq!(result, expected);
}
```

#### 2. 真实验证：lwext4_arce 集成测试 ⭐
```rust
// 使用真实镜像测试
let device = FileBlockDevice::from_test_image("test.ext4").unwrap();
let fs = Ext4Filesystem::new(device, config).unwrap();
// 测试实际功能
```

#### 3. 完整验证：arceos 集成
```bash
# 最后阶段
make run ARCH=riscv64
# 在 arceos 中测试
```

### 关键要素

1. ✅ **测试镜像**: 一次创建，重复使用（提交到 git）
2. ✅ **FileBlockDevice**: 简单但完整的测试用块设备
3. ✅ **分层测试**: 单元测试（快速）+ 集成测试（真实）
4. ✅ **辅助工具**: 测试数据生成、镜像检查脚本

### 工作流

```
实现函数 → 单元测试 → 通过 ✅
    ↓
集成测试 → 通过 ✅
    ↓
继续下一个函数
```

**这样测试既简单又有效！** 🎉
