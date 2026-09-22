//! Loopback-only transport witnesses; never contact a provider.
use super::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

struct Server {
    url: String,
    worker: thread::JoinHandle<Vec<String>>,
}

impl Server {
    fn start(responses: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(std::time::Instant::now() < deadline, "request timed out");
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(e) => panic!("accept: {e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                requests.push(read_request(&mut stream));
                stream.write_all(response.as_bytes()).unwrap();
                // A willing keep-alive server sees EOF after the response is
                // consumed, even while the client itself remains alive.
                let mut byte = [0];
                assert_eq!(
                    stream.read(&mut byte).unwrap(),
                    0,
                    "idle socket retained/reused"
                );
            }
            requests
        });
        Self { url, worker }
    }

    fn finish(self) -> Vec<String> {
        self.worker.join().unwrap()
    }
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() <= 16 * 1024);
    }
    let headers = String::from_utf8(bytes.clone()).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    assert!(length <= 16 * 1024);
    let offset = bytes.len();
    bytes.resize(offset + length, 0);
    stream.read_exact(&mut bytes[offset..]).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn response(status: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: keep-alive\r\n{headers}\r\n{body}",
        body.len()
    )
}

fn client() -> reqwest::Client {
    builder(crate::dns::NativeResolver::standalone())
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

#[test]
fn idle_connections_are_disposed_while_the_client_survives_across_destinations() {
    let runtime = crate::BackendRuntime::new().unwrap();
    let client = client();
    for _ in 0..4 {
        let server = Server::start(vec![response("200 OK", "", "ok"); 2]);
        for _ in 0..2 {
            assert_eq!(
                runtime.block_on(async {
                    client
                        .get(&server.url)
                        .send()
                        .await
                        .unwrap()
                        .text()
                        .await
                        .unwrap()
                }),
                "ok"
            );
        }
        assert_eq!(server.finish().len(), 2);
    }
    drop(client);
}

#[test]
fn redirects_preserve_307_body_and_strip_credentials_across_authorities() {
    let runtime = crate::BackendRuntime::new().unwrap();
    let client = client();
    let target = Server::start(vec![response("200 OK", "", "ok")]);
    let source = Server::start(vec![response(
        "307 Temporary Redirect",
        &format!("Location: {}/redirected\r\n", target.url),
        "",
    )]);
    assert_eq!(
        runtime.block_on(async {
            client
                .post(&source.url)
                .bearer_auth("synthetic-test-key")
                .body("retained request body")
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        }),
        "ok"
    );
    let initial = source.finish();
    let redirected = target.finish();
    assert!(
        initial[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic-test-key")
    );
    assert!(redirected[0].starts_with("POST /redirected HTTP/1.1"));
    assert!(redirected[0].ends_with("retained request body"));
    assert!(
        !redirected[0]
            .to_ascii_lowercase()
            .contains("authorization:")
    );
}

#[test]
fn ten_redirects_succeed_and_an_eleventh_is_a_transport_error() {
    let runtime = crate::BackendRuntime::new().unwrap();
    let client = client();
    let redirect = response("302 Found", "Location: /again\r\n", "");
    let mut accepted = vec![redirect.clone(); MAX_REDIRECTS];
    accepted.push(response("200 OK", "", "ok"));
    let server = Server::start(accepted);
    assert_eq!(
        runtime.block_on(async {
            client
                .get(&server.url)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        }),
        "ok"
    );
    assert_eq!(server.finish().len(), MAX_REDIRECTS + 1);

    let server = Server::start(vec![redirect; MAX_REDIRECTS + 1]);
    let error = runtime
        .block_on(async { client.get(&server.url).send().await })
        .unwrap_err();
    assert!(error.is_redirect(), "{error}");
    assert_eq!(server.finish().len(), MAX_REDIRECTS + 1);
}
