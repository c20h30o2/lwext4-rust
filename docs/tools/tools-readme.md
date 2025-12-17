# Tools 工具集

本目录包含 lwext4-rust 项目的辅助工具脚本。

## 工具列表

### 1. cleanup_git_ignored.sh

**用途**: 清理 git 中被追踪但应该被 .gitignore 忽略的文件

**功能**:
- 自动检测违反 .gitignore 规则的被追踪文件
- 统计并显示详细信息
- 安全地从 git 索引中移除（保留本地文件）
- 交互式确认，避免误操作

**使用方法**:

```bash
# 在项目根目录运行
./tools/cleanup_git_ignored.sh
```

**执行流程**:

1. 检查当前是否在 git 仓库中
2. 扫描并统计需要清理的文件
3. 显示详细统计信息和样本文件
4. 请求用户确认
5. 执行清理（使用 `git rm --cached`）
6. 显示清理结果和后续操作建议

**检测的文件类型**:

- `lwext4_arce/target/` - Rust 编译产物
- `lwext4_core/target/` - Rust 编译产物
- `target/` - 根目录编译产物
- `devlogs/` - 开发日志
- `deprecated*` - 废弃文件
- `.claude/` - Claude Code 临时文件

**注意事项**:

- ✅ 使用 `--cached` 标志，只移除 git 追踪，保留本地文件
- ✅ 不影响工作目录内容
- ✅ 清理后需要手动提交
- ⚠️ 其他协作者 pull 后，这些文件会从他们的 git 中移除

**示例输出**:

```
========================================
  Git 追踪清理工具
========================================

📊 分析当前追踪状态...

发现以下被追踪但应该忽略的文件:

  lwext4_arce/target/   : 91 个文件
  lwext4_core/target/   : 120 个文件
  target/ (根目录)      : 0 个文件
  devlogs/              : 0 个文件
  deprecated*           : 0 个文件
  .claude/              : 0 个文件

总计: 211 个文件需要清理

...

是否继续清理? [y/N] y

🧹 开始清理...

正在清理 lwext4_arce/target/...
  ✓ 已移除 lwext4_arce/target/ (91 个文件)
正在清理 lwext4_core/target/...
  ✓ 已移除 lwext4_core/target/ (120 个文件)

✅ 清理完成！
```

**清理后的操作**:

```bash
# 1. 查看变更
git status

# 2. 提交变更
git commit -m "chore: remove build artifacts from git tracking"

# 3. 验证清理成功
git ls-files | grep 'target/'
# 应该返回空结果
```

---

### 2. count_rust_lines

**用途**: 统计 Rust 代码行数

**使用方法**:

```bash
./tools/count_rust_lines
```

---

### 3. count-lines.rs

**用途**: 代码行数统计工具源码

---

## 添加新工具

在添加新工具时，请遵循以下规范：

1. **命名**: 使用小写字母和下划线（snake_case）
2. **权限**: 可执行脚本添加 `+x` 权限
3. **文档**: 在本 README 中添加工具说明
4. **注释**: 脚本开头添加用途说明
5. **错误处理**: 使用 `set -e` 并提供清晰的错误信息

## 工具分类

### 代码质量
- count_rust_lines - 代码统计

### Git 管理
- cleanup_git_ignored.sh - Git 追踪清理

### 待添加
- [ ] format_check.sh - 代码格式检查
- [ ] license_check.sh - 许可证检查
- [ ] dependency_audit.sh - 依赖安全审计

---

**最后更新**: 2025-12-16
