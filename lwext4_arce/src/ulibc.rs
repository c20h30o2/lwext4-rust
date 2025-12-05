//! 该模块模拟libc的部分功能（如printf、内存分配），适配ext4库的C接口。

/// 模拟printf函数，用于日志输出
mod uprint {
    use core::ffi::{c_char, c_int};

    /// 当启用"print"特性时，实现完整的printf格式化
    #[cfg(feature = "print")]
    #[linkage = "weak"] // 弱链接，允许外部覆盖
    #[unsafe(no_mangle)] // 保持C兼容的函数名
    unsafe extern "C" fn printf(str: *const c_char, mut args: ...) -> c_int {
        use printf_compat::{format, output};

        // 分配字符串缓冲区
        let mut s = alloc::string::String::new();
        // 格式化参数到字符串
        let bytes_written = unsafe { format(str as _, args.as_va_list(), output::fmt_write(&mut s)) };
        // 输出到日志
        info!("{}", s);

        bytes_written
    }

    /// 当禁用"print"特性时，仅输出日志标记
    #[cfg(not(feature = "print"))]
    #[linkage = "weak"]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn printf(str: *const c_char, _args: ...) -> c_int {
        use core::ffi::CStr;
        // 转换C字符串为Rust字符串
        let c_str = unsafe { CStr::from_ptr(str) };
        // 输出日志标记
        info!("[lwext4] {:?}", c_str);
        0 // 返回0表示成功
    }
}

/// 模拟内存分配函数（malloc/calloc/realloc/free），适配ext4的内存管理接口
mod ualloc {
    use alloc::alloc::{alloc, dealloc, Layout};
    use alloc::slice::from_raw_parts_mut;
    use core::cmp::min;
    use core::ffi::{c_int, c_size_t, c_void};

    /// 模拟calloc：分配内存并初始化为0
    #[unsafe(no_mangle)]
    pub extern "C" fn ext4_user_calloc(m: c_size_t, n: c_size_t) -> *mut c_void {
        // 先分配内存
        let mem = ext4_user_malloc(m * n);

        // 调用C的memset初始化内存为0
        unsafe extern "C" {
            pub fn memset(dest: *mut c_void, c: c_int, n: c_size_t) -> *mut c_void;
        }
        unsafe { memset(mem, 0, m * n) }
    }

    /// 模拟realloc：重新分配内存并复制数据
    #[unsafe(no_mangle)]
    pub extern "C" fn ext4_user_realloc(memblock: *mut c_void, size: c_size_t) -> *mut c_void {
        if memblock.is_null() {
            warn!("realloc a null mem pointer");
            return ext4_user_malloc(size);
        }

        // 获取内存控制块（存储原始大小）
        let ptr = memblock.cast::<MemoryControlBlock>();
        let old_size = unsafe { ptr.sub(1).read().size }; //  unsafe：访问控制块
        info!("realloc from {} to {}", old_size, size);

        // 分配新内存
        let mem = ext4_user_malloc(size);

        // 复制旧数据到新内存
        unsafe {
            let copy_size = min(size, old_size);
            let new_buf = from_raw_parts_mut(mem as *mut u8, copy_size);
            let old_buf = from_raw_parts_mut(memblock as *mut u8, copy_size);
            new_buf.copy_from_slice(old_buf);
        }
        // 释放旧内存
        ext4_user_free(memblock);

        mem
    }

    /// 内存控制块：存储分配的内存大小（用于free时释放正确的空间）
    struct MemoryControlBlock {
        size: usize, // 分配的内存大小
    }
    /// 控制块的大小（字节）
    const CTRL_BLK_SIZE: usize = core::mem::size_of::<MemoryControlBlock>();

    /// 模拟malloc：分配指定大小的内存
    #[unsafe(no_mangle)]
    pub extern "C" fn ext4_user_malloc(size: c_size_t) -> *mut c_void {
        // 实际分配的大小 = 请求大小 + 控制块大小
        let layout = Layout::from_size_align(size + CTRL_BLK_SIZE, 8).unwrap();
        unsafe {
            let ptr = alloc(layout); // 分配内存
            assert!(!ptr.is_null(), "malloc failed"); // 确保分配成功

            // 在控制块中存储分配的大小
            let ctrl_ptr = ptr.cast::<MemoryControlBlock>();
            ctrl_ptr.write(MemoryControlBlock { size });
            // 返回控制块之后的地址（用户可见的内存起始地址）
            ctrl_ptr.add(1).cast()
        }
    }

    /// 模拟free：释放内存
    #[unsafe(no_mangle)]
    pub extern "C" fn ext4_user_free(ptr: *mut c_void) {
        if ptr.is_null() {
            warn!("free a null pointer !");
            return;
        }

        // 计算控制块的地址
        let user_ptr = ptr.cast::<MemoryControlBlock>();
        assert!(user_ptr as usize > CTRL_BLK_SIZE, "invalid pointer");
        unsafe {
            let ctrl_ptr = user_ptr.sub(1); // 控制块在用户指针之前
            let size = ctrl_ptr.read().size; // 读取原始大小
            // 释放整个内存块（包括控制块）
            let layout = Layout::from_size_align(size + CTRL_BLK_SIZE, 8).unwrap();
            dealloc(ctrl_ptr.cast(), layout);
        }
    }
}