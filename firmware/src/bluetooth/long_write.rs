use core::fmt;

use defmt::Format;
use heapless::Vec;

pub struct ConnectionContext {
    pub long_write: LongWriteAccumulator<1024>,
}

#[derive(Debug)]
pub struct LongWriteAccumulator<const N: usize> {
    buf: Vec<u8, N>,
    expected_handle: Option<u16>,
}

pub enum GenericWrite<'a, D> {
    Short(D),
    Long { data: &'a [u8], handle: u16 },
}

impl<const N: usize> Default for LongWriteAccumulator<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Format, Clone, Copy)]
pub enum LongWriteError {
    UnexpectedHandle,
    IncorrectLength,
}

impl fmt::Display for LongWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = match self {
            Self::UnexpectedHandle => "UnexpectedHandle",
            Self::IncorrectLength => "IncorrectLength",
        };
        write!(f, "{}", d)
    }
}

impl core::error::Error for LongWriteError {}

impl<const N: usize> LongWriteAccumulator<N> {
    pub const fn new() -> Self {
        Self {
            buf: Vec::new(),
            expected_handle: None,
        }
    }

    pub fn prepare(
        &mut self,
        handle: u16,
        offset: usize,
        data: &[u8],
    ) -> Result<(), LongWriteError> {
        // First fragment
        if self.expected_handle.is_none() {
            self.expected_handle = Some(handle);
        }

        if self.expected_handle != Some(handle) {
            return Err(LongWriteError::UnexpectedHandle);
        }

        if offset != self.buf.len() {
            return Err(LongWriteError::IncorrectLength); // enforce strict ordering
        }

        self.buf
            .extend_from_slice(data)
            .map_err(|_| LongWriteError::IncorrectLength)?;
        Ok(())
    }

    pub fn execute(&mut self) -> (&[u8], u16) {
        let result = self.buf.as_slice();
        (result, self.expected_handle.unwrap())
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.expected_handle = None;
    }
}
