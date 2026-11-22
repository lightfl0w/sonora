use std::path::Path;

pub const BITS_PER_SAMPLE: u16 = 16;

pub fn write_wav(
    path: &Path,
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
) -> std::io::Result<()> {
    use std::io::Write;

    let frames = left.len().min(right.len());
    let data_bytes = (frames * 4) as u32;

    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&2u16.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    header.extend_from_slice(&4u16.to_le_bytes());
    header.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_bytes.to_le_bytes());

    let mut file = std::fs::File::create(path)?;
    file.write_all(&header)?;

    let mut buf = Vec::with_capacity(8192);
    for i in 0..frames {
        let l = to_i16(left[i]);
        let r = to_i16(right[i]);
        buf.extend_from_slice(&l.to_le_bytes());
        buf.extend_from_slice(&r.to_le_bytes());
        if buf.len() >= 8192 {
            file.write_all(&buf)?;
            buf.clear();
        }
    }
    file.write_all(&buf)?;
    Ok(())
}

fn to_i16(sample: f32) -> i16 {
    if sample.is_finite() {
        (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_valid_riff() {
        let dir = std::env::temp_dir().join("sonora-wav-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.wav");

        let left = vec![0.0f32; 100];
        let right = vec![0.0f32; 100];
        write_wav(&path, &left, &right, 44100).unwrap();

        let data = std::fs::read(&path).unwrap();
        assert_eq!(&data[0..4], b"RIFF");
        assert_eq!(&data[8..12], b"WAVE");
        assert_eq!(&data[36..40], b"data");
        assert_eq!(data.len(), 44 + 100 * 4);
        assert_eq!(u32::from_le_bytes(data[40..44].try_into().unwrap()), 400);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn samples_are_clamped_not_wrapped() {
        assert_eq!(to_i16(5.0), i16::MAX);
        assert_eq!(to_i16(-5.0), -i16::MAX);
        assert_eq!(to_i16(0.0), 0);
        assert_eq!(to_i16(f32::NAN), 0);
    }

    #[test]
    fn shorter_channel_is_respected() {
        let dir = std::env::temp_dir().join("sonora-wav-test2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("b.wav");

        write_wav(&path, &[0.1; 10], &[0.1; 4], 8000).unwrap();
        let data = std::fs::read(&path).unwrap();
        assert_eq!(data.len(), 44 + 4 * 4);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
