use crate::{resample::Resampler, AudioDecoder, AudioEncodder};

pub struct PcmaDecoder {
    resample: Resampler<8000, 48000>,
    tmp_buf: [i16; 960],
}

impl Default for PcmaDecoder {
    fn default() -> Self {
        Self {
            resample: Default::default(),
            tmp_buf: [0; 960],
        }
    }
}

impl AudioDecoder for PcmaDecoder {
    fn decode(&mut self, in_buf: &[u8], out_buf: &mut [i16]) -> Option<usize> {
        decode_pcma(in_buf, &mut self.tmp_buf[0..in_buf.len()]);
        // upsample to 48k
        self.resample.resample(&self.tmp_buf[..in_buf.len()], out_buf)
    }
}

pub struct PcmaEncoder {
    resample: Resampler<48000, 8000>,
    tmp_buf: [i16; 960],
}

impl Default for PcmaEncoder {
    fn default() -> Self {
        Self {
            resample: Default::default(),
            tmp_buf: [0; 960],
        }
    }
}

impl AudioEncodder for PcmaEncoder {
    fn encode(&mut self, in_buf: &[i16], out_buf: &mut [u8]) -> Option<usize> {
        // downsample to 8k
        let out_samples = self.resample.resample(in_buf, &mut self.tmp_buf)?;
        encode_pcma(&self.tmp_buf[..out_samples], &mut out_buf[..out_samples]);
        Some(out_samples)
    }
}

/// µ-law to A-law conversion look-up table.
///
/// Copied from CCITT G.711 specifications.
#[allow(dead_code)]
const ULAW_TO_ALAW: [u8; 128] = [
    1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 27, 29, 31, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 46, 48, 49, 50, 51,
    52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101,
    102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128,
];

/// A-law to µ-law conversion look-up table.
///
/// Copied from CCITT G.711 specifications.
#[allow(dead_code)]
const ALAW_TO_ULAW: [u8; 128] = [
    1, 3, 5, 7, 9, 11, 13, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 32, 33, 33, 34, 34, 35, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 48, 49, 49, 50, 51,
    52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98,
    99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127,
];

const ALAW_TO_LINEAR: [i16; 256] = [
    -5504, -5248, -6016, -5760, -4480, -4224, -4992, -4736, -7552, -7296, -8064, -7808, -6528, -6272, -7040, -6784, -2752, -2624, -3008, -2880, -2240, -2112, -2496, -2368, -3776, -3648, -4032, -3904,
    -3264, -3136, -3520, -3392, -22016, -20992, -24064, -23040, -17920, -16896, -19968, -18944, -30208, -29184, -32256, -31232, -26112, -25088, -28160, -27136, -11008, -10496, -12032, -11520, -8960,
    -8448, -9984, -9472, -15104, -14592, -16128, -15616, -13056, -12544, -14080, -13568, -344, -328, -376, -360, -280, -264, -312, -296, -472, -456, -504, -488, -408, -392, -440, -424, -88, -72,
    -120, -104, -24, -8, -56, -40, -216, -200, -248, -232, -152, -136, -184, -168, -1376, -1312, -1504, -1440, -1120, -1056, -1248, -1184, -1888, -1824, -2016, -1952, -1632, -1568, -1760, -1696,
    -688, -656, -752, -720, -560, -528, -624, -592, -944, -912, -1008, -976, -816, -784, -880, -848, 5504, 5248, 6016, 5760, 4480, 4224, 4992, 4736, 7552, 7296, 8064, 7808, 6528, 6272, 7040, 6784,
    2752, 2624, 3008, 2880, 2240, 2112, 2496, 2368, 3776, 3648, 4032, 3904, 3264, 3136, 3520, 3392, 22016, 20992, 24064, 23040, 17920, 16896, 19968, 18944, 30208, 29184, 32256, 31232, 26112, 25088,
    28160, 27136, 11008, 10496, 12032, 11520, 8960, 8448, 9984, 9472, 15104, 14592, 16128, 15616, 13056, 12544, 14080, 13568, 344, 328, 376, 360, 280, 264, 312, 296, 472, 456, 504, 488, 408, 392,
    440, 424, 88, 72, 120, 104, 24, 8, 56, 40, 216, 200, 248, 232, 152, 136, 184, 168, 1376, 1312, 1504, 1440, 1120, 1056, 1248, 1184, 1888, 1824, 2016, 1952, 1632, 1568, 1760, 1696, 688, 656, 752,
    720, 560, 528, 624, 592, 944, 912, 1008, 976, 816, 784, 880, 848,
];

