use crate::error::MidiError;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    fn u16be(&mut self) -> Option<u16> {
        Some(((self.u8()? as u16) << 8) | self.u8()? as u16)
    }

    fn u32be(&mut self) -> Option<u32> {
        Some(((self.u16be()? as u32) << 16) | self.u16be()? as u32)
    }

    fn var_len(&mut self) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..4 {
            let b = self.u8()?;
            value = (value << 7) | (b & 0x7F) as u32;
            if b & 0x80 == 0 {
                return Some(value);
            }
        }
        Some(value)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }

    fn skip(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.data.len());
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawNote {
    pub start_beats: f64,
    pub len_beats: f64,
    pub midi: u8,
    pub vel: f32,
    pub channel: u8,
}

#[derive(Debug, Default)]
pub struct MidiFile {
    pub notes: Vec<RawNote>,
    pub ppq: u16,
    pub tempo_us_per_qn: u32,
}

impl MidiFile {
    pub fn bpm(&self) -> f32 {
        if self.tempo_us_per_qn == 0 {
            120.0
        } else {
            60_000_000.0 / self.tempo_us_per_qn as f32
        }
    }

    pub fn is_drum_channel(&self, channel: u8) -> bool {
        channel == 9
    }
}

pub fn parse(data: &[u8]) -> Result<MidiFile, MidiError> {
    let mut r = Reader::new(data);

    if r.take(4) != Some(b"MThd") {
        return Err(MidiError::NotMidi);
    }
    let header_len = r.u32be().ok_or(MidiError::Corrupt("header len"))?;
    let _format = r.u16be().ok_or(MidiError::Corrupt("format"))?;
    let n_tracks = r.u16be().ok_or(MidiError::Corrupt("track count"))?;
    let division = r.u16be().ok_or(MidiError::Corrupt("division"))?;

    if division & 0x8000 != 0 {
        return Err(MidiError::Smpte);
    }
    let ppq = division.max(1);

    if header_len > 6 {
        r.skip((header_len - 6) as usize);
    }

    let mut out = MidiFile {
        ppq,
        tempo_us_per_qn: 500_000,
        ..Default::default()
    };
    let mut tempo_seen = false;

    for _ in 0..n_tracks {
        loop {
            if r.eof() {
                break;
            }
            let id = match r.take(4) {
                Some(id) => id,
                None => break,
            };
            let len = match r.u32be() {
                Some(l) => l as usize,
                None => break,
            };
            if id == b"MTrk" {
                if let Some(track) = r.take(len) {
                    parse_track(track, &mut out, &mut tempo_seen);
                }
                break;
            } else {
                r.skip(len);
            }
        }
    }

    Ok(out)
}

fn parse_track(data: &[u8], out: &mut MidiFile, tempo_seen: &mut bool) {
    let mut r = Reader::new(data);
    let mut tick: u64 = 0;
    let mut running_status: u8 = 0;
    let mut open: Vec<((u8, u8), (u64, f32))> = Vec::new();

    while !r.eof() {
        let delta = match r.var_len() {
            Some(d) => d as u64,
            None => break,
        };
        tick += delta;

        let mut byte = match r.u8() {
            Some(b) => b,
            None => break,
        };

        if byte < 0x80 {
            if running_status == 0 {
                break;
            }
            r.pos -= 1;
            byte = running_status;
        }

        if byte == 0xFF {
            let kind = match r.u8() {
                Some(k) => k,
                None => break,
            };
            let len = match r.var_len() {
                Some(l) => l as usize,
                None => break,
            };
            let payload = r.take(len).unwrap_or(&[]);

            if kind == 0x51 && len >= 3 && !*tempo_seen {
                out.tempo_us_per_qn =
                    ((payload[0] as u32) << 16) | ((payload[1] as u32) << 8) | payload[2] as u32;
                *tempo_seen = true;
            }
            if kind == 0x2F {
                break;
            }
            continue;
        }

        if byte == 0xF0 || byte == 0xF7 {
            let len = match r.var_len() {
                Some(l) => l as usize,
                None => break,
            };
            r.skip(len);
            continue;
        }

        let status = byte;
        running_status = status;
        let kind = status & 0xF0;
        let channel = status & 0x0F;

        match kind {
            0x80 | 0x90 => {
                let note = match r.u8() {
                    Some(n) => n & 0x7F,
                    None => break,
                };
                let vel = match r.u8() {
                    Some(v) => v & 0x7F,
                    None => break,
                };
                let key = (channel, note);
                if kind == 0x90 && vel > 0 {
                    if let Some(slot) = open.iter_mut().find(|(k, _)| *k == key) {
                        slot.1 = (tick, vel as f32 / 127.0);
                    } else {
                        open.push((key, (tick, vel as f32 / 127.0)));
                    }
                } else if let Some(pos) = open.iter().position(|(k, _)| *k == key) {
                    let (_, (start, v)) = open.remove(pos);
                    out.notes.push(RawNote {
                        start_beats: start as f64 / out.ppq as f64,
                        len_beats: tick.saturating_sub(start).max(1) as f64 / out.ppq as f64,
                        midi: note,
                        vel: v,
                        channel,
                    });
                }
            }
            0xA0 | 0xB0 | 0xE0 => r.skip(2),
            0xC0 | 0xD0 => r.skip(1),
            _ => break,
        }
    }

    for ((ch, note), (start, v)) in open {
        out.notes.push(RawNote {
            start_beats: start as f64 / out.ppq as f64,
            len_beats: tick.saturating_sub(start).max(1) as f64 / out.ppq as f64,
            midi: note,
            vel: v,
            channel: ch,
        });
    }
}

pub fn read_file(path: &std::path::Path) -> Result<MidiFile, MidiError> {
    let bytes = std::fs::read(path)?;
    parse(&bytes)
}
