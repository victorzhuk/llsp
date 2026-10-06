mod support;

use llsp::config::Layers;
use serde_json::json;
use support::Client;

#[test]
fn initialize_reports_capabilities() {
    let c = Client::start();
    assert_eq!(c.init["serverInfo"]["name"], "llsp");
    assert_eq!(c.init["capabilities"]["positionEncoding"], "utf-16");
    assert_eq!(c.init["capabilities"]["textDocumentSync"]["change"], 2);
    assert!(c.shutdown(), "clean shutdown");
}

#[test]
fn utf8_negotiated_when_offered() {
    let c = Client::with(
        Layers::default(),
        json!({"capabilities": {"general": {"positionEncodings": ["utf-8", "utf-16"]}}}),
    );
    assert_eq!(c.init["capabilities"]["positionEncoding"], "utf-8");
    c.shutdown();
}

#[test]
fn unknown_request_is_method_not_found() {
    let mut c = Client::start();
    let err = c.request_raw("foo/bar", json!({})).unwrap_err();
    assert_eq!(err.0, -32601);
    c.notify_raw("foo/baz", json!({}));
    c.shutdown();
}

#[test]
fn exit_without_shutdown_is_unclean() {
    let (server_conn, conn) = lsp_server::Connection::memory();
    let server = std::thread::spawn(move || llsp::server::run(server_conn, Layers::default()));
    conn.sender
        .send(
            lsp_server::Request::new(1.into(), "initialize".into(), json!({"capabilities": {}}))
                .into(),
        )
        .unwrap();
    let _ = conn.receiver.recv().unwrap();
    conn.sender
        .send(lsp_server::Notification::new("initialized".into(), json!({})).into())
        .unwrap();
    conn.sender
        .send(lsp_server::Notification::new("exit".into(), json!(null)).into())
        .unwrap();
    assert!(!server.join().unwrap().unwrap());
}

#[test]
fn invalid_init_options_fail_initialize() {
    let (server_conn, conn) = lsp_server::Connection::memory();
    let server = std::thread::spawn(move || llsp::server::run(server_conn, Layers::default()));
    conn.sender
        .send(
            lsp_server::Request::new(
                1.into(),
                "initialize".into(),
                json!({"capabilities": {}, "initializationOptions": {"nope": 1}}),
            )
            .into(),
        )
        .unwrap();
    match conn.receiver.recv().unwrap() {
        lsp_server::Message::Response(r) => {
            let e = r.response_result.expect_err("error");
            assert!(e.message.contains("nope"), "{}", e.message);
        }
        other => panic!("{other:?}"),
    }
    assert!(server.join().unwrap().is_err());
}

#[test]
fn watchers_registered_only_on_dynamic_registration() {
    let mut c = Client::with(
        Layers::default(),
        json!({"capabilities": {"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}}}),
    );
    // The registration is sent after `initialized`; a request drains the channel.
    c.request_raw("workspace/symbol", json!({"query": ""}))
        .unwrap();
    let regs = c
        .server_requests
        .iter()
        .filter(|r| r.method == "client/registerCapability")
        .count();
    assert_eq!(
        regs,
        1,
        "{:?}",
        c.server_requests
            .iter()
            .map(|r| r.method.as_str())
            .collect::<Vec<_>>()
    );
    let reg = c
        .server_requests
        .iter()
        .find(|r| r.method == "client/registerCapability")
        .unwrap();
    let params = &reg.params["registrations"][0];
    assert_eq!(params["method"], "workspace/didChangeWatchedFiles");
    let glob = params["registerOptions"]["watchers"][0]["globPattern"]
        .as_str()
        .unwrap();
    assert!(
        glob.contains("lisp") && glob.contains("clj") && glob.starts_with("**/*.{"),
        "{glob}"
    );
    c.shutdown();

    // Without dynamic registration nothing is registered.
    let mut c = Client::start();
    c.request_raw("workspace/symbol", json!({"query": ""}))
        .unwrap();
    assert!(c.server_requests.is_empty());
    c.shutdown();
}

#[test]
fn tcp_serves_one_unauthenticated_session() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;

    // Reserve an ephemeral port, then hand it to the server's listener.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let server = std::thread::spawn(move || {
        let (conn, io_threads) = lsp_server::Connection::listen(("127.0.0.1", port)).unwrap();
        let served = llsp::server::run(conn, Layers::default());
        io_threads.join().unwrap();
        served
    });

    // The first local connection owns the session.
    let stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => break s,
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    };
    let mut stream = stream;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let send = |s: &mut TcpStream, body: &str| {
        write!(s, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    };
    let mut out = BufReader::new(stream.try_clone().unwrap());
    send(
        &mut stream,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#,
    );
    let mut header = String::new();
    out.read_line(&mut header).unwrap();
    let len: usize = header
        .trim()
        .strip_prefix("Content-Length: ")
        .unwrap()
        .parse()
        .unwrap();
    out.read_line(&mut String::new()).unwrap();
    let mut body = vec![0; len];
    out.read_exact(&mut body).unwrap();
    let init: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "llsp");
    send(
        &mut stream,
        r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#,
    );

    // A second local connection receives no LSP traffic: either the OS refuses
    // it (the listener is closed after the first accept) or it hangs silently.
    match TcpStream::connect(("127.0.0.1", port)) {
        Err(_) => {}
        Ok(mut second) => {
            second
                .set_read_timeout(Some(std::time::Duration::from_millis(200)))
                .unwrap();
            let mut buf = [0u8; 1];
            assert!(
                second.read(&mut buf).is_err(),
                "second connection must not receive LSP traffic"
            );
        }
    }

    // The first session keeps answering and shuts down cleanly.
    send(
        &mut stream,
        r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#,
    );
    header.clear();
    out.read_line(&mut header).unwrap();
    out.read_line(&mut String::new()).unwrap();
    send(&mut stream, r#"{"jsonrpc":"2.0","method":"exit"}"#);
    assert!(server.join().unwrap().unwrap());
}