const ULAW_TO_LINEAR: [i16; 256] = [
    -32124, -31100, -30076, -29052, -28028, -27004, -25980, -24956, -23932, -22908, -21884, -20860, -19836, -18812, -17788, -16764, -15996, -15484, -14972, -14460, -13948, -13436, -12924, -12412,
    -11900, -11388, -10876, -10364, -9852, -9340, -8828, -8316, -7932, -7676, -7420, -7164, -6908, -6652, -6396, -6140, -5884, -5628, -5372, -5116, -4860, -4604, -4348, -4092, -3900, -3772, -3644,
    -3516, -3388, -3260, -3132, -3004, -2876, -2748, -2620, -2492, -2364, -2236, -2108, -1980, -1884, -1820, -1756, -1692, -1628, -1564, -1500, -1436, -1372, -1308, -1244, -1180, -1116, -1052, -988,
    -924, -876, -844, -812, -780, -748, -716, -684, -652, -620, -588, -556, -524, -492, -460, -428, -396, -372, -356, -340, -324, -308, -292, -276, -260, -244, -228, -212, -196, -180, -164, -148,
    -132, -120, -112, -104, -96, -88, -80, -72, -64, -56, -48, -40, -32, -24, -16, -8, -1, 32124, 31100, 30076, 29052, 28028, 27004, 25980, 24956, 23932, 22908, 21884, 20860, 19836, 18812, 17788,
    16764, 15996, 15484, 14972, 14460, 13948, 13436, 12924, 12412, 11900, 11388, 10876, 10364, 9852, 9340, 8828, 8316, 7932, 7676, 7420, 7164, 6908, 6652, 6396, 6140, 5884, 5628, 5372, 5116, 4860,
    4604, 4348, 4092, 3900, 3772, 3644, 3516, 3388, 3260, 3132, 3004, 2876, 2748, 2620, 2492, 2364, 2236, 2108, 1980, 1884, 1820, 1756, 1692, 1628, 1564, 1500, 1436, 1372, 1308, 1244, 1180, 1116,
    1052, 988, 924, 876, 844, 812, 780, 748, 716, 684, 652, 620, 588, 556, 524, 492, 460, 428, 396, 372, 356, 340, 324, 308, 292, 276, 260, 244, 228, 212, 196, 180, 164, 148, 132, 120, 112, 104, 96,
    88, 80, 72, 64, 56, 48, 40, 32, 24, 16, 8, 0,
];

/// Convert an 8-bit A-law value to a 16-bit LPCM sample.
#[inline]
fn alaw_to_linear(alaw_value: u8) -> i16 {
    ALAW_TO_LINEAR[(alaw_value) as usize]
}

/// Convert an 8-bit µ-law value to a 16-bit LPCM sample.
#[inline]
fn ulaw_to_linear(ulaw_value: u8) -> i16 {
    ULAW_TO_LINEAR[ulaw_value as usize]
}

/// Convert a 16-bit LPCM sample to an 8-bit A-law value.
#[allow(overflowing_literals, unused_comparisons)]
fn linear_to_alaw(sample: i16) -> u8 {
    let mut pcm_value = sample;
    let sign = (pcm_value & 0x8000) >> 8;
    if sign != 0 {
        pcm_value = -pcm_value;
    }
    let mut exponent: i16 = 7;
    let mut mask = 0x4000;
    while pcm_value & mask == 0 && exponent > 0 {
        exponent -= 1;
        mask >>= 1;
    }
    let manitssa: i16 = if exponent == 0 {
        (pcm_value >> 4) & 0x0f
    } else {
        (pcm_value >> (exponent + 3)) & 0x0f
    };
    let alaw_value = sign | exponent << 4 | manitssa;
    (alaw_value ^ 0xd5) as u8
}

