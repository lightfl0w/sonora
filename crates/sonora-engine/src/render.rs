use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use sonora_core::Song;
use sonora_dsp::{Biquad, Compressor, Reverb, soft_clip};

use crate::command::{Command, Param};
use crate::library::VoiceLibrary;
use crate::pool::VoicePool;
use crate::sequencer::Sequencer;
use crate::transport::Transport;

pub const SCOPE_LEN: usize = 1024;
pub const OUTPUT_GAIN: f32 = 4.5;
const FADE_IN_BARS: f32 = 4.0;
const FADE_OUT_BARS: f32 = 8.0;

pub fn fade_envelope(bar_pos: f32, total_bars: f32) -> f32 {
    let fade_in = (bar_pos / FADE_IN_BARS).clamp(0.0, 1.0);
    let out_start = (total_bars - FADE_OUT_BARS).max(0.0);
    let fade_out = if bar_pos <= out_start {
        1.0
    } else {
        ((total_bars - bar_pos) / FADE_OUT_BARS).clamp(0.0, 1.0)
    };
    fade_in.min(fade_out)
}

#[derive(Clone, Copy)]
struct CountIn {
    left: u32,
    frame: u64,
    next_click: u64,
}

pub struct RenderCore {
    pub pool: VoicePool,
    pub seq: Sequencer,
    pub reverb: Reverb,
    pub transport: Arc<Transport>,
    lib: Arc<VoiceLibrary>,
    song: Song,
    sr: f32,
    master: f32,
    loop_bars: Option<(f32, f32)>,
    metronome: bool,
    count_in: u32,
    count: Option<CountIn>,
    last_click_beat: i64,
    hp: Biquad,
    shelf: Biquad,
    comp: Compressor,
    scope: Arc<[AtomicU32; SCOPE_LEN]>,
    scope_idx: usize,
    peak_l: f32,
    peak_r: f32,
}

