use serde::Serialize;
use thiserror::Error;

/// 统一错误类型，方便返回给前端
#[derive(Error, Debug)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] sqlx::Error),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("图片处理错误: {0}")]
    Image(#[from] image::ImageError),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("压缩包错误: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Base64 解码错误: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("{0}")]
    Message(String),
}

impl AppError {
    pub fn msg(m: impl Into<String>) -> Self {
        AppError::Message(m.into())
    }
}

// 转换为 Tauri 能发送的结果（错误序列化为字符串）
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
