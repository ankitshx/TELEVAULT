//! Streaming cryptographic hasher and bounded hash writers for TELEVAULT.

use crate::error::{IntegrityError, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

/// Bounded buffer size for streaming SHA-256 computation (64 KiB).
pub const STREAM_BUFFER_SIZE: usize = 64 * 1024;

/// Discards payload bytes immediately without memory allocation while computing a running SHA-256 hash.
#[derive(Debug, Default)]
pub struct NullHashWriter {
    hasher: Sha256,
    bytes_written: u64,
}

impl NullHashWriter {
    /// Creates a new [`NullHashWriter`].
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            bytes_written: 0,
        }
    }

    /// Returns the total number of bytes written so far.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Finalizes the hash computation and returns `(sha256_hex, total_bytes)`.
    pub fn finalize(self) -> (String, u64) {
        let hex = format!("{:x}", self.hasher.finalize());
        (hex, self.bytes_written)
    }
}

impl Write for NullHashWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hasher.update(buf);
        self.bytes_written += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A pass-through [`Write`] adapter that forwards all written bytes to an inner writer
/// while simultaneously calculating a running SHA-256 digest.
pub struct HashWriter<W: Write> {
    inner: W,
    hasher: Sha256,
    bytes_written: u64,
}

impl<W: Write> HashWriter<W> {
    /// Wraps an existing writer with a running SHA-256 digest.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes_written: 0,
        }
    }

    /// Returns the total number of bytes forwarded so far.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Finalizes the hash digest and returns `(inner_writer, sha256_hex, total_bytes)`.
    pub fn finalize(self) -> (W, String, u64) {
        let hex = format!("{:x}", self.hasher.finalize());
        (self.inner, hex, self.bytes_written)
    }
}

impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hasher.update(&buf[..written]);
        self.bytes_written += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Utility for stream hashing with bounded memory buffers.
pub struct StreamHasher;

impl StreamHasher {
    /// Computes the lowercase SHA-256 hex digest and total byte count from a [`Read`] stream
    /// using a bounded 64 KiB buffer.
    pub fn hash_reader<R: Read>(reader: &mut R) -> Result<(String, u64)> {
        let mut buffer = [0u8; STREAM_BUFFER_SIZE];
        let mut hasher = Sha256::new();
        let mut total_bytes: u64 = 0;

        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            total_bytes += n as u64;
        }

        let hex = format!("{:x}", hasher.finalize());
        Ok((hex, total_bytes))
    }

    /// Computes the lowercase SHA-256 hex digest of an in-memory byte slice.
    pub fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Reads from a stream and validates that its size and SHA-256 match expected values.
    pub fn verify_reader<R: Read>(
        reader: &mut R,
        expected_sha256: &str,
        expected_size: Option<u64>,
    ) -> Result<()> {
        let (calculated_hash, total_bytes) = Self::hash_reader(reader)?;

        if let Some(expected) = expected_size {
            if total_bytes != expected {
                return Err(IntegrityError::SizeMismatch {
                    expected,
                    actual: total_bytes,
                });
            }
        }

        if !calculated_hash.eq_ignore_ascii_case(expected_sha256) {
            return Err(IntegrityError::HashMismatch {
                expected: expected_sha256.to_string(),
                calculated: calculated_hash,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_hash_bytes_and_stream_hasher_equivalence() {
        let data = b"The quick brown fox jumps over the lazy dog";
        let expected = "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592";

        assert_eq!(StreamHasher::hash_bytes(data), expected);

        let mut cursor = Cursor::new(data);
        let (hash, count) = StreamHasher::hash_reader(&mut cursor).unwrap();
        assert_eq!(hash, expected);
        assert_eq!(count, data.len() as u64);
    }

    #[test]
    fn test_null_hash_writer() {
        let data = b"TELEVAULT bounded null hash writer test payload";
        let expected_hash = StreamHasher::hash_bytes(data);

        let mut writer = NullHashWriter::new();
        writer.write_all(data).unwrap();
        assert_eq!(writer.bytes_written(), data.len() as u64);

        let (hash, bytes) = writer.finalize();
        assert_eq!(hash, expected_hash);
        assert_eq!(bytes, data.len() as u64);
    }

    #[test]
    fn test_hash_writer_passthrough() {
        let data = b"Streaming through HashWriter directly to inner buffer";
        let expected_hash = StreamHasher::hash_bytes(data);

        let mut output = Vec::new();
        let mut writer = HashWriter::new(&mut output);
        writer.write_all(data).unwrap();

        let (_inner, hash, bytes) = writer.finalize();
        assert_eq!(hash, expected_hash);
        assert_eq!(bytes, data.len() as u64);
        assert_eq!(output, data);
    }

    #[test]
    fn test_verify_reader_mismatch_detection() {
        let data = b"Valid content";
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let mut cursor = Cursor::new(data);
        let res = StreamHasher::verify_reader(&mut cursor, wrong_hash, Some(data.len() as u64));
        assert!(matches!(res, Err(IntegrityError::HashMismatch { .. })));

        let mut cursor2 = Cursor::new(data);
        let correct_hash = StreamHasher::hash_bytes(data);
        let res2 = StreamHasher::verify_reader(&mut cursor2, &correct_hash, Some(999));
        assert!(matches!(res2, Err(IntegrityError::SizeMismatch { .. })));
    }
}
