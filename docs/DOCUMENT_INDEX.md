# lwext4-rust 文档索引

本文档提供项目所有文档的完整索引，帮助快速定位所需信息。

## 📋 目录结构

```
docs/
├── design/           # 设计文档
├── development/      # 开发文档
├── guides/          # 使用指南
├── testing/         # 测试文档
└── tools/           # 工具文档
```

---

## 🎨 设计文档 (design/)

### 架构设计 (architecture/)
- [API 设计](design/architecture/api-design.md) - Rust API 设计原则和接口定义
- [Rust 重新设计](design/architecture/rust-redesign.md) - 从 C 到 Rust 的架构重新设计

### 模块设计 (modules/)
- [lwext4-core 实现计划](design/modules/lwext4-core/implementation-plan.md) - 核心模块实现计划
- [重构状态](design/modules/lwext4-core/refactoring-status.md) - 重构进度追踪
- [模块说明](design/modules/lwext4-core/README.md) - lwext4-core 模块概述

---

## 🔧 开发文档 (development/)

### 修复记录 (fixes/)
- [修复摘要](development/fixes/FIXES_SUMMARY.md) - 所有模块修复的汇总
- [Journal 修复完成](development/fixes/JOURNAL_FIX_COMPLETE.md) - Journal 模块修复详情

### 实现文档 (implementation/)
- [目录实现对比](development/implementation/dir-implementation-comparison.md) - C vs Rust 目录操作实现对比
- [Extent 基础设施分析](development/implementation/extent-infrastructure-analysis.md) - Extent 树实现分析
- [HTree 分裂实现](development/implementation/htree-split-implementation.md) - HTree 目录索引分裂算法

### 迁移文档 (migration/)

#### Rust 实现迁移主目录
- [迁移概述](development/migration/rust-implementation-migration/README.md) - 迁移项目总览
- [文档索引](development/migration/rust-implementation-migration/DOCUMENT_INDEX.md) - 迁移相关文档索引
- [C 接口必要性分析](development/migration/rust-implementation-migration/C_INTERFACE_NECESSITY_ANALYSIS.md) - 是否需要保留 C 接口
- [编译 vs 功能](development/migration/rust-implementation-migration/COMPILATION_VS_FUNCTIONALITY.md) - 编译成功与功能实现的区别

#### 第一步：设计分析 (step1-design-analysis/)
- [概述](development/migration/rust-implementation-migration/step1-design-analysis/README.md)
- [零修改可行性](development/migration/rust-implementation-migration/step1-design-analysis/ZERO_MODIFICATION_FEASIBILITY.md) - C 代码零修改迁移评估
- [结构映射](development/migration/rust-implementation-migration/step1-design-analysis/C_TO_RUST_STRUCTURE_MAPPING.md) - C 到 Rust 结构体映射
- [接口兼容性分析](development/migration/rust-implementation-migration/step1-design-analysis/INTERFACE_COMPATIBILITY_ANALYSIS.md) - 接口层面的兼容性
- [两种方法对比](development/migration/rust-implementation-migration/step1-design-analysis/TWO_APPROACHES_COMPARISON.md) - 不同迁移策略的对比
- [修订设计原则](development/migration/rust-implementation-migration/step1-design-analysis/REVISED_DESIGN_PRINCIPLES.md) - 最终确定的设计原则
- [最终实现计划](development/migration/rust-implementation-migration/step1-design-analysis/FINAL_IMPLEMENTATION_PLAN.md) - 确定的实施方案

#### 第二步：类型系统修复 (step2-type-system-fixes/)
- [概述](development/migration/rust-implementation-migration/step2-type-system-fixes/README.md)
- [ARCE 错误分析](development/migration/rust-implementation-migration/step2-type-system-fixes/ARCE_ERROR_ANALYSIS.md) - lwext4_arce 编译错误分析
- [ARCE 适配计划](development/migration/rust-implementation-migration/step2-type-system-fixes/ARCE_ADAPTATION_PLAN.md) - 适配方案
- [P0 修复结果](development/migration/rust-implementation-migration/step2-type-system-fixes/P0_FIX_RESULTS.md) - 高优先级问题修复结果

#### 第三步：函数签名修复 (step3-function-signature-fixes/)
- [概述](development/migration/rust-implementation-migration/step3-function-signature-fixes/README.md)
- [修订实现摘要](development/migration/rust-implementation-migration/step3-function-signature-fixes/REVISED_IMPLEMENTATION_SUMMARY.md) - 修改后的实现总结
- [会话进度摘要](development/migration/rust-implementation-migration/step3-function-signature-fixes/SESSION_PROGRESS_SUMMARY.md) - 开发会话记录

