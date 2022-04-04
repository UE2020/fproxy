use tokio::io::{ErrorKind, BufReader, AsyncWriteExt, AsyncReadExt};
use tokio::task::spawn;
use std::io::{Result, Error, Cursor};
use tokio::net::{TcpListener, TcpStream};
use std::collections::HashMap;

pub mod parser;
pub use parser::Parser;

async fn handle_client(stream: TcpStream) -> std::io::Result<()> {
    // Check the method
    let mut parser = Parser::new(BufReader::new(stream));
    let method = parser.consume_until(" ").await?;

    match method.as_str() {
        "CONNECT" => {
            // Get the hostname
            let hostname = parser.consume_until(" HTTP/1.1\r\n").await?;

            println!("CONNECT {}", hostname);
            
            parser.consume_until("\r\n\r\n").await?;

            parser.inner().get_mut().write(b"HTTP/1.1 200 OK\r\n\r\n").await?;

            let mut proxy_stream = TcpStream::connect(hostname).await?;

            let stream = parser.into_inner();
            let stream = stream.into_inner();

            // Disable nagle
            proxy_stream.set_nodelay(true)?;
            stream.set_nodelay(true)?;

            let (mut proxy_stream_read, mut proxy_stream_write) = proxy_stream.into_split();
            let (mut stream_read, mut stream_write) = stream.into_split();

            {
                spawn(async move { loop {
                    let mut buffer = [0; 65536];
                    match stream_read.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(len) => {
                            if proxy_stream_write.write_all(&buffer[..len]).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                } });
            }

            loop {
                let mut buffer = [0; 65536];
                match proxy_stream_read.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(len) => {
                        if stream_write.write_all(&buffer[..len]).await.is_err() {
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
            let path = parser.consume_until(" HTTP/1.1\r\n").await?;

            // Get the headers
            let mut headers = HashMap::new();
            loop {
                let line = parser.consume_until("\r\n").await?;

                if line.len() == 0 {
                    break;
                }
                let mut header_parser = Parser::new(Cursor::new(line));
                let key = header_parser.consume_until(":").await?;
                header_parser.consume_whitespaces().await?;
                let value = header_parser.consume_until_end().await?;
                if key.to_lowercase() != "host" && !key.to_lowercase().starts_with("proxy-") && key.to_lowercase() != "connection" {
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
                parser.inner().read_exact(&mut body).await?;
            }

            // Make the http request
            let mut path_parser = Parser::new(Cursor::new(path.to_string()));
            // Consume the protocol
            let _ = path_parser.consume_until("://").await;
            let host = path_parser.consume_until("/").await?;
            
            println!("{} {}", method, host);
            
            let path = format!("/{}", path_parser.consume_until_end().await?);

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
            let mut proxy_stream = TcpStream::connect(format!("{}:80", host)).await?;
            proxy_stream.write_all(&request).await?;

            // Get the response
            let mut buffer = Vec::new();
            proxy_stream.read_to_end(&mut buffer).await?;

            // Write the response
            parser.inner().get_mut().write_all(&buffer).await?;

            Ok(())
        }
        _ => Err(Error::new(ErrorKind::Unsupported, "Invalid method")),
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Get the port from the command line
    let port = std::env::args()
        .nth(1)
        .expect("No port specified")
        .parse::<u16>()
        .expect("Invalid port");

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on {}", addr);

    // accept connections and process them serially
    loop {
        let (stream, _) = listener.accept().await?;
        spawn(async {
            if let Err(_) = handle_client(stream).await {
                //let _ = stream.write(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            }
        });
    }

    Ok(())
}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn parser_consume_until() {
        use std::io::Cursor;

        let mut parser = Parser::new(Cursor::new("qwertyuioptest".to_string()));
        assert_eq!(
            parser.consume_until("test").await.unwrap(),
            "qwertyuiop".to_string()
        );

        let mut parser = Parser::new(Cursor::new("nothing".to_string()));
        assert!(parser.consume_until("test").await.is_err());

        let mut parser = Parser::new(Cursor::new("randomdata   test1 test2".to_string()));
        assert_eq!(parser.consume_until("test").await.unwrap(), "randomdata   ");
        assert_eq!(parser.consume_until("test").await.unwrap(), "1 ");
    }
}
