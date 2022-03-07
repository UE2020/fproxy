use std::io::{prelude::*, Cursor, Error, ErrorKind};
use std::net::{TcpListener, TcpStream};
use std::thread::spawn;
use std::collections::HashMap;

pub mod parser;
pub use parser::Parser;

fn handle_client(stream: &mut TcpStream) -> std::io::Result<()> {
    // Check the method
    println!("Connection");
    let mut parser = Parser::new(stream);
    let method = parser.consume_until(" ")?;

    match method.as_str() {
        "CONNECT" => {
            // Get the hostname
            let hostname = parser.consume_until(" HTTP/1.1\r\n")?;

            println!("CONNECT {}", hostname);
            
            parser.consume_until("\r\n\r\n")?;

            parser.inner().write(b"HTTP/1.1 200 OK\r\n\r\n")?;

            let mut proxy_stream = TcpStream::connect(hostname)?;

            let stream = parser.into_inner();

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
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" => {
            // Get the path
            let path = parser.consume_until(" HTTP/1.1\r\n")?;

            // Get the headers
            let mut headers = HashMap::new();
            loop {
                let line = parser.consume_until("\r\n")?;

                if line.len() == 0 {
                    break;
                }
                let mut header_parser = Parser::new(Cursor::new(line));
                let key = header_parser.consume_until(":")?;
                header_parser.consume_whitespaces()?;
                let value = header_parser.consume_until_end()?;
                if key.to_lowercase() != "host" && !key.to_lowercase().starts_with("proxy-") {
                    headers.insert(key, value);
                }
            }

            headers.insert("Connection".to_string(), "close".to_string());

            let content_length = match headers.get("Content-Length") {
                Some(value) => value.parse::<usize>().unwrap(),
                None => 0,
            };

            // Get the body
            let mut body = vec![0u8; content_length];
            if content_length > 0 {
                parser.inner().read_exact(&mut body)?;
            }

            // Make the http request
            let mut path_parser = Parser::new(Cursor::new(path.to_string()));
            // Consume the protocol
            let _ = path_parser.consume_until("://");
            let host = path_parser.consume_until("/")?;
            
            println!("{} {}", method, host);
            
            let path = format!("/{}", path_parser.consume_until_end()?);

            let request = if content_length > 0 {
                let mut request = format!(
                    "{} {} HTTP/1.1\r\nHost: {}\r\n{}\r\n\r\n",
                    method, path, host, headers.iter().map(|(k, v)| format!("{}: {}", k, v)).collect::<Vec<String>>().join("\r\n")
                ).as_bytes().to_vec();
    
                request.append(&mut body);
                request
            } else {
                format!(
                    "{} {} HTTP/1.1\r\nHost: {}\r\n{}\r\n\r\n",
                    method, path, host, headers.iter().map(|(k, v)| format!("{}: {}", k, v)).collect::<Vec<String>>().join("\r\n")
                ).as_bytes().to_vec()
            };

            // Make the connection
            let mut proxy_stream = TcpStream::connect(format!("{}:80", host))?;
            proxy_stream.write_all(&request)?;

            // Get the response
            let mut buffer = Vec::new();
            proxy_stream.read_to_end(&mut buffer)?;

            // Write the response
            parser.inner().write_all(&buffer)?;

            Ok(())
        }
        _ => Err(Error::new(ErrorKind::Unsupported, "Invalid method")),
    }
}

fn main() -> std::io::Result<()> {
    // Get the port from the command line
    let port = std::env::args()
        .nth(1)
        .expect("No port specified")
        .parse::<u16>()
        .expect("Invalid port");

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)?;
    println!("Listening on {}", addr);

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
        use std::io::Cursor;

        let mut parser = Parser::new(Cursor::new("qwertyuioptest".to_string()));
        assert_eq!(
            parser.consume_until("test").unwrap(),
            "qwertyuiop".to_string()
        );

        let mut parser = Parser::new(Cursor::new("nothing".to_string()));
        assert!(parser.consume_until("test").is_err());

        let mut parser = Parser::new(Cursor::new("randomdata   test1 test2".to_string()));
        assert_eq!(parser.consume_until("test").unwrap(), "randomdata   ");
        assert_eq!(parser.consume_until("test").unwrap(), "1 ");
    }
}
