use std::fs;
use std::path::{Path, PathBuf};

// 递归遍历目录，收集所有 .rs 文件
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rs_files(&path));
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files
}

// 统计单个文件的有效行数
fn count_file_lines(file: &Path) -> usize {
    let content = fs::read_to_string(file).unwrap_or_else(|e| {
        eprintln!("警告：读取文件 {} 失败: {}", file.display(), e);
        String::new()
    });

    let mut valid_lines = 0;
    let mut in_block_comment = false; // 标记是否处于块注释中

    for line in content.lines() {
        let trimmed = line.trim(); // 去除首尾空白

        // 1. 处理块注释状态
        if in_block_comment {
            // 若当前行包含 */，则退出块注释
            if trimmed.contains("*/") {
                in_block_comment = false;
                // 检查 */ 之后是否有有效代码（如 "*/ let x = 1;"）
                let after_comment = trimmed.split("*/").nth(1).unwrap_or("").trim();
                if !after_comment.is_empty() {
                    valid_lines += 1;
                }
            }
            continue; // 块注释内的行，直接跳过
        }

        // 2. 检查是否进入块注释
        if trimmed.starts_with("/*") {
            in_block_comment = true;
            // 检查是否是单行块注释（/* ... */）
            if trimmed.ends_with("*/") {
                in_block_comment = false;
                // 检查 */ 之后是否有有效代码
                let after_comment = trimmed.split("*/").nth(1).unwrap_or("").trim();
                if !after_comment.is_empty() {
                    valid_lines += 1;
                }
            }
            continue;
        }

        // 3. 过滤空行
        if trimmed.is_empty() {
            continue;
        }

        // 4. 过滤纯单行注释行（//、///、//!）
        if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!") {
            continue;
        }

        // 5. 有效行计数
        valid_lines += 1;
    }

    println!("{}: {}", file.display(), valid_lines);
    valid_lines
}

fn main() {
    // 目标目录（改为你的 Rust 项目目录）
    let target_dir = Path::new("../lwext4_core");
    let rs_files = collect_rs_files(target_dir);

    let total: usize = rs_files.iter().map(|f| count_file_lines(f)).sum();

    println!("========================================");
    println!("Total valid lines (excl. empty/comment): {}", total);
}