//! 错误处理模块

use core::fmt;
use crate::consts::*;

/// ext4 错误类型
#[derive(Debug, Clone)]
pub struct Ext4Error {
    pub code: i32,
    pub message: Option<&'static str>,
}

impl Ext4Error {
    pub fn new(code: i32, message: impl Into<Option<&'static str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_code(code: i32) -> Self {
        Self {
            code,
            message: None,
        }
    }
}

impl fmt::Display for Ext4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(msg) = self.message {
            write!(f, "Ext4Error(code={}, msg={})", self.code, msg)
        } else {
            write!(f, "Ext4Error(code={})", self.code)
        }
    }
}

/// ext4 Result 类型
pub type Ext4Result<T> = Result<T, Ext4Error>;

/// 辅助函数：检查返回码
pub fn check_result(code: i32) -> Ext4Result<()> {
    if code == EOK {
        Ok(())
    } else {
        Err(Ext4Error::from_code(code))
    }
}
