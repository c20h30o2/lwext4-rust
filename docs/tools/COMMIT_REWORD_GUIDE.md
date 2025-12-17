# Commit 信息修改指南

本文档介绍如何修改 Git commit 信息的各种方法。

---

## 🎯 快速开始

```bash
# 使用交互式工具
./tools/reword_commit.sh
```

---

## 📋 修改场景

### 场景 1: 修改最后一次 commit（最简单）

**适用**: 刚刚提交，还没推送到远程

```bash
# 方法 1: 使用工具（推荐）
./tools/reword_commit.sh
# 选择选项 1

# 方法 2: 直接使用 git 命令
git commit --amend

# 或者指定新消息
git commit --amend -m "新的 commit 信息"
```

**示例**:

```bash
# 修改最后一次 commit 的信息
git commit --amend -m "docs: reorganize documentation structure

- Move all docs to docs/ directory
- Create clear categories (guides/design/development/testing)
- Add comprehensive documentation index
- Total: 39 documentation files organized"
```

---

### 场景 2: 修改最近的多个 commits

**适用**: 需要修改最近 2-5 个 commit

```bash
# 使用工具
./tools/reword_commit.sh
# 选择选项 2，输入要修改的数量

# 或直接使用 git
git rebase -i HEAD~3  # 修改最近 3 个 commits
```

**操作步骤**:

1. 编辑器会打开，显示类似内容：
   ```
   pick bd11ac5 chore: after running cleanup_git_ignored.sh
   pick 4612a4f chore: commit before running cleanup_git_ignored.sh
   pick cc940da feature:dir| dir_idx| parts of extent
   ```

2. 将要修改的 commit 前面的 `pick` 改为 `reword` (或简写 `r`)：
   ```
   reword bd11ac5 chore: after running cleanup_git_ignored.sh
   reword 4612a4f chore: commit before running cleanup_git_ignored.sh
   pick cc940da feature:dir| dir_idx| parts of extent
   ```

3. 保存并关闭编辑器

4. Git 会依次打开编辑器让你修改每个标记为 `reword` 的 commit 信息

---

### 场景 3: 修改特定的某个 commit

**适用**: 需要修改更早的某个特定 commit

```bash
# 使用工具
./tools/reword_commit.sh
# 选择选项 3，输入 commit hash

# 或找到 commit 位置后使用 rebase
git log --oneline -20  # 找到要修改的 commit
git rebase -i <commit_hash>^  # commit 的父节点
```

---

## 📝 常用的 Rebase 命令

在 `git rebase -i` 编辑器中可用的命令：

| 命令 | 简写 | 说明 |
|------|------|------|
| pick | p | 使用这个 commit |
| reword | r | 使用这个 commit，但修改 commit 信息 |
| edit | e | 使用这个 commit，但停下来进行修改（修改内容） |
| squash | s | 将这个 commit 合并到前一个 commit |
| fixup | f | 类似 squash，但丢弃这个 commit 的信息 |
| drop | d | 删除这个 commit |

**示例**:

```bash
# 原始
pick a1b2c3d commit 1
pick e4f5g6h commit 2
pick i7j8k9l commit 3

# 修改 commit 2 的信息
pick a1b2c3d commit 1
reword e4f5g6h commit 2
pick i7j8k9l commit 3

# 将 commit 3 合并到 commit 2
pick a1b2c3d commit 1
pick e4f5g6h commit 2
squash i7j8k9l commit 3
```

---

## ⚠️ 重要注意事项

### 1. 已推送到远程的 commits

如果 commit 已经推送到远程仓库，修改后需要 **force push**：

```bash
# 推荐使用 --force-with-lease（更安全）
git push --force-with-lease

# 或使用 --force（较危险）
git push --force
```

**警告**:
- ⚠️ Force push 会覆盖远程历史
- ⚠️ 如果其他人已经基于这些 commits 工作，会造成问题
- ✅ 建议只修改自己的分支
- ✅ 协作分支修改前先沟通

### 2. 协作项目注意事项

**可以修改**:
- ✅ 只在自己的功能分支上
- ✅ 还没有被其他人使用的 commits
- ✅ 修改后立即通知协作者

