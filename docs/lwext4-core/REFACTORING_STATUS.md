# lwext4-rust 重构状态报告

日期：2025-12-07
分支：`refactor/rust-idiomatic-core`

## 概述

本文档记录了 lwext4-rust 项目从 C FFI 到纯 Rust 实现的重构进度。

## 重构目标

1. **lwext4_core**: 将所有功能重构为纯 Rust 实现，采用 Rust 惯用风格
   - 零 unsafe 块（除必要的结构体定义）
   - Result-based 错误处理
   - 泛型编程（Generic over BlockDevice trait）
   - 完整的文档和类型安全

2. **lwext4_arce**: 适配新的 lwext4_core API，保持对外接口不变

## 已完成工作

### lwext4_core

#### ✅ Phase 1: 架构设计与类型定义 (已提交)
- [x] 创建设计文档 `docs/lwext4-core/RUST_REDESIGN.md`
- [x] 创建 `traits.rs` - 定义 BlockDevice trait
- [x] 更新 `types.rs` - 添加 `Ext4BlockDev<D>` 泛型结构
- [x] 修复编译错误（重复定义、类型别名等）

#### ✅ Phase 2: Block 操作重构 (已提交)
- [x] 完全重写 `block.rs`
  - `ext4_blocks_get_direct` - 直接读取块
  - `ext4_blocks_set_direct` - 直接写入块
  - `ext4_block_readbytes` - 字节级读取（支持跨块）
  - `ext4_block_writebytes` - 字节级写入（支持跨块）
  - `ext4_block_cache_flush` - 缓存刷新
- [x] 添加完整文档和函数包装
- [x] 实现方法 + 自由函数双接口
- [x] 零 unsafe 块

#### ✅ Phase 3: 集成测试 (已提交)
- [x] 创建 `lwext4_core/tests/integration_test.rs`
  - `test_block_device_creation` - 块设备创建测试
  - `test_block_read_write` - 块读写测试
  - `test_byte_level_read` - 字节级读取测试
  - `test_statistics` - 统计计数器测试
- [x] 添加缺失的 getter 方法
  - `lg_bcnt()` - 获取逻辑块数量
  - `ph_bcnt()` - 获取物理块数量
- [x] 所有测试通过 (4/4)

### lwext4_arce

#### ✅ Phase 1: 基础适配 (当前进度)
- [x] 切换默认特性为 `use-rust`
- [x] 重写 `error.rs` - 兼容 lwext4_core 错误类型
- [x] 重写 `blockdev.rs` - 使用 `Ext4BlockDev<D>`
- [x] 更新 `tests/common/mod.rs` - 适配新 BlockDevice trait

## 未完成工作

### lwext4_core

#### ⏸️ Phase 4: Filesystem 操作重构 (未开始)
- [ ] 重构 `fs.rs`
  - ext4_fs_init - 文件系统初始化
  - ext4_fs_fini - 文件系统清理
  - ext4_mount - 挂载文件系统
  - ext4_umount - 卸载文件系统
- [ ] 移除 C FFI 依赖
  - ext4_bcache_init_dynamic
  - ext4_bcache_fini_dynamic
  - ext4_block_bind_bcache
  - ext4_bcache_cleanup

#### ⏸️ Phase 5: Inode 操作重构 (未开始)
- [ ] 重构 `inode.rs`
  - ext4_fs_get_inode_ref - 获取 inode 引用
  - ext4_fs_put_inode_ref - 释放 inode 引用
  - ext4_fs_inode_blocks_init - 初始化 inode 块
  - ext4_fs_alloc_inode - 分配 inode
  - ext4_fs_free_inode - 释放 inode
  - ext4_fs_truncate_inode - 截断 inode

#### ⏸️ Phase 6: Directory 操作重构 (未开始)
- [ ] 重构 `dir.rs`
  - ext4_dir_find_entry - 查找目录项
  - ext4_dir_add_entry - 添加目录项
  - ext4_dir_remove_entry - 删除目录项
  - ext4_dir_iterator_init - 初始化目录迭代器
  - ext4_dir_iterator_next - 迭代下一项

### lwext4_arce

#### ⏸️ Phase 2: Filesystem 模块适配 (受阻 - 等待 lwext4_core Phase 4)
- [ ] 重构 `fs.rs` 以使用新 API
  - `Ext4Filesystem::new()` - 需要 lwext4_core 的 fs 实现
  - `mount()/umount()` - 需要 lwext4_core 的 mount 实现
- [ ] 当前问题：
  - `bdev.inner.as_mut()` - Ext4BlockDev 不提供此方法
  - C FFI 函数不存在（ext4_bcache_*, ext4_block_bind_bcache 等）

