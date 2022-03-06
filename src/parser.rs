use std::io::{prelude::*, Result};

#[derive(Debug)]
pub struct Parser<T: Read> {
    data: T,
}

impl<T: Read> Parser<T> {
    pub fn new(data: T) -> Self {
        Parser { data }
    }

    /// Consume the data until we hit the needle
    pub fn consume_until(&mut self, needle: &str) -> Result<String> {
        let mut consumed: String = String::new();
        let mut streak = 0;
        loop {
            let mut buf = [0u8; 1];
            self.data.read_exact(&mut buf)?;
            let next = buf[0] as char;
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

    pub fn consume_until_end(&mut self) -> Result<String> {
        let mut buf = String::new();
        self.data.read_to_string(&mut buf)?;
        Ok(buf)
    }

    /// Returns false if the parser has reached the end of the data
    pub fn consume_whitespaces(&mut self) -> Result<()> {
        loop {
            let mut buf = [0u8; 1];
            self.data.read_exact(&mut buf)?;
            // check if whitespace
            if !(buf[0] as char).is_whitespace() {
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