#### 第四步：最终验证 (step4-final-verification/)
- [概述](development/migration/rust-implementation-migration/step4-final-verification/README.md)
- [当前状态](development/migration/rust-implementation-migration/step4-final-verification/CURRENT_STATUS.md) - 最新进展
- [测试覆盖报告](development/migration/rust-implementation-migration/step4-final-verification/COVERAGE_TEST_REPORT.md) - 测试覆盖情况
- [最终成功摘要](development/migration/rust-implementation-migration/step4-final-verification/FINAL_SUCCESS_SUMMARY.md) - 迁移成功总结

### 模块开发 (modules/)

#### 分析文档 (analysis/)
- [Cargo 检查错误分析](development/modules/analysis/CARGO_CHECK_ERROR_ANALYSIS.md) - 编译错误诊断
- [其他模块分析](development/modules/analysis/OTHER_MODULES_ANALYSIS.md) - 辅助模块分析

#### Journal 模块 (journal/)
- [实现完成](development/modules/journal/IMPLEMENTATION_COMPLETE.md) - Journal (JBD2) 模块实现完成文档

#### Xattr 模块 (xattr/)
- [实现准备评估](development/modules/xattr/XATTR_IMPLEMENTATION_READINESS.md) - xattr 模块实现条件分析

### 状态追踪 (status/)
- [整体进度](development/status/overall-progress.md) - 项目整体开发进度
- [目录和 HTree 状态](development/status/dir-htree-status.md) - 目录模块开发状态
- [文件系统、Extent 和事务状态](development/status/fs-extent-transaction-status.md) - 核心模块状态

---

## 📖 使用指南 (guides/)

- [指南概览](guides/README.md) - 所有指南的索引
- [Claude 使用指南](guides/claude-usage.md) - 如何使用 Claude 辅助开发本项目

---

## 🧪 测试文档 (testing/)

### 测试策略
- [测试策略](testing/strategy.md) - 整体测试方法和原则

### 测试计划 (plans/)
- [HTree 分裂测试计划](testing/plans/htree-split-test-plan.md) - HTree 目录索引分裂功能的测试计划

### 测试报告 (reports/)
- [覆盖率报告](testing/reports/coverage-report.md) - 测试覆盖率分析报告

---

## 🛠️ 工具文档 (tools/)

- [工具说明](tools/tools-readme.md) - 项目工具概览
- [Commit 重写指南](tools/COMMIT_REWORD_GUIDE.md) - Git commit 信息改写工具使用指南

---

## 📑 快速导航

### 我想了解...

#### 项目架构和设计
- 从 [Rust 重新设计](design/architecture/rust-redesign.md) 开始
- 查看 [API 设计](design/architecture/api-design.md) 了解接口设计

#### 如何参与开发
- 阅读 [迁移概述](development/migration/rust-implementation-migration/README.md)
- 查看 [整体进度](development/status/overall-progress.md) 了解当前状态
- 参考 [Claude 使用指南](guides/claude-usage.md) 提高开发效率

#### 某个模块的实现细节
- **Journal**: [实现完成文档](development/modules/journal/IMPLEMENTATION_COMPLETE.md)
- **Xattr**: [实现准备评估](development/modules/xattr/XATTR_IMPLEMENTATION_READINESS.md)
- **目录操作**: [目录实现对比](development/implementation/dir-implementation-comparison.md)
- **Extent 树**: [Extent 基础设施分析](development/implementation/extent-infrastructure-analysis.md)

#### 测试和验证
- [测试策略](testing/strategy.md) - 了解测试方法
- [覆盖率报告](testing/reports/coverage-report.md) - 查看测试覆盖情况

#### 使用工具
- [工具说明](tools/tools-readme.md) - 查看可用工具
- [Commit 重写指南](tools/COMMIT_REWORD_GUIDE.md) - 使用 reword 工具

---

## 📝 文档维护

### 添加新文档
新文档应按照以下规则分类：

- **设计文档** → `docs/design/`
  - 架构设计 → `architecture/`
  - 模块设计 → `modules/`

- **开发文档** → `docs/development/`
  - 修复记录 → `fixes/`
  - 实现文档 → `implementation/`
  - 迁移记录 → `migration/`
  - 模块开发 → `modules/<模块名>/`
  - 状态追踪 → `status/`

- **指南文档** → `docs/guides/`

- **测试文档** → `docs/testing/`
  - 测试计划 → `plans/`
  - 测试报告 → `reports/`

- **工具文档** → `docs/tools/`

### 更新索引
添加新文档后，请更新本索引文件。

---

## 🔗 相关链接

- [项目主 README](../README.md)
- [lwext4 C 库](https://github.com/gkostka/lwext4)
- [Rust 官方文档](https://doc.rust-lang.org/)

---

最后更新：2025-12-17
