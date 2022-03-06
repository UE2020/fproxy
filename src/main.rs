use std::io::{prelude::*, Error, ErrorKind};
use std::net::{TcpListener, TcpStream};
use std::thread::spawn;

pub mod parser;
pub use parser::Parser;

fn handle_client(stream: &mut TcpStream) -> std::io::Result<()> {
    // Check the method
    let mut buffer = vec![0; 8000];
    let len = stream.read(&mut buffer)?;
    if len == 0 {
        return Ok(());
    }
    buffer.resize(len, 0);
    let mut parser = Parser::new(unsafe { String::from_utf8_unchecked(buffer) });
    let method = parser
        .consume_until(" ")
        .ok_or(Error::new(ErrorKind::InvalidData, "No method"))?;
    match method {
        "CONNECT" => {
            // Get the hostname
            let hostname = parser
                .consume_until(" HTTP/1.1\r\n")
                .ok_or(Error::new(ErrorKind::InvalidData, "No hostname"))?;
            println!("{} (len={})", hostname, hostname.len());

            stream.write(b"HTTP/1.1 200 OK\r\n\r\n")?;

            let mut proxy_stream = TcpStream::connect(hostname)?;

            // Disable nagle
            proxy_stream.set_nodelay(true)?;
            stream.set_nodelay(true)?;

            {
                let mut proxy_stream = proxy_stream.try_clone()?;
                let mut stream = stream.try_clone()?;
                spawn(move || loop {
                    let mut buffer = [0; 65536];
                    match stream.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(len) => {
                            if proxy_stream.write_all(&buffer[..len]).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                });
            }

            loop {
                let mut buffer = [0; 65536];
                match proxy_stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(len) => {
                        if stream.write_all(&buffer[..len]).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            Ok(())
        }
        "GET" => {
            // Get the path
            let path = parser
                .consume_until(" HTTP/1.1\r\n")
                .ok_or(Error::new(ErrorKind::InvalidData, "No path"))?
                .to_string();
            println!("GET {} (len={})", path, path.len());

            // Get the headers
            let mut ua = String::new();
            loop {
                let line = parser
                    .consume_until("\r\n")
                    .ok_or(Error::new(ErrorKind::InvalidData, "No headers"))?
                    .to_string();
                if line.len() == 0 {
                    break;
                }
                let mut header_parser = Parser::new(line);
                let key = header_parser
                    .consume_until(":")
                    .ok_or(Error::new(ErrorKind::InvalidData, "No key"))?
                    .to_string();
                header_parser.consume_whitespaces();
                let value = header_parser.consume_until_end().to_string();
                if key == "User-Agent" {
                    ua = value.trim_start().to_string();
                }
            }

            // Make the http request
            let mut path_parser = Parser::new(path.to_string());
            // Consume the protocol
            let _ = path_parser.consume_until("://");
            let host = path_parser
                .consume_until("/")
                .ok_or(Error::new(ErrorKind::InvalidData, "No host"))?
                .to_string();

            let path = format!("/{}", path_parser.consume_until_end());

            let request = format!(
                "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: {}\r\n\r\n",
                path, host, ua
            );

            // Make the connection
            let mut proxy_stream = TcpStream::connect(format!("{}:80", host))?;
            proxy_stream.write_all(request.as_bytes())?;

            // Get the response
            let mut buffer = Vec::new();
            proxy_stream.read_to_end(&mut buffer)?;

            // Write the response
            stream.write_all(&buffer)?;

            Ok(())
        }
        _ => Err(Error::new(ErrorKind::Unsupported, "Invalid method")),
    }
}

fn main() -> std::io::Result<()> {
    // Get the port from the command line
    let port = std::env::args()
        .nth(1).expect("No port specified").parse::<u16>().expect("Invalid port");
    
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;

    // accept connections and process them serially
    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            spawn(move || {
                if let Err(_) = handle_client(&mut stream) {
                    let _ = stream.write(b"HTTP/1.1 400 Bad Request\r\n\r\n");
                }
            });
        }
    }
    Ok(())
}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn parser_consume_until() {
        let mut parser = Parser::new("qwertyuioptest".to_string());
        assert_eq!(parser.consume_until("test"), Some("qwertyuiop"));

        let mut parser = Parser::new("nothing".to_string());
        assert_eq!(parser.consume_until("test"), None);

        let mut parser = Parser::new("randomdata   test1 test2".to_string());
        assert_eq!(parser.consume_until("test"), Some("randomdata   "));
        assert_eq!(parser.consume_until("test"), Some("1 "));
    }
}