/// Convert a 16-bit LPCM sample to an 8-bit µ-law value.
fn linear_to_ulaw(sample: i16) -> u8 {
    let mut pcm_value = sample;
    let sign = (pcm_value >> 8) & 0x80;
    if sign != 0 {
        pcm_value = -pcm_value;
    }
    if pcm_value > 32635 {
        pcm_value = 32635;
    }
    pcm_value += 0x84;
    let mut exponent: i16 = 7;
    let mut mask = 0x4000;
    while pcm_value & mask == 0 {
        exponent -= 1;
        mask >>= 1;
    }
    let manitssa: i16 = (pcm_value >> (exponent + 3)) & 0x0f;
    let ulaw_value = sign | exponent << 4 | manitssa;
    (!ulaw_value) as u8
}

pub fn encode_pcma(input: &[i16], encoded: &mut [u8]) {
    assert_eq!(input.len(), encoded.len());
    for i in 0..input.len() {
        encoded[i] = linear_to_alaw(input[i]);
    }
}

pub fn encode_pcma_f32(input: &[f32], encoded: &mut [u8]) {
    assert_eq!(input.len(), encoded.len());
    for i in 0..input.len() {
        encoded[i] = linear_to_alaw((input[i] * 32768.0) as i16);
    }
}

pub fn encode_pcmu(input: &[i16], encoded: &mut [u8]) {
    assert_eq!(input.len(), encoded.len());
    let mut encoded = vec![0; input.len()];
    for i in 0..input.len() {
        encoded[i] = linear_to_ulaw(input[i]);
    }
}

pub fn encode_pcmu_f32(input: &[f32], encoded: &mut [u8]) {
    assert_eq!(input.len(), encoded.len());
    let mut encoded = vec![0; input.len()];
    for i in 0..input.len() {
        encoded[i] = linear_to_ulaw((input[i] * 32768.0) as i16);
    }
}

pub fn decode_pcma(input: &[u8], decoded: &mut [i16]) {
    assert_eq!(input.len(), decoded.len());
    for i in 0..input.len() {
        decoded[i] = alaw_to_linear(input[i]);
    }
}

pub fn decode_pcma_f32(input: &[u8], decoded: &mut [f32]) {
    assert_eq!(input.len(), decoded.len());
    for i in 0..input.len() {
        decoded[i] = alaw_to_linear(input[i]) as f32 / 32768.0;
    }
}

pub fn decode_pcma_f32_keep(input: &[u8], decoded: &mut [f32]) {
    assert_eq!(input.len(), decoded.len());
    for i in 0..input.len() {
        decoded[i] = alaw_to_linear(input[i]) as f32;
    }
}

pub fn decode_pcmu(input: &[u8], decoded: &mut [i16]) {
    assert_eq!(input.len(), decoded.len());
    for i in 0..input.len() {
        decoded[i] = ulaw_to_linear(input[i]);
    }
}

