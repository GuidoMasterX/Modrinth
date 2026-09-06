//! MurmurHash2 (CurseForge variant) file fingerprinting
//!
//! CurseForge identifies files by a MurmurHash2 variant computed over the file
//! bytes with all 0x09, 0x0a, 0x0d and 0x20 bytes stripped, using a seed of 1
//! and 32-bit unsigned arithmetic.

/// Computes the CurseForge variant of MurmurHash2 for the given bytes.
pub fn murmur2(data: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    const R: u32 = 24;
    const SEED: u32 = 1;

    let data: Vec<u8> = data
        .iter()
        .copied()
        .filter(|byte| !matches!(byte, 0x09 | 0x0a | 0x0d | 0x20))
        .collect();

    let length = data.len() as u32;
    let mut hash = SEED ^ length;

    for block in data.chunks_exact(4) {
        let mut k =
            u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);
        hash = hash.wrapping_mul(M);
        hash ^= k;
    }

    let tail = &data[data.len() - data.len() % 4..];
    match tail.len() {
        3 => hash ^= u32::from(tail[2]) << 16,
        2 => hash ^= u32::from(tail[1]) << 8,
        1 => hash ^= u32::from(tail[0]),
        _ => {}
    }
    if !tail.is_empty() {
        hash = hash.wrapping_mul(M);
    }

    hash ^= hash >> 13;
    hash = hash.wrapping_mul(M);
    hash ^= hash >> 15;

    hash
}

#[cfg(test)]
mod tests {
    use super::murmur2;

    #[test]
    fn empty_input() {
        assert_eq!(murmur2(&[]), 1540447798);
    }

    #[test]
    fn known_vectors() {
        assert_eq!(murmur2(b"hello"), 2788266382);
        assert_eq!(murmur2(b"abcd"), 3376380438);
    }

    #[test]
    fn zero_byte() {
        assert_eq!(murmur2(&[0x00]), 0);
    }

    #[test]
    fn whitespace_bytes_are_stripped() {
        assert_eq!(murmur2(b"a b"), murmur2(b"ab"));
    }
}
