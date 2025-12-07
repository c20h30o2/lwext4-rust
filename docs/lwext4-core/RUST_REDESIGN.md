# lwext4_core Rust 重构设计文档

## 重构目标

将 lwext4_core 从"C 风格源码级兼容"转变为"纯 Rust 惯用实现"，同时保持功能完整性。

## 设计原则

### 1. 命名约定
- **保留 C 风格命名**：`ext4_blocks_get_direct`、`ext4_inode_get_size` 等
- **目的**：便于对照 lwext4 C 代码查看实现，跟踪剩余未实现函数

### 2. 功能范围
- **仅实现 lwext4 已有功能**
- **未来扩展功能**：使用占位实现 + `TODO` 标记

### 3. Rust 特性采用
- **类型安全**：用引用和借用替代裸指针
- **错误处理**：用 `Result<T, E>` 替代错误码
- **所有权**：充分利用 Rust 所有权系统
- **生命周期**：明确生命周期标注
- **Option**：替代空指针检查
- **减少 unsafe**：仅在必要时使用

## 核心类型重构

### 1. BlockDevice Trait（灵活方案）

```rust
pub trait BlockDevice {
    /// 获取块大小（字节）
    fn block_size(&self) -> u32;

    /// 获取物理块大小（字节）
    fn physical_block_size(&self) -> u32;

    /// 获取总块数
    fn total_blocks(&self) -> u64;

    /// 读取多个块
    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<usize, Ext4Error>;

    /// 写入多个块
    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<usize, Ext4Error>;

    /// 刷新缓存（可选）
    fn flush(&mut self) -> Result<(), Ext4Error> {
        Ok(())
    }
}
```

### 2. 错误类型

```rust
#[derive(Debug, Clone)]
pub struct Ext4Error {
    code: i32,
    message: &'static str,
}

impl Ext4Error {
    pub fn new(code: i32, message: &'static str) -> Self {
        Self { code, message }
    }

    pub fn code(&self) -> i32 {
        self.code
    }
}

pub type Ext4Result<T> = Result<T, Ext4Error>;
```

### 3. 核心结构体重构

#### 之前（C 风格）：
```rust
#[repr(C)]
pub struct ext4_blockdev {
    pub bdif: *mut ext4_blockdev_iface,
    pub part_offset: u64,
    pub lg_bsize: u32,
    // ...
}
```

#### 之后（Rust 风格）：
```rust
pub struct Ext4BlockDev<D: BlockDevice> {
    device: D,
    part_offset: u64,
    lg_bsize: u32,
    ph_bsize: u32,
    ph_bcnt: u64,
    cache: Option<Box<Ext4BlockCache>>,
}
```

### 4. 函数签名重构

#### 之前（C 风格）：
```rust
pub fn ext4_blocks_get_direct(
    bdev: *mut Ext4BlockDevice,
    buf: *mut core::ffi::c_void,
    lba: u64,
    cnt: u32,
) -> i32
```

#### 之后（Rust 风格）：
```rust
pub fn ext4_blocks_get_direct<D: BlockDevice>(
    bdev: &mut Ext4BlockDev<D>,
    lba: u64,
    buf: &mut [u8],
) -> Result<usize, Ext4Error>
```

## 重构顺序

### Phase 1: 基础类型和 Trait（优先级 P0）
1. ✅ 创建新分支 `refactor/rust-idiomatic-core`
2. 定义 `BlockDevice` trait
3. 重构 `Ext4Error` 和 `Ext4Result`
4. 重构常量定义（consts.rs）

### Phase 2: 块设备层（优先级 P0）
1. 重构 `Ext4BlockDev<D>` 结构体
2. 重构 `ext4_blocks_get_direct` / `ext4_blocks_set_direct`
3. 重构块缓存相关函数
4. 更新测试

### Phase 3: Superblock 和 Inode（优先级 P0）
1. 重构 `Ext4Superblock` 结构体
2. 重构 `Ext4Inode` 和 `Ext4InodeRef`
3. 重构 inode 读取函数
4. 更新测试

### Phase 4: 目录和文件操作（优先级 P1）
1. 重构 `Ext4DirEntry` 和 `Ext4DirIterator`
2. 重构目录查找和遍历
3. 重构文件读写操作
4. 更新测试

### Phase 5: 文件系统操作（优先级 P1）
1. 重构 `Ext4Filesystem<D>` 结构体
2. 重构挂载/卸载操作
3. 重构块映射函数
4. 更新测试

### Phase 6: lwext4_arce 适配（优先级 P1）
1. 更新 lwext4_arce 以使用新的 lwext4_core API
2. 保持对外接口不变
3. 更新集成测试
4. 文档更新

