#[derive(Debug, thiserror::Error)]
pub enum MidiError {
    #[error("不是有效的 MIDI 文件（缺少 MThd 头）")]
    NotMidi,
    #[error("不支持的 SMPTE 时间格式")]
    Smpte,
    #[error("文件结构损坏：{0}")]
    Corrupt(&'static str),
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
}
