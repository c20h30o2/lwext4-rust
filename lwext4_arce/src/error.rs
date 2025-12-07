//! 错误处理模块，定义了ext4操作的错误类型和辅助方法。

// 当使用纯 Rust 实现时，直接使用 lwext4_core 的错误类型
#[cfg(feature = "use-rust")]
pub use lwext4_core::{Ext4Error, Ext4Result};

// 当使用 C FFI 时，定义自己的错误类型
#[cfg(not(feature = "use-rust"))]
mod ffi_error {
    use core::{
        error::Error,
        fmt::{Debug, Display},
    };

    /// 成功状态码
    const EOK: i32 = 0;

    /// ext4操作的结果类型（成功或错误）
    pub type Ext4Result<T = ()> = Result<T, Ext4Error>;

    /// ext4错误类型，包含错误码和上下文信息
    pub struct Ext4Error {
        pub code: i32,                         // 错误码（与C接口兼容）
        pub context: Option<&'static str>,     // 错误上下文（可选）
    }

    impl Ext4Error {
        /// 创建新的Ext4Error
        pub fn new(code: i32, context: impl Into<Option<&'static str>>) -> Self {
            Ext4Error {
                code,
                context: context.into(),
            }
        }
    }

    /// 从错误码转换为Ext4Error
    impl From<i32> for Ext4Error {
        fn from(code: i32) -> Self {
            Ext4Error::new(code, None)
        }
    }

    /// 实现Display trait，用于格式化错误信息
    impl Display for Ext4Error {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            if let Some(context) = self.context {
                write!(f, "ext4 error {}: {context}", self.code)
            } else {
                write!(f, "ext4 error {}", self.code)
            }
        }
    }

    /// 实现Debug trait，复用Display的实现
    impl Debug for Ext4Error {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            Display::fmt(self, f)
        }
    }

    /// 实现Error trait，使Ext4Error符合标准错误类型
    impl Error for Ext4Error {}

    /// 为结果类型添加上下文的 trait
    pub(crate) trait Context<T> {
        /// 为错误添加上下文信息
        fn context(self, context: &'static str) -> Result<T, Ext4Error>;
    }

    /// 为i32（C函数返回码）实现Context trait
    impl Context<()> for i32 {
        fn context(self, context: &'static str) -> Result<(), Ext4Error> {
            if self != EOK as _ {
                Err(Ext4Error::new(self, Some(context)))
            } else {
                Ok(())
            }
        }
    }

    /// 为Ext4Result实现Context trait（嵌套错误时添加上下文）
    impl<T> Context<T> for Ext4Result<T> {
        fn context(self, context: &'static str) -> Result<T, Ext4Error> {
            self.map_err(|e| Ext4Error::new(e.code, Some(context)))
        }
    }
}

#[cfg(not(feature = "use-rust"))]
pub use ffi_error::*;

// 为 lwext4_core 的 Result 类型添加 context 方法的扩展 trait
#[cfg(feature = "use-rust")]
pub(crate) trait Context<T> {
    /// 为错误添加上下文信息 (在 use-rust 模式下，context 被忽略因为 lwext4_core::Ext4Error 已包含消息)
    fn context(self, _context: &'static str) -> Self;
}

#[cfg(feature = "use-rust")]
impl<T> Context<T> for Result<T, Ext4Error> {
    fn context(self, _context: &'static str) -> Self {
        // lwext4_core 的 Ext4Error 已经包含了错误消息，所以直接返回
        self
    }
}

#[cfg(feature = "use-rust")]
impl Context<()> for i32 {
    fn context(self, context: &'static str) -> i32 {
        // 在 use-rust 模式下，这个方法不应该被调用
        // 因为所有函数都返回 Result 而不是 i32
        self
    }
}
