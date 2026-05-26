use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ClipboardError {
    Clipboard(String),
    Database(String),
    Io(String),
    NotFound(i64),
    Runtime(String),
}

impl Display for ClipboardError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardError::Clipboard(message) => write!(formatter, "剪贴板错误：{message}"),
            ClipboardError::Database(message) => write!(formatter, "数据库错误：{message}"),
            ClipboardError::Io(message) => write!(formatter, "文件系统错误：{message}"),
            ClipboardError::NotFound(id) => write!(formatter, "剪贴板记录不存在：{id}"),
            ClipboardError::Runtime(message) => write!(formatter, "运行时错误：{message}"),
        }
    }
}

impl std::error::Error for ClipboardError {}

impl From<rusqlite::Error> for ClipboardError {
    fn from(error: rusqlite::Error) -> Self {
        ClipboardError::Database(error.to_string())
    }
}

impl From<std::io::Error> for ClipboardError {
    fn from(error: std::io::Error) -> Self {
        ClipboardError::Io(error.to_string())
    }
}

impl From<arboard::Error> for ClipboardError {
    fn from(error: arboard::Error) -> Self {
        ClipboardError::Clipboard(error.to_string())
    }
}

impl serde::Serialize for ClipboardError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