## 迁移策略

### 渐进式重构
- **每次重构一个模块**
- **确保每步都能编译**
- **每个模块重构后立即编写测试**

### 兼容性考虑
- lwext4_arce 的对外 API 保持不变
- 仅内部实现改变

## 示例：block.rs 重构

### 重构前：
```rust
pub fn ext4_bdif_lock(bdev: *mut Ext4BlockDevice) {
    unsafe {
        if (*(*bdev).bdif).lock.is_none() {
            return;
        }
        if let Some(lock_fn) = (*(*bdev).bdif).lock {
            let r = lock_fn(bdev);
            debug_assert_eq!(r, EOK);
        }
    }
}
```

### 重构后：
```rust
// 由于使用 Rust 的 &mut 借用，自动保证互斥访问
// 如果需要多线程，使用 Mutex<Ext4BlockDev<D>>
// 因此不需要显式的 lock/unlock 函数
```

### 重构前：
```rust
pub fn ext4_blocks_get_direct(
    bdev: *mut Ext4BlockDevice,
    buf: *mut core::ffi::c_void,
    lba: u64,
    cnt: u32,
) -> i32 {
    unsafe {
        let pba = (lba * (*bdev).lg_bsize as u64 + (*bdev).part_offset)
                  / (*(*bdev).bdif).ph_bsize as u64;
        let pb_cnt = ((*bdev).lg_bsize / (*(*bdev).bdif).ph_bsize) as u32;
        ext4_bdif_bread(bdev, buf, pba, pb_cnt * cnt)
    }
}
```

### 重构后：
```rust
pub fn ext4_blocks_get_direct<D: BlockDevice>(
    bdev: &mut Ext4BlockDev<D>,
    lba: u64,
    buf: &mut [u8],
) -> Result<usize, Ext4Error> {
    let pba = (lba * bdev.lg_bsize as u64 + bdev.part_offset) / bdev.ph_bsize as u64;
    let pb_cnt = (bdev.lg_bsize / bdev.ph_bsize) as u32;

    // 检查 buffer 大小
    let required_size = (pb_cnt * bdev.ph_bsize) as usize;
    if buf.len() < required_size {
        return Err(Ext4Error::new(EINVAL, "buffer too small"));
    }

    bdev.device.read_blocks(pba, pb_cnt, buf)
}
```

## 关键改进点

### 1. 类型安全
- ✅ 编译时检查空指针
- ✅ 自动的生命周期管理
- ✅ 借用检查防止数据竞争

### 2. 错误处理
- ✅ 明确的错误传播（`?` 操作符）
- ✅ 类型安全的错误类型
- ✅ 无需手动检查错误码

### 3. 内存安全
- ✅ 无 use-after-free
- ✅ 无 double-free
- ✅ 无缓冲区溢出

### 4. 可维护性
- ✅ 更清晰的代码结构
- ✅ 更好的文档注释
- ✅ 更容易测试

## 性能考虑

### Zero-Cost Abstractions
- Rust 的抽象不引入运行时开销
- 泛型在编译时单态化
- 内联优化消除函数调用开销

### 与 C 版本性能对比
- **理论**：性能相当或更好
- **实践**：需要 benchmark 验证

## 测试策略

### 单元测试
- 每个重构的函数都有对应的单元测试
- 使用 mock BlockDevice 进行测试

### 集成测试
- 使用真实 ext4 镜像测试
- 对比 lwext4 C 版本的行为

### 性能测试
- Benchmark 关键操作
- 确保性能不退化

## 文档要求

### 代码注释
- 每个 pub 函数都有文档注释
- 说明参数、返回值、错误情况

### 示例代码
- 常用操作提供示例

### 迁移指南
- 为 lwext4_arce 提供迁移指南

## 时间线估计

- **Phase 1**: 1 天
- **Phase 2**: 2-3 天
- **Phase 3**: 2-3 天
- **Phase 4**: 2-3 天
- **Phase 5**: 2-3 天
- **Phase 6**: 1-2 天

**总计**: 约 10-15 天（根据实际进度调整）

## 成功标准

1. ✅ 所有测试通过
2. ✅ 无 unsafe 代码（或最小化）
3. ✅ lwext4_arce 对外接口保持不变
4. ✅ 性能不低于 C 风格实现
5. ✅ 代码覆盖率 > 80%
6. ✅ 文档完整

---

**创建时间**: 2025-12-07
**分支**: `refactor/rust-idiomatic-core`
**状态**: Phase 1 进行中
