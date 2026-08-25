// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The CPU's raw word representation and its typed bit conversions.

/// The value stored by the word-based CPU.
pub type Word = u64;

/// Encodes an [`i32`] in the low 32 bits of a word.
#[inline]
pub const fn encode_i32(value: i32) -> Word {
    value as u32 as Word
}

/// Decodes an [`i32`] from the low 32 bits of a word.
#[inline]
pub const fn decode_i32(word: Word) -> i32 {
    word as u32 as i32
}

/// Encodes a [`u32`] in the low 32 bits of a word.
#[inline]
pub const fn encode_u32(value: u32) -> Word {
    value as Word
}

/// Decodes a [`u32`] from the low 32 bits of a word.
#[inline]
pub const fn decode_u32(word: Word) -> u32 {
    word as u32
}

/// Encodes an [`i64`] without changing its bit pattern.
#[inline]
pub const fn encode_i64(value: i64) -> Word {
    value as Word
}

/// Decodes an [`i64`] without changing its bit pattern.
#[inline]
pub const fn decode_i64(word: Word) -> i64 {
    word as i64
}

/// Encodes a [`u64`] without changing its bit pattern.
#[inline]
pub const fn encode_u64(value: u64) -> Word {
    value
}

/// Decodes a [`u64`] without changing its bit pattern.
#[inline]
pub const fn decode_u64(word: Word) -> u64 {
    word
}

/// Encodes an [`f32`] using its IEEE-754 representation in the low 32 bits.
#[inline]
pub const fn encode_f32(value: f32) -> Word {
    value.to_bits() as Word
}

/// Decodes an [`f32`] from its IEEE-754 representation in the low 32 bits.
#[inline]
pub const fn decode_f32(word: Word) -> f32 {
    f32::from_bits(word as u32)
}

/// Encodes an [`f64`] using its IEEE-754 representation.
#[inline]
pub const fn encode_f64(value: f64) -> Word {
    value.to_bits()
}

/// Decodes an [`f64`] from its IEEE-754 representation.
#[inline]
pub const fn decode_f64(word: Word) -> f64 {
    f64::from_bits(word)
}

/// Encodes a boolean as its canonical word representation, `0` or `1`.
#[inline]
pub const fn encode_bool(value: bool) -> Word {
    value as Word
}

/// Decodes a boolean word. Zero is false and every nonzero word is true.
#[inline]
pub const fn decode_bool(word: Word) -> bool {
    word != 0
}

/// Encodes a numeric function reference.
#[inline]
pub const fn encode_function_ref(id: u32) -> Word {
    encode_u32(id)
}

/// Decodes a numeric function reference.
#[inline]
pub const fn decode_function_ref(word: Word) -> u32 {
    decode_u32(word)
}

/// Encodes a numeric heap reference.
#[inline]
pub const fn encode_heap_ref(id: u32) -> Word {
    encode_u32(id)
}

/// Decodes a numeric heap reference.
#[inline]
pub const fn decode_heap_ref(word: Word) -> u32 {
    decode_u32(word)
}

/// Encodes an interned string identifier.
#[inline]
pub const fn encode_string_id(id: u32) -> Word {
    encode_u32(id)
}

/// Decodes an interned string identifier.
#[inline]
pub const fn decode_string_id(word: Word) -> u32 {
    decode_u32(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_integers_are_canonicalized_to_low_32_bits() {
        assert_eq!(encode_i32(-1), u64::from(u32::MAX));
        assert_eq!(decode_i32(u64::MAX), -1);
        assert_eq!(encode_u32(u32::MAX), u64::from(u32::MAX));
        assert_eq!(decode_u32(u64::MAX), u32::MAX);
    }

    #[test]
    fn wide_integers_preserve_bits() {
        let signed = i64::MIN + 123;
        assert_eq!(decode_i64(encode_i64(signed)), signed);

        let unsigned = u64::MAX - 123;
        assert_eq!(decode_u64(encode_u64(unsigned)), unsigned);
    }

    #[test]
    fn floats_preserve_bits_including_nan_payloads() {
        let f32_bits = 0x7fc1_2345;
        assert_eq!(encode_f32(f32::from_bits(f32_bits)), u64::from(f32_bits));
        assert_eq!(decode_f32(u64::MAX).to_bits(), u32::MAX);

        let f64_bits = 0x7ff8_1234_5678_9abc;
        assert_eq!(encode_f64(f64::from_bits(f64_bits)), f64_bits);
        assert_eq!(decode_f64(f64_bits).to_bits(), f64_bits);
    }

    #[test]
    fn booleans_use_canonical_encoding() {
        assert_eq!(encode_bool(false), 0);
        assert_eq!(encode_bool(true), 1);
        assert!(!decode_bool(0));
        assert!(decode_bool(1));
        assert!(decode_bool(Word::MAX));
    }

    #[test]
    fn references_and_string_ids_are_numeric_words() {
        assert_eq!(decode_function_ref(encode_function_ref(7)), 7);
        assert_eq!(decode_heap_ref(encode_heap_ref(11)), 11);
        assert_eq!(decode_string_id(encode_string_id(13)), 13);
        assert_eq!(encode_heap_ref(u32::MAX), u64::from(u32::MAX));
    }
}
