//! A blocking, dependency-free HTTP mock for the provider tests.
//!
//! Small on purpose: it speaks exactly the HTTP/1.1 subset reqwest emits
//! (request line, headers, `Content-Length` body) and answers a scripted list of
//! responses in order, recording what it received. That is enough to pin both
//! provider request bodies byte-for-byte and to count retries — and it costs no
//! dependency (a wiremock-class crate would pull a second async stack in).

#![cfg(test)]

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct MockResponse {
    status: u16,
    /// Bytes, not text: a synthesized WAV is binary, and re-encoding it as UTF-8
    /// would corrupt every sample rate above 127.
    body: Vec<u8>,
    retry_after: Option<String>,
    /// Accept the connection and never answer — for the in-flight/busy test.
    hang: bool,
}

impl MockResponse {
    pub fn retry_after(mut self, seconds: &str) -> Self {
        self.retry_after = Some(seconds.to_string());
        self
    }
}

#[derive(Debug, Clone)]
pub struct MockRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl MockRequest {
    pub fn header(&self, name: &str) -> Option<String> {
        let name = name.to_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.clone())
    }

    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).expect("request body is JSON")
    }
}

pub struct MockServer {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<MockRequest>>>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl MockServer {
    pub fn ok(body: &str) -> MockResponse {
        Self::status(200, body)
    }

    /// A binary 200 — a WAV, an MP3, anything the provider streams back.
    pub fn ok_bytes(body: &[u8]) -> MockResponse {
        MockResponse {
            status: 200,
            body: body.to_vec(),
            retry_after: None,
            hang: false,
        }
    }

    pub fn status(status: u16, body: &str) -> MockResponse {
        MockResponse {
            status,
            body: body.as_bytes().to_vec(),
            retry_after: None,
            hang: false,
        }
    }

    pub fn hang() -> MockResponse {
        MockResponse {
            status: 200,
            body: Vec::new(),
            retry_after: None,
            hang: true,
        }
    }

    /// Serve `responses` in order; any further request gets a 500.
    pub fn start(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let address = listener.local_addr().expect("local address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let worker = {
            let requests = Arc::clone(&requests);
            let shutdown = Arc::clone(&shutdown);
            thread::spawn(move || {
                // Connections we deliberately never answer, kept open so the
                // client stays in flight instead of seeing an EOF.
                let mut hung = Vec::new();
                for stream in listener.incoming() {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(mut stream) = stream else { break };
                    let Some(request) = read_request(&mut stream) else {
                        continue;
                    };
                    let index = {
                        let mut log = requests.lock().expect("request log");
                        log.push(request);
                        log.len() - 1
                    };
                    match responses.get(index) {
                        Some(response) if response.hang => hung.push(stream),
                        Some(response) => write_response(&mut stream, response),
                        None => write_response(
                            &mut stream,
                            &MockServer::status(
                                500,
                                "{\"error\": \"mock server ran out of responses\"}",
                            ),
                        ),
                    }
                }
                drop(hung);
            })
        };

        Self {
            address,
            requests,
            shutdown,
            worker: Some(worker),
        }
    }

    /// `http://127.0.0.1:<port>/v1` — the `/v1` mirrors a real provider base URL,
    /// so the client's path building is exercised too.
    pub fn base_url(&self) -> String {
        format!("http://{}/v1", self.address)
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().expect("request log").len()
    }

    pub fn request(&self, index: usize) -> MockRequest {
        self.requests
            .lock()
            .expect("request log")
            .get(index)
            .cloned()
            .unwrap_or_else(|| panic!("no request #{index} was made"))
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Unblock `incoming()`, which is parked in accept().
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(200));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<MockRequest> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");

    let mut raw = Vec::new();
    let mut chunk = [0u8; 1024];
    let header_end = loop {
        if let Some(position) = find_header_end(&raw) {
            break position;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        raw.extend_from_slice(&chunk[..read]);
    };

    let head = String::from_utf8_lossy(&raw[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_string();
    let path = request_line.next()?.to_string();

    let mut headers = Vec::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.push((key.trim().to_lowercase(), value.trim().to_string()));
        }
    }

    let length: usize = headers
        .iter()
        .find(|(key, _)| key == "content-length")
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);

    let mut body = raw[header_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);

    Some(MockRequest {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}

fn find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_response(stream: &mut TcpStream, response: &MockResponse) {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        reason(response.status),
        response.body.len(),
    );
    if let Some(retry_after) = &response.retry_after {
        head.push_str(&format!("Retry-After: {retry_after}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}
