use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("无法读取工程文件 {path}：{source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("无法写入工程文件 {path}：{source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("工程文件内容损坏：{0}")]
    Parse(#[from] serde_json::Error),
    #[error("不是 Sonora 工程文件（缺少 format 字段）")]
    NotProject,
    #[error("工程格式为 {found}，本版本仅支持 {expected}")]
    WrongFormat { expected: String, found: String },
    #[error("工程版本 {found} 高于本程序支持的版本 {supported}")]
    NewerVersion { supported: u32, found: u32 },
}
