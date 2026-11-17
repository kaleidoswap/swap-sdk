//! A one-route-per-path HTTP stub on a local port, for tests that drive an
//! Esplora client without a network. Native-only: it binds a TCP listener.

use std::io::{Read, Write};

/// Serve `routes` (path, status code, body) on `127.0.0.1` and return the base
/// URL. A path with no route answers 404.
pub(crate) fn stub_esplora(routes: Vec<(String, u16, String)>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&buf[..n]),
                }
            }
            let request = String::from_utf8_lossy(&request);
            let path = request.split_whitespace().nth(1).unwrap_or("").to_string();
            let (code, body) = routes
                .iter()
                .find(|(route, _, _)| *route == path)
                .map(|(_, code, body)| (*code, body.clone()))
                .unwrap_or((404, "not found".into()));
            let _ = write!(
                stream,
                "HTTP/1.1 {code} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    base
}
