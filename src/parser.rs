use std::io::Result;

#[derive(Debug)]
pub struct Parser {
    data: String,
    position: usize,
}

impl Parser {
    pub fn new(data: String) -> Self {
        Parser { data, position: 0 }
    }

    /// Consume the data until we hit the needle
    pub fn consume_until(&mut self, needle: &str) -> Option<&str> {
        let start = self.position;
        let mut s = 0;
        loop {
            let next = self.data.as_bytes().get(self.position)?;
            if *next == *needle.as_bytes().get(s)? {
                if s + 1 == needle.len() {
                    self.position += 1;
                    break;
                }
                s += 1;
            } else {
                s = 0;
            }
            self.position += 1;
        }

        Some(&self.data[start..self.position - 1 - s])
    }

    pub fn consume_until_end(&mut self) -> &str {
        let start = self.position;
        self.position = self.data.len();
        &self.data[start..self.position]
    }

    /// Returns false if the parser has reached the end of the data
    pub fn consume_whitespaces(&mut self) -> bool {
        loop {
            let next = self.data.as_bytes().get(self.position);
            if next.is_none() {
                return true;
            } else if !unsafe { next.unwrap_unchecked() }.is_ascii_whitespace() {
                return false;
            }
            self.position += 1;
        }
    }

    pub fn into_data(self) -> (String, usize) {
        (self.data, self.position)
    }

    pub fn wind(&mut self, position: usize) {
        self.position += position;
    }

    pub fn rewind(&mut self, position: usize) {
        self.position -= position;
    }

    pub fn position(&self) -> usize {
        self.position
    }
}
