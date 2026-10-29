//! Local web front end: serves the UI and the JSON API from [`crate::api`].
//!
//! Built on `std::net` alone (no async runtime, no extra dependencies) and bound to
//! loopback by default, in keeping with the tool's offline design: it is meant to be
//! opened on the analyst's own machine, never exposed on an OT network. The public
//! website runs the same API in the visitor's browser instead (see [`crate::wasm`]).
//!
//! Pages (`/`, `/dashboard`, `/related-work`) and their assets are embedded at build
//! time, so there is no file-system path to traverse. Every response is sent with
//! `Connection: close`.

pub use crate::api::{AppState, Response};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const MAX_HEADER_BYTES: u64 = 16 * 1024;
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

fn asset(path: &str) -> Option<Response> {
    let (content_type, body) = match path {
        "/" | "/index.html" => ("text/html; charset=utf-8", include_str!("../web/index.html")),
        "/dashboard" | "/dashboard.html" => ("text/html; charset=utf-8", include_str!("../web/dashboard.html")),
        "/related-work" | "/related-work.html" => ("text/html; charset=utf-8", include_str!("../web/related-work.html")),
        "/styles.css" => ("text/css; charset=utf-8", include_str!("../web/styles.css")),
        "/console.css" => ("text/css; charset=utf-8", include_str!("../web/console.css")),
        "/iso.js" => ("text/javascript; charset=utf-8", include_str!("../web/iso.js")),
        "/scenes.js" => ("text/javascript; charset=utf-8", include_str!("../web/scenes.js")),
        "/app.js" => ("text/javascript; charset=utf-8", include_str!("../web/app.js")),
        "/dashboard.js" => ("text/javascript; charset=utf-8", include_str!("../web/dashboard.js")),
        "/favicon.svg" => ("image/svg+xml", include_str!("../web/favicon.svg")),
        _ => return None,
    };
    Some(Response { status: 200, content_type, body: body.as_bytes().to_vec() })
}

/// Route one request: embedded pages and assets first, then the API.
pub fn handle(state: &AppState, method: &str, target: &str, body: &[u8]) -> Response {
    if method == "GET" {
        let path = target.split('?').next().unwrap_or("/");
        if let Some(page) = asset(path) {
            return page;
        }
    }
    crate::api::handle(state, method, target, body)
}

/// Bind `addr` and serve until the process exits.
pub fn serve(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;
    eprintln!("rsashield {} serving http://{local}", env!("CARGO_PKG_VERSION"));
    if !local.ip().is_loopback() {
        eprintln!("warning: listening beyond loopback; keep this off OT networks (see docs/THREAT_MODEL.md)");
    }
    run(listener, Arc::new(AppState::new()))
}

/// Serve connections from an already-bound listener (one thread per connection).
pub fn run(listener: TcpListener, state: Arc<AppState>) -> io::Result<()> {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let state = Arc::clone(&state);
        thread::spawn(move || {
            let _ = handle_connection(stream, &state);
        });
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, state: &AppState) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    let mut reader = BufReader::new(stream.try_clone()?);

    let (method, target, content_length) = {
        let mut head = (&mut reader).take(MAX_HEADER_BYTES);
        let mut request_line = String::new();
        if head.read_line(&mut request_line)? == 0 {
            return Ok(());
        }
        let mut parts = request_line.split_whitespace();
        let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
            return write_response(&mut stream, &Response::error(400, "malformed request line"));
        };
        let (method, target) = (method.to_string(), target.to_string());

        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            if head.read_line(&mut line)? == 0 {
                return write_response(&mut stream, &Response::error(431, "request headers too large or truncated"));
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.trim().eq_ignore_ascii_case("content-length") {
                    match value.trim().parse() {
                        Ok(n) => content_length = n,
                        Err(_) => return write_response(&mut stream, &Response::error(400, "invalid Content-Length")),
                    }
                }
            }
        }
        (method, target, content_length)
    };

    if content_length > MAX_BODY_BYTES {
        return write_response(&mut stream, &Response::error(413, "request body too large (limit 2 MiB)"));
    }
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;

    let response = handle(state, &method, &target, &body);
    write_response(&mut stream, &response)
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Internal Server Error",
    }
}

fn write_response(stream: &mut TcpStream, r: &Response) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         Connection: close\r\n\r\n",
        r.status,
        reason(r.status),
        r.content_type,
        r.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&r.body)?;
    stream.flush()
}
