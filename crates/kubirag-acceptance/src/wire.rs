//! HTTP/1.1 minimo su `std::net`, quanto basta al client dello scenario e all'Ollama finto:
//! un messaggio per connessione (`Connection: close`), corpo con `Content-Length` o `chunked`.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Un messaggio HTTP: riga iniziale, header (nomi minuscoli, in ordine) e corpo.
#[derive(Debug, Default)]
pub struct Message {
    pub start: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Message {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

fn line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}

/// Legge un messaggio. Senza `Content-Length` né `chunked`, una richiesta non ha corpo e una
/// risposta arriva fino alla chiusura della connessione.
pub fn read(stream: &TcpStream, request: bool) -> io::Result<Message> {
    let mut reader = BufReader::new(stream);
    let start = line(&mut reader)?;
    if start.is_empty() {
        return Err(invalid("empty message"));
    }
    let mut headers = Vec::new();
    loop {
        let header = line(&mut reader)?;
        if header.is_empty() {
            break;
        }
        let (name, value) = header.split_once(':').ok_or_else(|| invalid(&header))?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }
    let mut message = Message {
        start,
        headers,
        body: Vec::new(),
    };
    if message
        .header("transfer-encoding")
        .is_some_and(|t| t.eq_ignore_ascii_case("chunked"))
    {
        loop {
            let size = line(&mut reader)?;
            let size = usize::from_str_radix(size.split(';').next().unwrap_or("").trim(), 16)
                .map_err(|_| invalid(&size))?;
            let mut chunk = vec![0; size + 2];
            reader.read_exact(&mut chunk)?;
            if size == 0 {
                break;
            }
            message.body.extend_from_slice(&chunk[..size]);
        }
    } else if let Some(length) = message.header("content-length") {
        let length = length.parse().map_err(|_| invalid(length))?;
        message.body = vec![0; length];
        reader.read_exact(&mut message.body)?;
    } else if !request {
        reader.read_to_end(&mut message.body)?;
    }
    Ok(message)
}

/// Scrive un messaggio con `Content-Length` e `Connection: close`.
pub fn write(
    stream: &mut TcpStream,
    start: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> io::Result<()> {
    let mut out = format!("{start}\r\n");
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    stream.write_all(out.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// Una richiesta a `base` (`http://host:porta`), con la risposta completa.
pub fn request(
    base: &str,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> io::Result<(u16, Message)> {
    let host = base.trim_start_matches("http://");
    let mut stream = TcpStream::connect(host)?;
    stream.set_read_timeout(Some(Duration::from_secs(60)))?;
    let mut all: Vec<(&str, &str)> = vec![("Host", host), ("Accept-Encoding", "identity")];
    all.extend(headers.iter().map(|(n, v)| (n.as_str(), v.as_str())));
    write(
        &mut stream,
        &format!("{method} {path} HTTP/1.1"),
        &all,
        body.unwrap_or_default(),
    )?;
    let response = read(&stream, false)?;
    let status = response
        .start
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| invalid(&response.start))?;
    Ok((status, response))
}
