use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::Value;

use super::super::PORT;

pub(super) fn server_ready() -> bool {
    let address: SocketAddr = ([127, 0, 0, 1], PORT).into();
    TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok()
}

pub(super) fn server_version() -> Option<u64> {
    let address: SocketAddr = ([127, 0, 0, 1], PORT).into();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let body = response.split("\r\n\r\n").nth(1)?;
    let value: Value = serde_json::from_str(body).ok()?;
    value.get("version").and_then(Value::as_u64)
}

pub(super) fn request_shutdown() {
    let address: SocketAddr = ([127, 0, 0, 1], PORT).into();
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return;
    };

    let _ = stream.write_all(
        b"POST /shutdown HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );

    let mut buffer = [0u8; 64];
    let _ = stream.read(&mut buffer);
}
