//! Streaming WAV header builder for PCM16 audio output.

/// Build a 44-byte streaming WAV header (PCM16, 24kHz, mono).
/// Data chunk size set to u32::MAX for unknown-length streaming.
pub fn build_streaming_wav_header() -> Vec<u8> {
    let sample_rate: u32 = 24000;
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * u32::from(num_channels) * u32::from(bits_per_sample / 8);
    let block_align = num_channels * (bits_per_sample / 8);

    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&u32::MAX.to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes()); // PCM
    header.extend_from_slice(&num_channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(header.len(), 44);
    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_header_length() {
        assert_eq!(build_streaming_wav_header().len(), 44);
    }

    #[test]
    fn test_wav_header_magic_bytes() {
        let h = build_streaming_wav_header();
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(&h[8..12], b"WAVE");
        assert_eq!(&h[12..16], b"fmt ");
        assert_eq!(&h[36..40], b"data");
    }

    #[test]
    fn test_wav_header_pcm_format() {
        let h = build_streaming_wav_header();
        assert_eq!(u16::from_le_bytes([h[20], h[21]]), 1); // PCM
        assert_eq!(u16::from_le_bytes([h[22], h[23]]), 1); // mono
        assert_eq!(u32::from_le_bytes([h[24], h[25], h[26], h[27]]), 24000);
        assert_eq!(u16::from_le_bytes([h[34], h[35]]), 16); // 16-bit
    }
}
