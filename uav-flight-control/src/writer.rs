/// Helper for formatting into a stack buffer
pub struct WriteBuf<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> WriteBuf<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.pos]
    }
}

impl<'a> core::fmt::Write for WriteBuf<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buf.len() - self.pos;
        let len = bytes.len().min(remaining);
        self.buf[self.pos..self.pos + len].copy_from_slice(&bytes[..len]);
        self.pos += len;
        Ok(())
    }
}
