# lwext4-rust 文档索引

本目录包含 lwext4-rust 项目的所有文档，按类型和用途分类组织。

## 📁 文档结构

```
docs/
├── guides/              # 📖 说明文档
├── design/              # 🏗️ 设计文档
├── development/         # 🔨 开发过程文档
└── testing/             # 🧪 测试文档
```

---

## 📖 说明文档 (guides/)

用户和开发者使用指南。

- **[项目总览](./guides/README.md)** - lwext4-rust 项目介绍
- **[Claude Code 使用说明](./guides/claude-usage.md)** - 如何使用 Claude Code 进行开发

---

## 🏗️ 设计文档 (design/)

架构设计、API 设计和模块设计文档。

### 架构设计 (architecture/)

- **[Rust 重新设计](./design/architecture/rust-redesign.md)** - 从 C 到 Rust 的重新设计
- **[API 设计](./design/architecture/api-design.md)** - Rust 惯用 API 设计原则

### 模块设计 (modules/)

- **[lwext4-core 模块](./design/modules/lwext4-core/)**
  - [README](./design/modules/lwext4-core/README.md) - 模块概述
  - [重构状态](./design/modules/lwext4-core/refactoring-status.md)
  - [实现计划](./design/modules/lwext4-core/implementation-plan.md)

---

## 🔨 开发过程文档 (development/)

开发过程中的状态记录、实现方案和迁移文档。

### 实现状态 (status/)

跟踪各模块的开发进度。

- **[总体进度](./development/status/overall-progress.md)** - 项目整体实现进度
- **[FS/Extent/Transaction 状态](./development/status/fs-extent-transaction-status.md)** - 文件系统核心模块状态
- **[目录 HTree 状态](./development/status/dir-htree-status.md)** - 目录索引实现状态

### 实现方案 (implementation/)

具体功能的实现方案和对比分析。

- **[HTree 分裂实现](./development/implementation/htree-split-implementation.md)** - 目录索引分裂算法实现
- **[目录实现对比](./development/implementation/dir-implementation-comparison.md)** - lwext4 C vs Rust 实现对比

### 迁移过程 (migration/)

从 C 代码到 Rust 的迁移过程记录。

- **[迁移文档索引](./development/migration/rust-implementation-migration/README.md)**
- **[步骤 1: 设计分析](./development/migration/rust-implementation-migration/step1-design-analysis/)**
- **[步骤 2: 类型系统修复](./development/migration/rust-implementation-migration/step2-type-system-fixes/)**
- **[步骤 3: 函数签名修复](./development/migration/rust-implementation-migration/step3-function-signature-fixes/)**
- **[步骤 4: 最终验证](./development/migration/rust-implementation-migration/step4-final-verification/)**

---

## 🧪 测试文档 (testing/)

测试策略、测试计划和测试报告。

### 测试策略

- **[测试策略](./testing/strategy.md)** - 整体测试策略和方法

### 测试计划 (plans/)

- **[HTree 分裂测试计划](./testing/plans/htree-split-test-plan.md)** - 目录索引分裂功能测试计划

### 测试报告 (reports/)

- **[代码覆盖率报告](./testing/reports/coverage-report.md)** - 测试覆盖率分析

---

## 🔍 查找文档

### 按开发阶段

- **需求分析阶段**: 查看 `design/` 目录
- **开发实现阶段**: 查看 `development/status/` 和 `development/implementation/`
- **测试阶段**: 查看 `testing/` 目录
- **迁移过程**: 查看 `development/migration/`

### 按文档类型

- **说明类**: `guides/`
- **设计类**: `design/`
- **状态类**: `development/status/`
- **方案类**: `development/implementation/`
- **过程类**: `development/migration/`
- **测试类**: `testing/`

---

## 📝 文档维护

### 添加新文档

1. 确定文档类型（说明/设计/开发/测试）
2. 放置到相应目录
3. 更新本索引文件
4. 使用有意义的文件名（kebab-case）

### 文档命名规范

- 使用小写字母和连字符
- 英文命名，简洁明确
- 例如：`htree-split-implementation.md`

### 目录规范

- 按文档类型分类
- 同类文档按主题分组
- 保持目录结构扁平（避免过深嵌套）

---

## 📊 文档统计

- 📖 说明文档: 2 份
- 🏗️ 设计文档: 5 份
- 🔨 开发文档: 30+ 份
- 🧪 测试文档: 3 份

**总计**: 40+ 份文档

---

最后更新: 2025-12-16
