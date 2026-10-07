//! Cooperative cancellation token and cancellation-aware streaming adapters.

use crate::error::{Result, TransferError};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cooperative cancellation token for transfer operations.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new active (uncancelled) token.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Triggers cancellation across all worker threads sharing this token.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns `true` if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Checks if cancellation was requested, returning `Err(TransferError::Cancelled)` if so.
    pub fn check_cancelled(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(TransferError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Helper stream checking cancellation before every bounded read.
pub struct CancellingReader<'a> {
    inner: &'a mut dyn Read,
    token: &'a CancellationToken,
}

impl<'a> CancellingReader<'a> {
    /// Wraps a reader with a cooperative cancellation token check.
    pub fn new(inner: &'a mut dyn Read, token: &'a CancellationToken) -> Self {
        Self { inner, token }
    }
}

impl<'a> Read for CancellingReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.token.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Transfer cancelled",
            ));
        }
        self.inner.read(buf)
    }
}

/// Helper stream checking cancellation before every bounded write.
pub struct CancellingWriter<'a> {
    inner: &'a mut dyn Write,
    token: &'a CancellationToken,
}

impl<'a> CancellingWriter<'a> {
    /// Wraps a writer with a cooperative cancellation token check.
    pub fn new(inner: &'a mut dyn Write, token: &'a CancellationToken) -> Self {
        Self { inner, token }
    }
}

impl<'a> Write for CancellingWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.token.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Transfer cancelled",
            ));
        }
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancellation_token_lifecycle() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        assert!(token.check_cancelled().is_ok());

        token.cancel();
        assert!(token.is_cancelled());
        assert!(matches!(
            token.check_cancelled(),
            Err(TransferError::Cancelled)
        ));
    }

    #[test]
    fn test_cancelling_reader_interrupts_stream() {
        let token = CancellationToken::new();
        let payload = b"Hello, stream cancellation test!";
        let mut slice = &payload[..];

        let mut reader = CancellingReader::new(&mut slice, &token);
        let mut buf = [0u8; 8];
        assert_eq!(reader.read(&mut buf).unwrap(), 8);

        token.cancel();
        let err = reader.read(&mut buf);
        assert!(err.is_err());
        assert_eq!(err.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
    }
}
