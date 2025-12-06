# lwext4-core 实现计划

## 已完成 ✅

### 框架搭建（2024-12-05）
- ✅ 创建模块结构（lib.rs, consts.rs, types.rs, error.rs等）
- ✅ 定义所有常量（EXT4_DEV_BSIZE, 文件类型常量等）
- ✅ 定义核心数据结构（Ext4Superblock, Ext4Inode, Ext4InodeRef等）
- ✅ 创建所有 36 个占位函数
- ✅ 编译通过（仅有未使用参数警告）

**代码统计**：
```bash
$ find src -name "*.rs" | xargs wc -l
  15 src/lib.rs
  58 src/consts.rs
  28 src/error.rs
 195 src/types.rs
  55 src/superblock.rs
 144 src/inode.rs
 104 src/block.rs
  74 src/dir.rs
  42 src/fs.rs
 -----
 715 total  # 已完成 ~60% 的框架代码
```

## 下一步：实现核心功能

### 阶段 1：只读功能（优先级 P0）

#### 1.1 Superblock 读取 ⬜ TODO
**文件**：`src/superblock.rs`
**任务**：
- [ ] 完善 `read_superblock()` - 从块设备读取并解析
- [ ] 实现字节序转换（little-endian）
- [ ] 添加验证逻辑（魔数、版本检查）

**测试目标**：能够读取并打印 rootfs 镜像的 superblock 信息

#### 1.2 Inode 读取 ⬜ TODO
**文件**：`src/inode.rs`
**任务**：
- [ ] 实现 `ext4_fs_get_inode_ref()` - 计算 inode 位置并读取
- [ ] 实现 `ext4_fs_put_inode_ref()` - 写回脏数据
- [ ] 实现 inode 位置计算公式

**公式**：
```
块组号 = (inode_num - 1) / inodes_per_group
组内索引 = (inode_num - 1) % inodes_per_group
inode_table_block = 块组描述符.inode_table
inode_offset = inode_table_block * block_size + 组内索引 * inode_size
```

**测试目标**：读取根目录 inode (inode 2)

#### 1.3 块映射 ⬜ TODO
**文件**：`src/inode.rs`
**任务**：
- [ ] 实现 `ext4_fs_get_inode_dblk_idx()` - 文件块号 → 磁盘块号
- [ ] 支持直接块（12个）
- [ ] 暂不支持间接块（简化实现）

**测试目标**：能够读取小文件（< 48KB）

#### 1.4 文件读取 ⬜ TODO
**文件**：`src/block.rs`
**任务**：
- [ ] 实现 `ext4_block_readbytes()` - 读取任意偏移的数据
- [ ] 处理跨块读取
- [ ] 处理非对齐读取

**测试目标**：读取 `/bin/busybox` 的前 4 字节（应该是 ELF 魔数）

#### 1.5 目录遍历 ⬜ TODO
**文件**：`src/dir.rs`
**任务**：
- [ ] 实现 `ext4_dir_find_entry()` - 查找目录项
- [ ] 实现 `ext4_dir_iterator_*()` - 遍历目录
- [ ] 解析目录项结构

**测试目标**：列出根目录内容（bin, etc, usr等）

### 阶段 2：写入功能（优先级 P1）

#### 2.1 Inode 分配 ⬜ TODO
**文件**：`src/inode.rs`
**任务**：
- [ ] 实现 `ext4_fs_alloc_inode()` - 从位图分配 inode
- [ ] 实现 `ext4_fs_free_inode()` - 释放 inode

#### 2.2 块分配 ⬜ TODO
**文件**：`src/block.rs`
**任务**：
- [ ] 实现 `ext4_fs_append_inode_dblk()` - 分配数据块
- [ ] 实现块位图操作

#### 2.3 文件写入 ⬜ TODO
**文件**：`src/block.rs`
**任务**：
- [ ] 实现 `ext4_block_writebytes()` - 写入数据

#### 2.4 目录修改 ⬜ TODO
**文件**：`src/dir.rs`
**任务**：
- [ ] 实现 `ext4_dir_add_entry()` - 添加目录项
- [ ] 实现 `ext4_dir_remove_entry()` - 删除目录项

### 阶段 3：缓存优化（优先级 P2）

#### 3.1 LRU 块缓存 ⬜ TODO
**文件**：`src/cache.rs`（新建）
**任务**：
- [ ] 实现简单的 LRU 缓存
- [ ] 实现 `ext4_bcache_init_dynamic()`
- [ ] 实现 `ext4_block_cache_flush()`

## 集成测试计划

### 测试 1：只读文件系统
```rust
#[test]
fn test_read_rootfs() {
    let device = RamDisk::from_file("rootfs-riscv64.img");
    let fs = Ext4Filesystem::new(device);

    // 1. 读取 superblock
    assert_eq!(fs.sb.magic, 0xEF53);

    // 2. 读取根目录
    let entries = fs.read_dir(2).collect();
    assert!(entries.contains("bin"));

    // 3. 读取文件
    let busybox_ino = fs.lookup(2, "bin")?.lookup("busybox")?;
    let mut buf = [0u8; 4];
    fs.read_at(busybox_ino, &mut buf, 0)?;
    assert_eq!(&buf, b"\x7fELF");
}
```

### 测试 2：写入功能
```rust
#[test]
fn test_write_file() {
    let mut fs = create_test_fs();

    // 创建文件
    let ino = fs.create(2, "test.txt", InodeType::RegularFile, 0o644)?;

    // 写入数据
    fs.write_at(ino, b"Hello, world!", 0)?;

    // 读取验证
    let mut buf = [0u8; 13];
    fs.read_at(ino, &mut buf, 0)?;
    assert_eq!(&buf, b"Hello, world!");
}
```

## 时间估算

| 阶段 | 任务 | 预计时间 |
|------|------|----------|
| 1.1 | Superblock 读取 | 2-3 小时 |
| 1.2 | Inode 读取 | 4-6 小时 |
| 1.3 | 块映射 | 3-4 小时 |
| 1.4 | 文件读取 | 3-4 小时 |
| 1.5 | 目录遍历 | 4-6 小时 |
| **测试 1** | 只读功能测试 | 4 小时 |
| 2.1-2.4 | 写入功能 | 12-16 小时 |
| **测试 2** | 写入功能测试 | 4 小时 |
| 3.1 | 缓存优化 | 4-6 小时 |
| **总计** | | **40-55 小时（5-7 工作日）** |

## 开发建议

1. **增量开发**：每完成一个模块立即测试
2. **参考 C 代码**：在 ~/files/lwext4 查看原始实现
3. **使用 hexdump**：验证读取的数据正确性
   ```bash
   hexdump -C rootfs-riscv64.img | head -50
   ```
4. **日志调试**：充分使用 `debug!()` 宏
5. **单元测试**：为每个函数编写测试

## 参考资料

- ext4 磁盘格式：https://ext4.wiki.kernel.org/index.php/Ext4_Disk_Layout
- C 版本 lwext4：~/files/lwext4/
- 现有 lwext4_rust：~/files/lwext4-rust/lwext4_rust/