**不要修改**:
- ❌ 主分支（main/master）的历史
- ❌ 已经被其他人基于开发的 commits
- ❌ 已经发布的版本标签

### 3. Rebase 冲突处理

如果在 rebase 过程中遇到冲突：

```bash
# 1. 解决冲突
# 编辑冲突文件，移除冲突标记

# 2. 标记为已解决
git add <解决冲突的文件>

# 3. 继续 rebase
git rebase --continue

# 或者放弃 rebase
git rebase --abort
```

---

## 📚 具体示例

### 示例 1: 改进最近两个 commit 的信息

**当前状态**:
```
bd11ac5 chore: after running cleanup_git_ignored.sh
4612a4f chore: commit before running cleanup_git_ignored.sh
```

**操作**:
```bash
git rebase -i HEAD~2
```

**在编辑器中**:
```
reword bd11ac5 chore: after running cleanup_git_ignored.sh
reword 4612a4f chore: commit before running cleanup_git_ignored.sh
```

**保存后，为每个 commit 输入新信息**:

第一个 commit:
```
chore: remove build artifacts from git tracking

- Remove lwext4_arce/target/ (91 files)
- Remove lwext4_core/target/ (120 files)
- Build artifacts should not be version controlled
- .gitignore rules already in place
```

第二个 commit:
```
chore: prepare for git cleanup

- Stage all changes before cleaning git tracking
- Ensure working directory is clean
```

---

### 示例 2: 合并多个小 commits

**当前状态**:
```
a1b2c3d fix: typo in function name
e4f5g6h fix: another typo
i7j8k9l fix: formatting
j1k2l3m feature: add new function
```

**操作**:
```bash
git rebase -i HEAD~4
```

**在编辑器中**:
```
pick j1k2l3m feature: add new function
squash a1b2c3d fix: typo in function name
squash e4f5g6h fix: another typo
squash i7j8k9l fix: formatting
```

**结果**: 所有修复会合并成一个 commit

---

## 🔄 撤销操作

如果 rebase 出问题了：

```bash
# 查看 reflog 找到 rebase 前的状态
git reflog

# 重置到 rebase 前
git reset --hard HEAD@{n}  # n 是 reflog 中的编号

# 或者在 rebase 过程中直接放弃
git rebase --abort
```

---

## 💡 最佳实践

### 写好 Commit 信息的原则

1. **使用约定式提交 (Conventional Commits)**
   ```
   <type>(<scope>): <subject>

   <body>

   <footer>
   ```

2. **常用类型**:
   - `feat`: 新功能
   - `fix`: 修复 bug
   - `docs`: 文档修改
   - `style`: 代码格式（不影响代码运行）
   - `refactor`: 重构
   - `test`: 测试相关
   - `chore`: 构建/工具/辅助工具

3. **第一行要求**:
   - 简短清晰（50 字符以内）
   - 动词开头，现在时
   - 不要句号结尾

4. **Body 部分**（可选）:
   - 详细说明为什么要这样修改
   - 可以包含多段
   - 每行不超过 72 字符

5. **Footer 部分**（可选）:
   - 关闭 Issue: `Closes #123`
   - Breaking changes: `BREAKING CHANGE: ...`

**好的示例**:
```
feat(extent): add CRC32C checksum support

- Implement compute_checksum() for extent blocks
- Add metadata_csum feature detection
- Include full test coverage (4/4 passed)

This enables the METADATA_CSUM feature for improved
data integrity verification.

Closes #42
```

---

## 🛠️ 工具使用示例

### 基本使用

```bash
# 1. 运行工具
./tools/reword_commit.sh

# 2. 查看最近的 commits

# 3. 选择操作方式
#    1 - 修改最后一次 commit
#    2 - 修改多个 commits
#    3 - 修改特定 commit
#    4 - 取消

# 4. 按提示操作
```

### 快速修改最后一次 commit

```bash
./tools/reword_commit.sh
# 输入: 1
# 输入新的 commit 信息
# 确认: y
```

---

## 📖 相关资源

- [Git Rebase 文档](https://git-scm.com/docs/git-rebase)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [如何写好 Git Commit 信息](https://chris.beams.io/posts/git-commit/)

---

**最后更新**: 2025-12-16
