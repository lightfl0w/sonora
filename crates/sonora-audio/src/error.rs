#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("找不到音频输出设备")]
    NoDevice,
    #[error("读取设备配置失败：{0}")]
    Config(String),
    #[error("不支持的采样格式：{0}")]
    UnsupportedFormat(String),
    #[error("构建音频流失败：{0}")]
    BuildStream(String),
    #[error("启动音频流失败：{0}")]
    Play(String),
}
