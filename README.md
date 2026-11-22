# Sonora

Sonora 是一个使用 Rust 编写的桌面音乐编曲与加法合成器项目。
项目以正弦波为基础，通过多个部分音的叠加构建不同音色，并将声音合成、音序编排与多轨编辑整合在一个桌面应用中。

## 快速开始

### 环境要求
- Rust 工具链
- 可用的音频输出设备
- CPAL 和 GPUI 所需的平台依赖

```bash
# 获取源码
git clone https://github.com/lightfl0w/sonora.git
cd sonora

# 编译
cargo build --release

# 运行
cargo run --release
```

## 仓库结构

| crate | 作用 |
| --- | --- |
| `sonora-app` | 可执行程序 |
| `sonora-ui` | UI组件 |
| `sonora-engine` | 合成与调度 |
| `sonora-audio` | cpal 音频输出与命令队列 |
| `sonora-core` | 数据模型 |
| `sonora-project` | 工程文件读写与默认路径 |
| `sonora-midi` | MIDI 导入导出 |
| `sonora-dsp` | DSP 基础件 |

## 测试

```bash
cargo test --workspace
```

## 已知限制

- 钢琴窗一次只编辑一条轨道
- MIDI 导出是 format 0 单轨，不保留轨道名与音色编号

## 许可证

MIT，见 [LICENSE](LICENSE)。
