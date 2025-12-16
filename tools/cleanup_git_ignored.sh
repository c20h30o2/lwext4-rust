#!/bin/bash
# Git 追踪清理脚本
# 用途: 从 git 中移除 .gitignore 中定义但仍被追踪的文件

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Git 追踪清理工具${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 检查是否在 git 仓库中
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo -e "${RED}❌ 错误: 当前目录不是 git 仓库${NC}"
    exit 1
fi

# 切换到仓库根目录
cd "$(git rev-parse --show-toplevel)"

echo -e "${YELLOW}📊 分析当前追踪状态...${NC}"
echo ""

# 统计被追踪的文件
tracked_arce_target=$(git ls-files | grep "^lwext4_arce/target/" | wc -l)
tracked_core_target=$(git ls-files | grep "^lwext4_core/target/" | wc -l)
tracked_root_target=$(git ls-files | grep "^target/" | wc -l)
tracked_devlogs=$(git ls-files | grep "^devlogs/" | wc -l)
tracked_deprecated=$(git ls-files | grep "^deprecated" | wc -l)
tracked_claude=$(git ls-files | grep "^\.claude/" | wc -l)

total=$((tracked_arce_target + tracked_core_target + tracked_root_target + tracked_devlogs + tracked_deprecated + tracked_claude))

# 显示统计
echo "发现以下被追踪但应该忽略的文件:"
echo ""
echo "  lwext4_arce/target/   : ${tracked_arce_target} 个文件"
echo "  lwext4_core/target/   : ${tracked_core_target} 个文件"
echo "  target/ (根目录)      : ${tracked_root_target} 个文件"
echo "  devlogs/              : ${tracked_devlogs} 个文件"
echo "  deprecated*           : ${tracked_deprecated} 个文件"
echo "  .claude/              : ${tracked_claude} 个文件"
echo ""
echo -e "${YELLOW}总计: ${total} 个文件需要清理${NC}"
echo ""

if [ "$total" -eq 0 ]; then
    echo -e "${GREEN}✅ 没有需要清理的文件，git 追踪状态正常！${NC}"
    exit 0
fi

# 显示样本文件
if [ "$tracked_arce_target" -gt 0 ]; then
    echo "lwext4_arce/target/ 样本文件 (前 5 个):"
    git ls-files | grep "^lwext4_arce/target/" | head -5 | sed 's/^/  - /'
    echo ""
fi

if [ "$tracked_core_target" -gt 0 ]; then
    echo "lwext4_core/target/ 样本文件 (前 5 个):"
    git ls-files | grep "^lwext4_core/target/" | head -5 | sed 's/^/  - /'
    echo ""
fi

# 询问确认
echo -e "${YELLOW}⚠️  警告: 此操作将从 git 索引中移除这些文件${NC}"
echo "   (文件仍保留在工作目录中，只是不再被 git 追踪)"
echo ""
read -p "是否继续清理? [y/N] " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${RED}❌ 已取消清理${NC}"
    exit 0
fi

echo ""
echo -e "${GREEN}🧹 开始清理...${NC}"
echo ""

# 执行清理
cleaned=0

if [ "$tracked_arce_target" -gt 0 ]; then
    echo "正在清理 lwext4_arce/target/..."
    if git rm -r --cached lwext4_arce/target/ > /dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} 已移除 lwext4_arce/target/ ($tracked_arce_target 个文件)"
        cleaned=$((cleaned + tracked_arce_target))
    else
        echo -e "  ${YELLOW}⚠${NC} lwext4_arce/target/ 清理失败或已清理"
    fi
fi

if [ "$tracked_core_target" -gt 0 ]; then
    echo "正在清理 lwext4_core/target/..."
    if git rm -r --cached lwext4_core/target/ > /dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} 已移除 lwext4_core/target/ ($tracked_core_target 个文件)"
        cleaned=$((cleaned + tracked_core_target))
    else
        echo -e "  ${YELLOW}⚠${NC} lwext4_core/target/ 清理失败或已清理"
    fi
fi

if [ "$tracked_root_target" -gt 0 ]; then
    echo "正在清理 target/..."
    if git rm -r --cached target/ > /dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} 已移除 target/ ($tracked_root_target 个文件)"
        cleaned=$((cleaned + tracked_root_target))
    else
        echo -e "  ${YELLOW}⚠${NC} target/ 清理失败或已清理"
    fi
fi

if [ "$tracked_devlogs" -gt 0 ]; then
    echo "正在清理 devlogs/..."
    if git rm -r --cached devlogs/ > /dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} 已移除 devlogs/ ($tracked_devlogs 个文件)"
        cleaned=$((cleaned + tracked_devlogs))
    else
        echo -e "  ${YELLOW}⚠${NC} devlogs/ 清理失败或已清理"
    fi
fi

if [ "$tracked_deprecated" -gt 0 ]; then
    echo "正在清理 deprecated*..."
    git ls-files | grep "^deprecated" | while read -r file; do
        git rm --cached "$file" > /dev/null 2>&1 || true
    done
    echo -e "  ${GREEN}✓${NC} 已移除 deprecated* ($tracked_deprecated 个文件)"
    cleaned=$((cleaned + tracked_deprecated))
fi

if [ "$tracked_claude" -gt 0 ]; then
    echo "正在清理 .claude/..."
    if git rm -r --cached .claude/ > /dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} 已移除 .claude/ ($tracked_claude 个文件)"
        cleaned=$((cleaned + tracked_claude))
    else
        echo -e "  ${YELLOW}⚠${NC} .claude/ 清理失败或已清理"
    fi
fi

echo ""
echo -e "${GREEN}✅ 清理完成！${NC}"
echo ""
echo -e "${BLUE}📊 清理统计:${NC}"
echo "  - 已从 git 索引移除: ${cleaned} 个文件"
echo "  - 文件仍保留在本地工作目录"
echo ""

# 显示 git 状态
echo -e "${BLUE}📋 Git 状态 (前 20 行):${NC}"
git status --short | head -20
echo ""

# 提示下一步
echo -e "${YELLOW}📝 下一步操作:${NC}"
echo ""
echo "1. 查看完整变更:"
echo "   git status"
echo ""
echo "2. 提交变更:"
echo "   git commit -m 'chore: remove build artifacts from git tracking"
echo ""
echo "   - Remove lwext4_arce/target/ (${tracked_arce_target} files)"
echo "   - Remove lwext4_core/target/ (${tracked_core_target} files)"
echo "   - These are build artifacts and should not be tracked"
echo "   - .gitignore already has these directories listed'"
echo ""
echo "3. 验证清理结果:"
echo "   git ls-files | grep 'target/'"
echo "   (应该返回空结果)"
echo ""
echo -e "${GREEN}✨ 完成！${NC}"
