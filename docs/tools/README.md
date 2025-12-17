# 项目工具文档

本目录包含 lwext4-rust 项目使用的工具和脚本的文档。

## 工具列表

### Git 相关工具

#### Commit 重写工具
- **文档**: [COMMIT_REWORD_GUIDE.md](./COMMIT_REWORD_GUIDE.md)
- **脚本**: `../../tools/reword_commit.sh`
- **功能**: 改写 git commit 信息，统一提交格式
- **使用场景**: 规范化 commit message，符合项目约定

### 代码统计工具

#### Rust 代码行数统计
- **脚本**: `../../tools/count_rust_lines`
- **功能**: 统计 Rust 代码行数
- **使用方法**:
  ```bash
  ./tools/count_rust_lines
  ```

### 清理工具

#### Git 忽略文件清理
- **脚本**: `../../tools/cleanup_git_ignored.sh`
- **功能**: 清理被 .gitignore 忽略的文件
- **使用方法**:
  ```bash
  ./tools/cleanup_git_ignored.sh
  ```

## 工具开发指南

### 添加新工具

1. 将工具脚本放在 `tools/` 目录
2. 在本目录创建使用文档
3. 更新本 README 添加工具说明
4. 确保工具有可执行权限（`chmod +x`）

### 文档规范

工具文档应包含：
- 工具用途和功能
- 使用方法和示例
- 参数说明
- 注意事项
- 常见问题

### 命名规范

- 脚本使用下划线命名: `script_name.sh`
- 文档使用大写蛇形命名: `TOOL_NAME_GUIDE.md`

## 相关链接

- [tools/ 目录说明](./tools-readme.md) - 工具目录的原始 README
- [开发指南](../guides/) - 项目开发指南

---

最后更新：2025-12-17