impl RenderCore {
    pub fn new(
        song: Song,
        sr: f32,
        scope: Arc<[AtomicU32; SCOPE_LEN]>,
        lib: Arc<VoiceLibrary>,
    ) -> Self {
        let mut core = Self {
            pool: VoicePool::new(64),
            seq: Sequencer::new(),
            reverb: Reverb::new(sr),
            transport: Arc::new(Transport::default()),
            master: song.master,
            loop_bars: None,
            metronome: false,
            count_in: 0,
            count: None,
            last_click_beat: -1,
            hp: Biquad::highpass(40.0, 0.7, sr),
            shelf: Biquad::highshelf(6000.0, -1.5, sr),
            comp: Compressor::new(-24.0, 1.5, 20.0, 400.0, sr),
            song,
            sr,
            lib,
            scope,
            scope_idx: 0,
            peak_l: 0.0,
            peak_r: 0.0,
        };
        core.reload();
        core
    }
    pub fn song(&self) -> &Song {
        &self.song
    }
    pub fn sample_rate(&self) -> f32 {
        self.sr
    }
    fn reload(&mut self) {
        self.apply_song_params();
        self.seq.load(&self.song, self.sr);
    }
    fn apply_song_params(&mut self) {
        self.reverb.set_size(self.song.reverb_size);
        self.reverb.set_damp(self.song.reverb_damp);
        self.reverb.set_wet(self.song.reverb_wet);
    }
    fn reload_at(&mut self, bar: f32) {
        let was_playing = self.seq.playing;
        self.reload();
        self.seq.seek_to_bar(bar, &self.song);
        self.seq.playing = was_playing;
    }
    pub fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Play => {
                self.pool.all_off();
                if self.count_in > 0 {
                    self.count = Some(CountIn {
                        left: self.count_in,
                        frame: 0,
                        next_click: 0,
                    });
                } else {
                    self.seq.start(&self.song);
                    self.last_click_beat = -1;
                }
            }
            Command::Stop => {
                self.count = None;
                self.seq.playing = false;
                self.pool.all_off();
                self.reverb.clear();
            }
            Command::Seek(bar) => {
                self.seq.seek_to_bar(bar, &self.song);
                self.pool.all_off();
                self.last_click_beat = -1;
                self.transport
                    .set_position(self.seq.position_bars(&self.song));
                self.transport.set_bar(self.seq.current_bar(&self.song));
            }
            Command::Set(param) => self.set_param(param),
            Command::Reload(song) => {
                let bar = self.seq.position_bars(&self.song);
                self.song = song;
                self.master = self.song.master;
                self.reload_at(bar);
            }
            Command::UpdateVoices(lib) => {
                self.lib = Arc::new(lib);
            }
            Command::PreviewPreset(preset, midi) => {
                let freq = crate::voice::midi_to_freq(midi as f32);
                let id = self.seq.next_note_id();
                self.pool
                    .trigger_sine(&preset, freq, 2.0, 0.9, 0.0, self.sr, id);
            }
            Command::PreviewVoice(voice, midi) => {
                let preset = self.lib.preset(voice);
                let freq = crate::voice::midi_to_freq(midi as f32);
                if voice == sonora_core::voice::builtin::DRUMS {
                    let id = self.seq.next_note_id();
                    self.pool.trigger_drum(midi, 0.3, 0.9, 0.0, self.sr, id);
                } else {
                    let id = self.seq.next_note_id();
                    self.pool
                        .trigger_sine(&preset, freq, 2.0, 0.9, 0.0, self.sr, id);
                }
            }
            Command::SetLoop(range) => {
                self.loop_bars = range.and_then(|(a, b)| {
                    if b > a + 1e-3 {
                        Some((a.max(0.0), b))
                    } else {
                        None
                    }
                });
            }
            Command::SetMetronome(on) => {
                self.metronome = on;
            }
            Command::SetCountIn(beats) => {
                self.count_in = beats.min(16);
            }
            Command::AllOff => self.pool.all_off(),
        }
    }
    fn beat_frames(&self) -> u64 {
        ((self.song.bar_secs() / 4.0) * self.sr).max(1.0) as u64
    }
    fn trigger_click(&mut self, accent: bool) {
        let preset = crate::preset::click(accent);
        let freq = if accent { 2200.0 } else { 1600.0 };
        let id = self.seq.next_note_id();
        self.pool
            .trigger_sine(&preset, freq, 0.06, 0.9, 0.0, self.sr, id);
    }
    fn set_param(&mut self, param: Param) {
        match param {
            Param::Master(v) => {
                self.master = v.clamp(0.0, 1.0);
                self.song.master = self.master;
            }
            Param::ReverbSize(v) => {
                self.song.reverb_size = v;
                self.reverb.set_size(v);
            }
            Param::ReverbDamp(v) => {
                self.song.reverb_damp = v;
                self.reverb.set_damp(v);
            }
            Param::ReverbWet(v) => {
                self.song.reverb_wet = v;
                self.reverb.set_wet(v);
            }
            Param::Bpm(v) => {
                let bar = self.seq.position_bars(&self.song);
                self.song.bpm = v.clamp(20.0, 300.0);
                self.reload_at(bar);
            }
            Param::TrackLevel(i, v) => {
                if let Some(t) = self.song.tracks.get_mut(i) {
                    t.level = v;
                }
            }
        }
    }
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        out_l.fill(0.0);
        out_r.fill(0.0);
        if let Some(mut count) = self.count {
            count.frame += out_l.len() as u64;
            let beat_frames = self.beat_frames();
            while count.left > 0 && count.next_click <= count.frame {
                let accent = count.left == self.count_in;
                self.trigger_click(accent);
                count.left -= 1;
                count.next_click += beat_frames;
            }
            if count.left == 0 {
                self.count = None;
                self.seq.start(&self.song);
                self.last_click_beat = -1;
            } else {
                self.count = Some(count);
            }
        }
        if self.seq.playing {
            self.seq
                .advance(&mut self.pool, &self.lib, &self.song, out_l.len(), 1.0);
            if let Some((start, end)) = self.loop_bars {
                if self.seq.position_bars(&self.song) >= end {
                    self.seq.seek_to_bar(start, &self.song);
                }
            }
            if self.metronome {
                let beat = (self.seq.position_bars(&self.song) * 4.0).floor() as i64;
                if beat > self.last_click_beat {
                    self.trigger_click(beat.rem_euclid(4) == 0);
                    self.last_click_beat = beat;
                }
            }
        }
        let idle = !self.seq.playing && self.pool.active_count() == 0 && self.count.is_none();
        if idle {
            self.reverb.clear();
            self.transport.set_playing(false);
            self.transport
                .set_position(self.seq.position_bars(&self.song));
            self.transport.set_bar(self.seq.current_bar(&self.song));
            self.transport.set_voices(0);
            self.transport.push_peaks(0.0, 0.0);
            return;
        }
        self.pool.render(out_l, out_r, self.sr);
        let bar = if self.seq.playing {
            self.seq.current_bar(&self.song)
        } else {
            self.song.bars
        };
        let swell = self.song.swell(bar);
        let gain = (0.35 + swell * 0.65) * self.master * OUTPUT_GAIN;
        let fade = if self.seq.playing {
            let bar_pos = self.seq.frame as f32 / (self.song.bar_secs() * self.sr).max(1.0);
            fade_envelope(bar_pos, self.song.bars as f32)
        } else {
            1.0
        };
        self.reverb.process(out_l, out_r);
        let scale = gain * fade;
        let mut peak_l = 0.0f32;
        let mut peak_r = 0.0f32;
        for n in 0..out_l.len() {
            let mut l = out_l[n] * scale;
            let mut r = out_r[n] * scale;
            l = self.hp.process(l);
            r = self.hp.process(r);
            l = self.shelf.process(l);
            r = self.shelf.process(r);
            let mono = (l + r) * 0.5;
            let cgain = if mono.abs() > 1e-4 {
                let c = self.comp.process(mono);
                (c / mono).clamp(0.25, 1.0)
            } else {
                self.comp.process(0.0);
                1.0
            };
            let out_l_s = if l.abs() > 1e-5 {
                soft_clip(l * cgain)
            } else {
                l * cgain
            };
            let out_r_s = if r.abs() > 1e-5 {
                soft_clip(r * cgain)
            } else {
                r * cgain
            };
            let l_final = out_l_s.clamp(-1.0, 1.0);
            let r_final = out_r_s.clamp(-1.0, 1.0);
            out_l[n] = l_final;
            out_r[n] = r_final;
            peak_l = peak_l.max(l_final.abs());
            peak_r = peak_r.max(r_final.abs());
            let idx = self.scope_idx;
            self.scope[idx].store(((l_final + r_final) * 0.5).to_bits(), Ordering::Relaxed);
            self.scope_idx = (self.scope_idx + 1) % SCOPE_LEN;
        }
        self.peak_l = peak_l;
        self.peak_r = peak_r;
        self.transport.set_playing(self.seq.playing);
        self.transport
            .set_position(self.seq.position_bars(&self.song));
        self.transport.set_bar(self.seq.current_bar(&self.song));
        self.transport.set_voices(self.pool.active_count() as u32);
        self.transport.push_peaks(self.peak_l, self.peak_r);
        if self.seq.finished() {
            self.seq.playing = false;
            self.pool.all_off();
        }
    }
    pub fn scope_write_index(&self) -> usize {
        self.scope_idx
    }
    pub fn scope_snapshot(&self, out: &mut Vec<f32>) {
        out.clear();
        out.reserve(SCOPE_LEN);
        for i in 0..SCOPE_LEN {
            let idx = (self.scope_idx + i) % SCOPE_LEN;
            out.push(f32::from_bits(self.scope[idx].load(Ordering::Relaxed)));
        }
    }
}