pub fn decode_pcmu_f32(input: &[u8], decoded: &mut [f32]) {
    assert_eq!(input.len(), decoded.len());
    for i in 0..input.len() {
        decoded[i] = ulaw_to_linear(input[i]) as f32 / 32768.0;
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_pcma, encode_pcma};

    #[test]
    fn decode() {
        let pcma = vec![
            85, 85, 85, 85, 85, 213, 85, 213, 213, 213, 213, 85, 213, 85, 213, 213, 213, 85, 85, 213, 85, 85, 85, 213, 85, 85, 85, 85, 213, 85, 213, 213, 213, 85, 213, 213, 85, 213, 85, 85, 85, 213,
            85, 85, 212, 85, 213, 85, 85, 85, 85, 213, 85, 212, 213, 85, 213, 84, 85, 213, 213, 213, 212, 213, 84, 84, 213, 213, 85, 212, 213, 213, 85, 84, 213, 85, 212, 213, 85, 213, 84, 213, 85,
            213, 213, 85, 212, 84, 213, 85, 213, 85, 213, 213, 85, 212, 84, 85, 213, 215, 85, 213, 213, 87, 85, 87, 212, 213, 215, 213, 84, 212, 86, 213, 85, 215, 213, 84, 212, 87, 84, 84, 215, 215,
            85, 85, 213, 84, 84, 212, 215, 209, 87, 86, 213, 85, 85, 212, 208, 213, 81, 84, 84, 213, 212, 213, 214, 213, 80, 87, 213, 215, 212, 215, 212, 87, 86, 86, 212, 214,
        ];
        let mut raw = vec![0; pcma.len()];
        decode_pcma(&pcma, &mut raw);
        let expected: Vec<i16> = vec![
            -8, -8, -8, -8, -8, 8, -8, 8, 8, 8, 8, -8, 8, -8, 8, 8, 8, -8, -8, 8, -8, -8, -8, 8, -8, -8, -8, -8, 8, -8, 8, 8, 8, -8, 8, 8, -8, 8, -8, -8, -8, 8, -8, -8, 24, -8, 8, -8, -8, -8, -8, 8,
            -8, 24, 8, -8, 8, -24, -8, 8, 8, 8, 24, 8, -24, -24, 8, 8, -8, 24, 8, 8, -8, -24, 8, -8, 24, 8, -8, 8, -24, 8, -8, 8, 8, -8, 24, -24, 8, -8, 8, -8, 8, 8, -8, 24, -24, -8, 8, 40, -8, 8, 8,
            -40, -8, -40, 24, 8, 40, 8, -24, 24, -56, 8, -8, 40, 8, -24, 24, -40, -24, -24, 40, 40, -8, -8, 8, -24, -24, 24, 40, 72, -40, -56, 8, -8, -8, 24, 88, 8, -72, -24, -24, 8, 24, 8, 56, 8,
            -88, -40, 8, 40, 24, 40, 24, -40, -56, -56, 24, 56,
        ];
        assert_eq!(raw, expected);
    }

    #[test]
    fn encode() {
        let raw: Vec<i16> = vec![
            -8, -8, -8, -8, -8, 8, -8, 8, 8, 8, 8, -8, 8, -8, 8, 8, 8, -8, -8, 8, -8, -8, -8, 8, -8, -8, -8, -8, 8, -8, 8, 8, 8, -8, 8, 8, -8, 8, -8, -8, -8, 8, -8, -8, 24, -8, 8, -8, -8, -8, -8, 8,
            -8, 24, 8, -8, 8, -24, -8, 8, 8, 8, 24, 8, -24, -24, 8, 8, -8, 24, 8, 8, -8, -24, 8, -8, 24, 8, -8, 8, -24, 8, -8, 8, 8, -8, 24, -24, 8, -8, 8, -8, 8, 8, -8, 24, -24, -8, 8, 40, -8, 8, 8,
            -40, -8, -40, 24, 8, 40, 8, -24, 24, -56, 8, -8, 40, 8, -24, 24, -40, -24, -24, 40, 40, -8, -8, 8, -24, -24, 24, 40, 72, -40, -56, 8, -8, -8, 24, 88, 8, -72, -24, -24, 8, 24, 8, 56, 8,
            -88, -40, 8, 40, 24, 40, 24, -40, -56, -56, 24, 56,
        ];
        let expected = vec![
            85, 85, 85, 85, 85, 213, 85, 213, 213, 213, 213, 85, 213, 85, 213, 213, 213, 85, 85, 213, 85, 85, 85, 213, 85, 85, 85, 85, 213, 85, 213, 213, 213, 85, 213, 213, 85, 213, 85, 85, 85, 213,
            85, 85, 212, 85, 213, 85, 85, 85, 85, 213, 85, 212, 213, 85, 213, 84, 85, 213, 213, 213, 212, 213, 84, 84, 213, 213, 85, 212, 213, 213, 85, 84, 213, 85, 212, 213, 85, 213, 84, 213, 85,
            213, 213, 85, 212, 84, 213, 85, 213, 85, 213, 213, 85, 212, 84, 85, 213, 215, 85, 213, 213, 87, 85, 87, 212, 213, 215, 213, 84, 212, 86, 213, 85, 215, 213, 84, 212, 87, 84, 84, 215, 215,
            85, 85, 213, 84, 84, 212, 215, 209, 87, 86, 213, 85, 85, 212, 208, 213, 81, 84, 84, 213, 212, 213, 214, 213, 80, 87, 213, 215, 212, 215, 212, 87, 86, 86, 212, 214,
        ];
        let mut encoded = vec![0; raw.len()];
        encode_pcma(&raw, &mut encoded);
        assert_eq!(encoded, expected);
    }
}