#### ⏸️ Phase 3: Inode 模块适配 (受阻 - 等待 lwext4_core Phase 5)
- [ ] 重构 `inode/file.rs`
  - `read_bytes()/write_bytes()` - 签名已更改
  - `ext4_block_readbytes(bdev, offset, buf, len)` → `ext4_block_readbytes(bdev, offset, buf)`
- [ ] 重构 `inode/dir.rs`
- [ ] 当前问题：
  - 函数签名不匹配（参数数量和类型）
  - 需要 &mut Ext4BlockDev<D> 而不是原始指针

## 阻塞问题

### 主要阻塞

**lwext4_arce 的 fs.rs 和 inode 模块无法编译**

**原因**: lwext4_core 只完成了 block 操作的重构，fs/inode/dir 模块仍是占位实现，未提供 lwext4_arce 所需的功能。

**影响**:
- lwext4_arce 无法完全切换到 `use-rust` 模式
- 只有 blockdev.rs 可以使用新 API
- fs.rs 和 inode 模块仍需要 C FFI

### 次要问题

1. **类型不兼容**:
   - C 风格原始指针 vs Rust 引用
   - `ext4_blockdev*` (C) vs `Ext4BlockDev<D>` (Rust)

2. **函数签名变更**:
   - 参数数量和类型不同
   - 返回值类型不同 (i32 vs Result<T>)

## 后续步骤建议

### 选项 1: 顺序重构 (推荐)

按模块顺序完成 lwext4_core 重构，然后逐步适配 lwext4_arce：

1. **完成 lwext4_core Phase 4** (filesystem 操作)
   - 实现 ext4_fs_init/fini
   - 实现 mount/umount
   - 实现 superblock 解析

2. **适配 lwext4_arce fs.rs**
   - 使用新的 lwext4_core fs API

3. **完成 lwext4_core Phase 5** (inode 操作)
   - 实现 inode 引用管理
   - 实现 inode 分配/释放

4. **适配 lwext4_arce inode 模块**
   - 使用新的 lwext4_core inode API

5. **完成 lwext4_core Phase 6** (directory 操作)
   - 实现目录项操作
   - 实现目录迭代器

6. **最终测试与集成**
   - 端到端测试
   - 性能基准测试

**优点**: 逐步推进，风险可控
**缺点**: 耗时较长

### 选项 2: 双模式并存 (临时方案)

保持 lwext4_arce 同时支持 use-ffi 和 use-rust 两种模式：

1. blockdev.rs 使用 use-rust (已完成)
2. fs.rs, inode 暂时保持 use-ffi
3. 逐步迁移功能模块

**优点**: 可以部分使用新 API
**缺点**: 代码复杂度增加，维护困难

### 选项 3: 最小可行重构

只重构核心功能，其他功能暂不支持：

1. lwext4_core 只实现 block + superblock 读取
2. lwext4_arce 只提供只读文件系统功能
3. 写入、inode 管理等功能后续添加

**优点**: 可以快速验证架构可行性
**缺点**: 功能不完整

## 技术债务

1. **lwext4_core 中的占位实现**:
   - fs.rs 中的函数只返回 0 或占位值
   - inode.rs 中的函数未实现
   - dir.rs 中的函数未实现

2. **测试覆盖率**:
   - 只有 block 操作有集成测试
   - 缺少 fs/inode/dir 测试

3. **文档**:
   - 需要为所有新 API 添加示例
   - 需要迁移指南

## 估算工作量

基于当前进度（Phase 1-3 完成），估算剩余工作量：

- **lwext4_core Phase 4 (fs)**: 5-7 天
- **lwext4_core Phase 5 (inode)**: 7-10 天
- **lwext4_core Phase 6 (dir)**: 5-7 天
- **lwext4_arce 完整适配**: 3-5 天
- **测试与文档**: 3-5 天

**总计**: 约 23-34 天 (1 个月左右)

## 成功标准

- [ ] lwext4_core 所有模块使用纯 Rust 实现
- [ ] 零 unsafe 块（除必要的结构体定义）
- [ ] lwext4_arce 使用 use-rust 模式编译通过
- [ ] 所有集成测试通过
- [ ] 性能与 C FFI 版本相当
- [ ] 完整的 API 文档

## Git 提交历史

1. `feat: create rust-idiomatic refactoring design doc` (Phase 1)
2. `refactor: add BlockDevice trait and generic Ext4BlockDev` (Phase 1)
3. `refactor: rewrite block.rs in pure Rust with zero unsafe` (Phase 2)
4. `feat: add integration tests and missing getter methods` (Phase 3)

## 参考资料

- [lwext4 C 实现](https://github.com/gkostka/lwext4)
- [Rust 设计文档](docs/lwext4-core/RUST_REDESIGN.md)
- [ext4 文件系统规范](https://ext4.wiki.kernel.org/)
