use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl HttpRequest {
    pub fn parse<R: Read>(mut reader: BufReader<R>) -> Result<Self, String> {
        let mut request_line = String::new();
        if reader
            .read_line(&mut request_line)
            .map_err(|e| e.to_string())?
            == 0
        {
            return Err("Empty Request".to_string());
        }

        let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
        if parts.len() < 2 {
            return Err("Invalid Request-Line".to_string());
        }

        let method = parts[0].to_string();
        let uri = parts[1].to_string();

        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some((key, value)) = trimmed.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            }
        }

        let mut body = None;
        if let Some(len_str) = headers.get("content-length") {
            if let Ok(len) = len_str.parse::<usize>() {
                let mut buf = vec![0; len];
                reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
                body = String::from_utf8(buf).ok();
            }
        }

        Ok(HttpRequest {
            method,
            uri,
            headers,
            body,
        })
    }
}

pub fn send_response(
    stream: &mut TcpStream,
    status_line: &str,
    content_type: &str,
    contents: &[u8],
) {
    let length = contents.len();
    let response = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {length}\r\n\r\n"
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(contents);
    let _ = stream.flush();
}

pub fn get_file_path(request_uri: &str) -> PathBuf {
    let path_str = if request_uri == "/" {
        "/index.html"
    } else {
        request_uri
    };

    let clean_path = path_str.trim_start_matches('/');
    let public_dir = Path::new("public");
    let requested_path = public_dir.join(clean_path);

    if let Ok(canonical_public) = public_dir.canonicalize() {
        if let Ok(canonical_requested) = requested_path.canonicalize() {
            if canonical_requested.starts_with(&canonical_public) && canonical_requested.is_file() {
                return canonical_requested;
            }
        }
    }
    public_dir.join("404.html")
}

pub fn get_content_type(path: &Path) -> &str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=UTF-8",
        Some("css") => "text/css; charset=UTF-8",
        Some("js") => "application/javascript; charset=UTF-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        _ => "text/plain; charset=UTF-8",
    }
}
