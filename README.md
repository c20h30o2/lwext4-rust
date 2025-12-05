# lwext4-rust 重构项目

纯 Rust 实现的 ext4 文件系统，用于 arceos 操作系统。

## 项目结构

```
lwext4-rust/
├── lwext4_rust/       # 现有的 C FFI 版本（临时依赖）
├── lwext4_arce/       # arceos 适配层
│   └── 依赖 lwext4_rust，实现 axfs-ng-vfs 接口
└── lwext4_core/       # 纯 Rust 核心实现 ✨ 当前开发重点
    ├── Cargo.toml
    ├── IMPLEMENTATION_PLAN.md  # 实现计划
    └── src/
        ├── lib.rs          # 主入口
        ├── consts.rs       # 常量定义
        ├── types.rs        # 数据结构
        ├── error.rs        # 错误处理
        ├── superblock.rs   # Superblock 操作
        ├── inode.rs        # Inode 操作
        ├── block.rs        # 块操作
        ├── dir.rs          # 目录操作
        └── fs.rs           # 文件系统核心
```

## 当前状态

✅ **阶段 0：框架搭建**（已完成）
- 创建模块结构
- 定义 36 个占位函数
- 编译通过

⬜ **阶段 1：只读功能**（进行中）
- Superblock 读取
- Inode 读取
- 文件读取
- 目录遍历

⬜ **阶段 2：写入功能**（计划中）

⬜ **阶段 3：缓存优化**（计划中）

## 快速开始

### 编译 lwext4-core

```bash
cd lwext4_core
cargo build
```

### 运行测试

```bash
cargo test
```

### 代码统计

```bash
find lwext4_core/src -name "*.rs" | xargs wc -l
# 当前：~715 行
# 目标：~1200 行（最小实现）
```

## 开发路线

详见 [lwext4_core/IMPLEMENTATION_PLAN.md](lwext4_core/IMPLEMENTATION_PLAN.md)

## 贡献

请在实现每个功能后更新 IMPLEMENTATION_PLAN.md 中的复选框。

