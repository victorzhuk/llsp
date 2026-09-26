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
