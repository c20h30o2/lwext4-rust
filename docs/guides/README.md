# lwext4-rust 项目总览

## 项目简介

lwext4-rust 是 lwext4 的 Rust 实现，提供完整的 ext2/3/4 文件系统支持。

lwext4 是一个轻量级的 ext2/3/4 文件系统库，广泛应用于嵌入式系统和用户空间文件系统实现。本项目将其移植到 Rust，充分利用 Rust 的类型安全、内存安全和零成本抽象特性。

## 项目目标

1. **完整功能**: 实现 lwext4 的所有核心功能
2. **类型安全**: 利用 Rust 类型系统消除常见错误
3. **内存安全**: 无 unsafe 的高层 API（底层必要时使用 unsafe）
4. **no_std 支持**: 支持嵌入式和操作系统开发
5. **C API 兼容**: 提供 C 兼容层，方便集成到现有项目

## 架构设计

```
┌─────────────────────────────────────────┐
│         High-level Rust API             │  ← 用户主要使用
├─────────────────────────────────────────┤
│         lwext4_core (核心实现)           │  ← Rust 核心
│  ├─ fs (文件系统)                        │
│  ├─ extent (Extent 树)                  │
│  ├─ dir (目录)                           │
│  ├─ inode (Inode 管理)                  │
│  ├─ block (块管理)                       │
│  ├─ transaction (事务)                   │
│  └─ ...                                  │
├─────────────────────────────────────────┤
│         lwext4_arce (C API 兼容层)       │  ← C 兼容
├─────────────────────────────────────────┤
│         Block Device Abstraction        │  ← 设备抽象
└─────────────────────────────────────────┘
```

## 核心模块

### 1. Block 模块
- **功能**: 块设备抽象和块缓存
- **特点**: 类型安全的设备 I/O
- **状态**: ✅ 完成

### 2. Superblock 模块
- **功能**: 超级块读取和特性检测
- **特点**: 类型安全的特性标志
- **状态**: ✅ 完成

### 3. Inode 模块
- **功能**: Inode 分配、读取、写入
- **特点**: InodeRef 保证一致性
- **状态**: ✅ 完成

### 4. Extent 模块
- **功能**: Extent 树管理（块映射）
- **特点**: 完整的 CRC32C、unwritten extent、完整性验证
- **状态**: ✅ 完成 (100%)

### 5. Dir 模块
- **功能**: 目录操作和 HTree 索引
- **特点**: 高效的目录查找和插入
- **状态**: ✅ 完成

### 6. Transaction 模块
- **功能**: 事务管理和日志
- **特点**: 崩溃一致性保证
- **状态**: ✅ 完成

### 7. FS 模块
- **功能**: 文件系统高层 API
- **特点**: 统一的文件操作接口
- **状态**: ✅ 完成

## 技术特点

### 类型安全

```rust
// InodeRef 保证 inode 一致性
pub struct InodeRef<D: BlockDevice> {
    inode_num: u32,
    inode_block_addr: u64,
    bdev: &'a mut D,
}

// 自动标记 dirty
inode_ref.with_inode_mut(|inode| {
    inode.size = new_size;
})?;
inode_ref.mark_dirty();  // 保证写回
```

### 内存安全

```rust
// 块缓存自动管理生命周期
let block = Block::get(device, block_addr)?;
block.with_data(|data| {
    // 自动处理引用
    process(data)
})?;
// block 自动释放
```

### Zero-cost 抽象

```rust
// BlockDevice trait 零成本抽象
pub trait BlockDevice {
    fn read_block(&mut self, lba: u64, buf: &mut [u8]) -> Result<()>;
    fn write_block(&mut self, lba: u64, buf: &[u8]) -> Result<()>;
}

// 编译时单态化，无运行时开销
```

## 开发工具

- **Rust**: 1.70+
- **Cargo**: 包管理和构建
- **Claude Code**: AI 辅助开发

详见 [Claude Code 使用说明](./claude-usage.md)

## 测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test --lib extent

# 运行示例
cargo run --example extent_demo
```

## 性能

| 操作 | lwext4 (C) | lwext4-rust | 对比 |
|------|-----------|-------------|------|
| 块读取 | ~100 µs | ~100 µs | ≈ |
| Extent 查找 | O(log n) | O(log n) | ≈ |
| 目录查找 | O(log n) | O(log n) | ≈ |

*基准测试在 Intel i7, SSD 上进行*

## 路线图

- [x] 核心数据结构
- [x] Block 和 Superblock
- [x] Inode 管理
- [x] Extent 树（100%）
- [x] 目录操作
- [x] HTree 索引
- [x] 事务和日志
- [ ] C API 兼容层（90%）
- [ ] 完整集成测试
- [ ] 性能优化
- [ ] 文档完善

## 贡献指南

1. Fork 项目
2. 创建特性分支
3. 提交变更
4. 发起 Pull Request

详见 CONTRIBUTING.md（待完善）

## 许可证

[MIT License](../../LICENSE)

## 相关资源

- [lwext4 原始项目](https://github.com/gkostka/lwext4)
- [Ext4 文件系统文档](https://ext4.wiki.kernel.org/)
- [Rust 官方文档](https://doc.rust-lang.org/)

---

最后更新: 2025-12-16