pub fn render_offline(song: &Song, sr: u32, lib: Arc<VoiceLibrary>) -> (Vec<f32>, Vec<f32>) {
    let scope: Arc<[AtomicU32; SCOPE_LEN]> = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
    let mut core = RenderCore::new(song.clone(), sr as f32, scope, lib);
    core.handle(Command::Play);
    let total = ((song.duration_secs() + 8.0) * sr as f32) as usize;
    let mut l = vec![0.0f32; total];
    let mut r = vec![0.0f32; total];
    const BLOCK: usize = 512;
    let mut pos = 0;
    while pos < total {
        let n = BLOCK.min(total - pos);
        core.render(&mut l[pos..pos + n], &mut r[pos..pos + n]);
        pos += n;
    }
    (l, r)
}
#[cfg(test)]
mod tests {
    use super::*;
    use sonora_core::song::{Note, Track};
    use sonora_core::voice::builtin;
    const SR: f32 = 44100.0;
    fn core_with(tracks: Vec<Track>, bars: u32) -> RenderCore {
        let mut song = Song::empty();
        song.tracks = tracks;
        song.bars = bars;
        song.bpm = 120.0;
        let scope: Arc<[AtomicU32; SCOPE_LEN]> =
            Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
        RenderCore::new(song, SR, scope, Arc::new(VoiceLibrary::builtins()))
    }
    fn note(bar: u32, beat: f32, midi: u8) -> Note {
        Note {
            bar,
            beat,
            midi,
            len: 1.0,
            vel: 0.8,
        }
    }
    #[test]
    fn silence_when_idle() {
        let mut core = core_with(vec![Track::new(builtin::PIANO, false, 1.0)], 4);
        let n = 512;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        core.render(&mut l, &mut r);
        assert!(l.iter().all(|s| *s == 0.0));
        assert!(!core.transport.is_playing());
    }
    #[test]
    fn play_produces_audible_output() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(2, 1.0, 64)];
        let mut core = core_with(vec![t], 4);
        core.handle(Command::Play);
        let n = SR as usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        core.render(&mut l, &mut r);
        let peak = l
            .iter()
            .chain(r.iter())
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        assert!(peak > 0.001, "应产生信号，实际 {peak}");
    }
    #[test]
    fn output_never_exceeds_unity() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = (1..=8).map(|b| note(b, 1.0, 60)).collect();
        let mut core = core_with(vec![t], 8);
        core.handle(Command::Play);
        let n = SR as usize * 4;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        core.render(&mut l, &mut r);
        for s in l.iter().chain(r.iter()) {
            assert!(s.abs() <= 1.0, "输出 {s} 超出满量程");
        }
    }
    #[test]
    fn stop_clears_reverb_tail() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60)];
        let mut core = core_with(vec![t], 4);
        core.handle(Command::Play);
        let n = 512;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        core.render(&mut l, &mut r);
        core.handle(Command::Stop);
        core.render(&mut l, &mut r);
        assert!(l.iter().chain(r.iter()).all(|s| *s == 0.0));
    }
    #[test]
    fn bpm_change_keeps_position() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(3, 1.0, 64), note(5, 1.0, 67)];
        let mut core = core_with(vec![t], 8);
        core.handle(Command::Play);
        let one_bar = (core.song().bar_secs() * SR) as usize;
        let mut l = vec![0.0f32; one_bar * 2];
        let mut r = vec![0.0f32; one_bar * 2];
        core.render(&mut l, &mut r);
        let before = core.seq.position_bars(core.song());
        core.handle(Command::Set(Param::Bpm(180.0)));
        let after = core.seq.position_bars(core.song());
        assert!(
            (after - before).abs() < 0.05,
            "改 BPM 后位置应保持：{before} → {after}"
        );
        assert!(core.seq.playing);
    }
    #[test]
    fn volume_change_does_not_reload_events() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60)];
        let mut core = core_with(vec![t], 4);
        let peak_events = core.seq.peak_events();
        for v in [0.1f32, 0.5, 0.9, 0.2] {
            core.handle(Command::Set(Param::TrackLevel(0, v)));
        }
        assert_eq!(core.seq.peak_events(), peak_events, "改音量不应重建事件表");
    }
    #[test]
    fn preview_makes_sound_without_playing() {
        let mut core = core_with(vec![Track::new(builtin::PIANO, false, 1.0)], 4);
        let preset = crate::preset::piano();
        core.handle(Command::PreviewPreset(Box::new(preset), 60));
        let n = SR as usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        core.render(&mut l, &mut r);
        let peak = l
            .iter()
            .chain(r.iter())
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        assert!(peak > 0.001, "试听应出声，实际 {peak}");
    }
    #[test]
    fn seek_updates_transport_position() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(4, 1.0, 64)];
        let mut core = core_with(vec![t], 8);
        core.handle(Command::Play);
        core.handle(Command::Seek(3.0));
        let pos = core.transport.position_bars();
        assert!(
            (pos - 3.0).abs() < 0.01,
            "transport 应报告 3 小节，实际 {pos}"
        );
    }
    fn run_secs(core: &mut RenderCore, secs: f32) {
        let n = (SR * secs) as usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        core.render(&mut l, &mut r);
    }
    #[test]
    fn loop_wraps_back_to_the_start() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(5, 1.0, 64)];
        let mut core = core_with(vec![t], 8);
        core.handle(Command::SetLoop(Some((0.0, 1.0))));
        core.handle(Command::Play);
        core.handle(Command::Seek(0.0));
        run_secs(&mut core, 3.0);
        let pos = core.transport.position_bars();
        assert!(
            (0.0..1.0).contains(&pos),
            "循环播放应始终停在区间内，实际 {pos}"
        );
        assert!(core.seq.playing, "循环时不应停下");
    }
    #[test]
    fn loop_off_plays_through_the_whole_song() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(4, 1.0, 64)];
        let mut core = core_with(vec![t], 8);
        core.handle(Command::SetLoop(None));
        core.handle(Command::Play);
        run_secs(&mut core, 9.0);
        let pos = core.transport.position_bars();
        assert!(pos > 3.0, "没有循环时应一直往前播，实际 {pos}");
    }
    #[test]
    fn degenerate_loop_range_is_ignored() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(4, 1.0, 64)];
        let mut core = core_with(vec![t], 8);
        core.handle(Command::SetLoop(Some((3.0, 3.0))));
        core.handle(Command::Play);
        run_secs(&mut core, 9.0);
        assert!(core.transport.position_bars() > 3.0);
    }
    fn peak_of(core: &mut RenderCore, secs: f32) -> f32 {
        let n = (SR * secs) as usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        core.render(&mut l, &mut r);
        l.iter()
            .chain(r.iter())
            .map(|s| s.abs())
            .fold(0.0f32, f32::max)
    }
    #[test]
    fn metronome_clicks_even_with_no_notes() {
        let mut core = core_with(vec![Track::new(builtin::PIANO, false, 0.9)], 4);
        core.handle(Command::SetMetronome(true));
        core.handle(Command::Play);
        let peak = peak_of(&mut core, 1.0);
        assert!(peak > 0.001, "节拍器应踩出咔哒声，实际峰值 {peak}");
    }
    #[test]
    fn metronome_off_is_silent() {
        let mut core = core_with(vec![Track::new(builtin::PIANO, false, 0.9)], 4);
        core.handle(Command::SetMetronome(false));
        core.handle(Command::Play);
        let peak = peak_of(&mut core, 1.0);
        assert!(peak < 1e-4, "关掉节拍器后空曲子应静音，实际 {peak}");
    }
    #[test]
    fn count_in_delays_the_song() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60)];
        let mut core = core_with(vec![t], 4);
        core.handle(Command::SetCountIn(4));
        core.handle(Command::Play);
        run_secs(&mut core, 0.5);
        assert!(!core.seq.playing, "预卷期间不应开始播歌");
        run_secs(&mut core, 2.0);
        assert!(core.seq.playing, "数完 4 拍后应开始播放");
    }
    #[test]
    fn play_after_stop_resumes_instead_of_restarting() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(8, 1.0, 64)];
        let mut core = core_with(vec![t], 16);
        core.handle(Command::Play);
        run_secs(&mut core, 4.0);
        core.handle(Command::Stop);
        let stopped_at = core.transport.position_bars();
        assert!(stopped_at > 1.5, "应停在两小节附近，实际 {stopped_at}");
        core.handle(Command::Play);
        let resumed_at = core.transport.position_bars();
        assert!(
            (resumed_at - stopped_at).abs() < 0.01,
            "继续播放不应复位：停在 {stopped_at}，重播到了 {resumed_at}"
        );
    }
    #[test]
    fn play_after_finish_starts_from_the_beginning() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60)];
        let mut core = core_with(vec![t], 2);
        core.handle(Command::Play);
        run_secs(&mut core, 20.0);
        assert!(!core.seq.playing, "播到尾巴后应自动停下");
        core.handle(Command::Play);
        run_secs(&mut core, 0.05);
        let pos = core.transport.position_bars();
        assert!(pos < 0.1, "播完再按播放应回到开头，实际 {pos}");
        assert!(core.seq.playing);
    }
    #[test]
    fn offline_render_produces_sound() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(1, 3.0, 64)];
        let mut song = Song::empty();
        song.tracks = vec![t];
        song.bars = 4;
        song.bpm = 120.0;
        let (l, r) = render_offline(&song, SR as u32, Arc::new(VoiceLibrary::builtins()));
        assert_eq!(l.len(), r.len());
        let peak = l
            .iter()
            .chain(r.iter())
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        assert!(peak > 0.001, "离线渲染应出声，实际 {peak}");
        assert!(peak <= 1.0, "离线渲染不应溢出，实际 {peak}");
    }
    #[test]
    fn offline_render_is_deterministic() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60)];
        let mut song = Song::empty();
        song.tracks = vec![t];
        song.bars = 2;
        let lib = Arc::new(VoiceLibrary::builtins());
        let (l1, _) = render_offline(&song, SR as u32, lib.clone());
        let (l2, _) = render_offline(&song, SR as u32, lib);
        let diff: f32 = l1.iter().zip(l2.iter()).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff < 1e-3, "两次渲染应一致，差异 {diff}");
    }
    #[test]
    fn fade_envelope_is_flat_in_middle() {
        assert!(fade_envelope(0.0, 32.0) < 0.01);
        assert!((fade_envelope(16.0, 32.0) - 1.0).abs() < 1e-6);
        assert!(fade_envelope(32.0, 32.0) < 0.01);
    }
}
