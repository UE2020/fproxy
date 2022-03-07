use std::io::{prelude::*, Result};

#[derive(Debug)]
pub struct Parser<T: Read> {
    data: T,
    leftover: Option<u8>,
}

impl<T: Read> Parser<T> {
    pub fn new(data: T) -> Self {
        Parser { data, leftover: None }
    }

    /// Consume the data until we hit the needle
    pub fn consume_until(&mut self, needle: &str) -> Result<String> {
        let mut consumed: String = String::new();
        let mut streak = 0;
        loop {
            let byte = self.read_byte()?;
            let next = byte as char;
            if next == needle.chars().nth(streak).unwrap() {
                streak += 1;
                if streak == needle.len() {
                    break;
                }
            } else {
                streak = 0;
            }
            consumed.push(next);
        }

        // cut off the needle
        for _ in 0..streak - 1 {
            consumed.pop();
        }

        Ok(consumed)
    }

    fn read_byte(&mut self) -> Result<u8> {
        if let Some(b) = self.leftover {
            self.leftover = None;
            return Ok(b);
        }

        let mut buf = [0u8; 1];
        self.data.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    pub fn consume_until_end(&mut self) -> Result<String> {
        let mut buf = String::new();
        self.data.read_to_string(&mut buf)?;
        Ok(buf)
    }

    /// Returns false if the parser has reached the end of the data
    pub fn consume_whitespaces(&mut self) -> Result<()> {
        loop {
            let byte = self.read_byte()?;
            // check if whitespace
            if !(byte as char).is_whitespace() {
                self.leftover = Some(byte);
                break Ok(());
            }
        }
    }

    pub fn inner(&mut self) -> &mut T {
        &mut self.data
    }

    pub fn into_inner(self) -> T {
        self.data
    }
}
