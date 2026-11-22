use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_queue::ArrayQueue;
use sonora_core::Song;
use sonora_engine::command::Command;
use sonora_engine::library::VoiceLibrary;
use sonora_engine::render::{RenderCore, SCOPE_LEN};
use sonora_engine::transport::Transport;

use crate::error::AudioError;

const QUEUE_CAP: usize = 256;
const MAX_BLOCK_FRAMES: usize = 16384;

pub struct AudioEngine {
    queue: Arc<ArrayQueue<Command>>,
    #[allow(dead_code)]
    stream: Option<cpal::Stream>,
    pub scope: Arc<[AtomicU32; SCOPE_LEN]>,
    pub scope_write: Arc<AtomicU32>,
    pub transport: Arc<Transport>,
    pub sample_rate: f32,
    pub device_name: String,
    pub channels: u16,
}

impl AudioEngine {
    pub fn start(song: Song, lib: VoiceLibrary) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;
        let device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "未知设备".to_string());
        let supported = device
            .default_output_config()
            .map_err(|e| AudioError::Config(e.to_string()))?;
        let sr = supported.sample_rate();
        let channels = supported.channels();
        let config = supported.config();
        let format = supported.sample_format();
        let queue = Arc::new(ArrayQueue::new(QUEUE_CAP));
        let scope: Arc<[AtomicU32; SCOPE_LEN]> =
            Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
        let scope_write = Arc::new(AtomicU32::new(0));
        let core = RenderCore::new(song, sr as f32, scope.clone(), Arc::new(lib));
        let transport = core.transport.clone();
        let stream = match format {
            cpal::SampleFormat::F32 => build_f32_stream(
                &device,
                config,
                core,
                queue.clone(),
                scope_write.clone(),
                channels as usize,
            )?,
            cpal::SampleFormat::I16 => build_i16_stream(
                &device,
                config,
                core,
                queue.clone(),
                scope_write.clone(),
                channels as usize,
            )?,
            other => return Err(AudioError::UnsupportedFormat(format!("{other:?}"))),
        };
        stream.play().map_err(|e| AudioError::Play(e.to_string()))?;
        Ok(Self {
            queue,
            stream: Some(stream),
            scope,
            scope_write,
            transport,
            sample_rate: sr as f32,
            device_name,
            channels,
        })
    }
    pub fn send(&self, cmd: Command) {
        let _ = self.queue.push(cmd);
    }
    pub fn is_playing(&self) -> bool {
        self.transport.is_playing()
    }
    pub fn scope_snapshot(&self, out: &mut Vec<f32>) {
        let end = self.scope_write.load(Ordering::Relaxed) as usize % SCOPE_LEN;
        out.clear();
        out.reserve(SCOPE_LEN);
        for i in 0..SCOPE_LEN {
            let idx = (end + i) % SCOPE_LEN;
            out.push(f32::from_bits(self.scope[idx].load(Ordering::Relaxed)));
        }
    }
}

struct Shared {
    core: RenderCore,
    queue: Arc<ArrayQueue<Command>>,
    scope_write: Arc<AtomicU32>,
    channels: usize,
    l: Vec<f32>,
    r: Vec<f32>,
}

impl Shared {
    fn new(
        core: RenderCore,
        queue: Arc<ArrayQueue<Command>>,
        scope_write: Arc<AtomicU32>,
        channels: usize,
    ) -> Self {
        Self {
            core,
            queue,
            scope_write,
            channels,
            l: vec![0.0; MAX_BLOCK_FRAMES],
            r: vec![0.0; MAX_BLOCK_FRAMES],
        }
    }
    fn fill(&mut self, sample_count: usize) -> usize {
        let frames = (sample_count / self.channels).min(MAX_BLOCK_FRAMES);
        while let Some(cmd) = self.queue.pop() {
            self.core.handle(cmd);
        }
        self.core
            .render(&mut self.l[..frames], &mut self.r[..frames]);
        self.scope_write
            .store(self.core.scope_write_index() as u32, Ordering::Relaxed);
        frames
    }
}

fn build_f32_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    core: RenderCore,
    queue: Arc<ArrayQueue<Command>>,
    scope_write: Arc<AtomicU32>,
    channels: usize,
) -> Result<cpal::Stream, AudioError> {
    let mut shared = Shared::new(core, queue, scope_write, channels);
    device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| {
                let frames = shared.fill(data.len());
                for (i, frame) in data.chunks_mut(channels).enumerate() {
                    let (li, ri) = if i < frames {
                        (shared.l[i], shared.r[i])
                    } else {
                        (0.0, 0.0)
                    };
                    for (c, s) in frame.iter_mut().enumerate() {
                        *s = if c % 2 == 0 { li } else { ri };
                    }
                }
            },
            |e| eprintln!("音频流错误: {e}"),
            None,
        )
        .map_err(|e| AudioError::BuildStream(e.to_string()))
}

fn build_i16_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    core: RenderCore,
    queue: Arc<ArrayQueue<Command>>,
    scope_write: Arc<AtomicU32>,
    channels: usize,
) -> Result<cpal::Stream, AudioError> {
    let mut shared = Shared::new(core, queue, scope_write, channels);
    device
        .build_output_stream(
            config,
            move |data: &mut [i16], _| {
                let frames = shared.fill(data.len());
                for (i, frame) in data.chunks_mut(channels).enumerate() {
                    let (ls, rs) = if i < frames {
                        (
                            (shared.l[i].clamp(-1.0, 1.0) * 32767.0) as i16,
                            (shared.r[i].clamp(-1.0, 1.0) * 32767.0) as i16,
                        )
                    } else {
                        (0i16, 0i16)
                    };
                    for (c, s) in frame.iter_mut().enumerate() {
                        *s = if c % 2 == 0 { ls } else { rs };
                    }
                }
            },
            |e| eprintln!("音频流错误: {e}"),
            None,
        )
        .map_err(|e| AudioError::BuildStream(e.to_string()))
}
