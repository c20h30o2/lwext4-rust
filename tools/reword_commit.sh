#!/bin/bash
# Commit 信息修改工具
# 用途: 修改最近的 commit 信息

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Commit 信息修改工具${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 检查是否在 git 仓库中
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo -e "${RED}❌ 错误: 当前目录不是 git 仓库${NC}"
    exit 1
fi

# 检查工作区是否干净
if ! git diff-index --quiet HEAD -- 2>/dev/null; then
    echo -e "${YELLOW}⚠️  警告: 工作区有未提交的修改${NC}"
    echo "建议先提交或暂存修改，然后再修改历史 commit"
    echo ""
    read -p "是否继续? [y/N] " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 0
    fi
    echo ""
fi

# 显示最近的 commits
echo -e "${CYAN}📋 最近的 10 个 commits:${NC}"
echo ""
git log --oneline --decorate -10
echo ""

# 提供选项
echo -e "${YELLOW}选择修改方式:${NC}"
echo ""
echo "  1) 修改最后一次 commit (HEAD)"
echo "  2) 修改最近 N 次 commit (交互式 rebase)"
echo "  3) 修改特定的 commit"
echo "  4) 取消"
echo ""
read -p "请选择 [1-4]: " -n 1 -r choice
echo ""
echo ""

case $choice in
    1)
        # 修改最后一次 commit
        echo -e "${GREEN}📝 修改最后一次 commit${NC}"
        echo ""

        # 显示当前 commit 信息
        echo -e "${CYAN}当前 commit 信息:${NC}"
        git log -1 --pretty=format:"%h - %s%n%n%b" HEAD
        echo ""
        echo ""

        echo "请输入新的 commit 信息 (输入完成后按 Ctrl+D):"
        echo "---"
        new_msg=$(cat)
        echo "---"
        echo ""

        if [ -z "$new_msg" ]; then
            echo -e "${RED}❌ commit 信息不能为空${NC}"
            exit 1
        fi

        echo -e "${YELLOW}新的 commit 信息:${NC}"
        echo "$new_msg"
        echo ""

        read -p "确认修改? [y/N] " -n 1 -r
        echo ""

        if [[ $REPLY =~ ^[Yy]$ ]]; then
            git commit --amend -m "$new_msg"
            echo ""
            echo -e "${GREEN}✅ Commit 信息已更新${NC}"
            echo ""
            echo "新的 commit:"
            git log -1 --oneline HEAD
        else
            echo -e "${RED}❌ 已取消${NC}"
        fi
        ;;

    2)
        # 交互式 rebase
        echo -e "${GREEN}📝 交互式修改多个 commits${NC}"
        echo ""
        read -p "要修改最近几个 commits? [输入数字]: " count

        if ! [[ "$count" =~ ^[0-9]+$ ]]; then
            echo -e "${RED}❌ 无效的数字${NC}"
            exit 1
        fi

        if [ "$count" -lt 1 ]; then
            echo -e "${RED}❌ 数字必须大于 0${NC}"
            exit 1
        fi

        echo ""
        echo -e "${YELLOW}将要修改最近 ${count} 个 commits${NC}"
        echo ""
        echo "操作说明:"
        echo "  1. 编辑器会打开，显示最近 ${count} 个 commits"
        echo "  2. 将要修改的 commit 前面的 'pick' 改为 'reword' 或 'r'"
        echo "  3. 保存并关闭编辑器"
        echo "  4. 对每个标记为 reword 的 commit，编辑器会再次打开让你修改信息"
        echo ""
        echo "其他可用命令:"
        echo "  - pick   = 使用这个 commit"
        echo "  - reword = 使用这个 commit，但修改信息"
        echo "  - edit   = 使用这个 commit，但停下来修改"
        echo "  - squash = 将这个 commit 合并到前一个"
        echo "  - drop   = 删除这个 commit"
        echo ""
        read -p "继续? [y/N] " -n 1 -r
        echo ""

        if [[ $REPLY =~ ^[Yy]$ ]]; then
            git rebase -i HEAD~${count}
            echo ""
            echo -e "${GREEN}✅ Rebase 完成${NC}"
        else
            echo -e "${RED}❌ 已取消${NC}"
        fi
        ;;

    3)
        # 修改特定 commit
        echo -e "${GREEN}📝 修改特定的 commit${NC}"
        echo ""
        read -p "请输入要修改的 commit hash (前 7 位即可): " commit_hash

        if [ -z "$commit_hash" ]; then
            echo -e "${RED}❌ commit hash 不能为空${NC}"
            exit 1
        fi

        # 验证 commit 是否存在
        if ! git rev-parse --verify "$commit_hash" >/dev/null 2>&1; then
            echo -e "${RED}❌ 找不到 commit: $commit_hash${NC}"
            exit 1
        fi

        echo ""
        echo -e "${CYAN}当前 commit 信息:${NC}"
        git log -1 --pretty=format:"%h - %s%n%n%b" "$commit_hash"
        echo ""
        echo ""

        # 计算需要 rebase 的深度
        commit_count=$(git rev-list --count ${commit_hash}..HEAD)
        rebase_target=$((commit_count + 1))

        echo -e "${YELLOW}需要 rebase 最近 ${rebase_target} 个 commits${NC}"
        echo ""
        echo "操作说明:"
        echo "  1. 编辑器会打开，找到 commit ${commit_hash}"
        echo "  2. 将其前面的 'pick' 改为 'reword' 或 'r'"
        echo "  3. 保存并关闭编辑器"
        echo "  4. 编辑器会再次打开让你修改 commit 信息"
        echo ""
        read -p "继续? [y/N] " -n 1 -r
        echo ""

        if [[ $REPLY =~ ^[Yy]$ ]]; then
            git rebase -i HEAD~${rebase_target}
            echo ""
            echo -e "${GREEN}✅ Rebase 完成${NC}"
        else
            echo -e "${RED}❌ 已取消${NC}"
        fi
        ;;

    4)
        echo -e "${RED}❌ 已取消${NC}"
        exit 0
        ;;

    *)
        echo -e "${RED}❌ 无效的选择${NC}"
        exit 1
        ;;
esac

echo ""
echo -e "${BLUE}📋 更新后的 commits:${NC}"
git log --oneline --decorate -10
echo ""

# 检查是否需要 force push
if git status | grep -q "Your branch and.*have diverged"; then
    echo -e "${YELLOW}⚠️  注意: 本地分支与远程分支已分叉${NC}"
    echo ""
    echo "如果已经推送到远程，需要使用 force push:"
    echo "  git push --force-with-lease"
    echo ""
    echo "⚠️  Force push 会覆盖远程历史，请谨慎使用！"
    echo "   如果其他人已经基于旧的 commit 工作，会造成问题。"
    echo ""
fi

echo -e "${GREEN}✨ 完成！${NC}"
