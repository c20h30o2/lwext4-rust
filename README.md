# lwext4-rust

Rust implementation of lwext4 (ext2/3/4 filesystem library)

## 项目结构

```
lwext4-rust/
├── lwext4_core/        # 核心 Rust 实现
├── lwext4_arce/        # C API 兼容层
├── docs/               # 📚 所有文档
└── lwext4/             # 原始 C 代码（参考）
```

## 快速开始

```bash
# 构建项目
cargo build

# 运行测试
cargo test

# 运行示例
cargo run --example extent_demo
```

## 文档

所有项目文档位于 [`docs/`](./docs/) 目录：

- 📖 [说明文档](./docs/guides/) - 使用指南
- 🏗️ [设计文档](./docs/design/) - 架构和 API 设计
- 🔨 [开发文档](./docs/development/) - 实现状态和方案
- 🧪 [测试文档](./docs/testing/) - 测试策略和报告

详见 [文档索引](./docs/README.md)

## 特性

- ✅ 完整的 Rust 实现（no_std 支持）
- ✅ 类型安全的块设备抽象
- ✅ Extent 树完整支持（100%）
- ✅ 目录索引（HTree）支持
- ✅ 事务和日志支持
- ⏳ C API 兼容层（进行中）

## 许可证

See [LICENSE](./LICENSE)
