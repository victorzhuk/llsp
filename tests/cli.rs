use std::path::Path;
use std::process::{Command, Output};

fn llsp(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_llsp"))
        .args(args)
        .current_dir(dir)
        .env("XDG_CONFIG_HOME", dir.join("xdg"))
        .env("HOME", dir)
        .env_remove("LLSP_CONFIG")
        .env_remove("LLSP_LOG")
        .output()
        .unwrap()
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)))
}

#[test]
fn check_reports_errors_as_json_when_piped() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("broken.lisp"), "(defun f (x) x\n").unwrap();
    std::fs::write(dir.path().join("ok.clj"), "(ns a) {:a [1]}").unwrap();
    let out = llsp(dir.path(), &["check", "."]);
    assert_eq!(out.status.code(), Some(1));
    let v = json(&out);
    let reports = v.as_array().unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["code"], "unclosed-delimiter");
    assert_eq!(reports[0]["line"], 1);
    assert_eq!(reports[0]["column"], 1);
    assert_eq!(reports[0]["severity"], "error");
    assert!(
        reports[0]["path"]
            .as_str()
            .unwrap()
            .ends_with("broken.lisp")
    );
}

#[test]
fn check_clean_file_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("ok.clj"), "(ns a) {:a [1]}").unwrap();
    let out = llsp(dir.path(), &["check", "ok.clj", "--format", "text"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}

#[test]
fn check_text_format() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.scm"), "(a))").unwrap();
    let out = llsp(dir.path(), &["check", "a.scm", "--format", "text"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "a.scm:1:4: error[unexpected-closer]: unexpected closing delimiter\n"
    );
}

#[test]
fn check_missing_file_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let out = llsp(dir.path(), &["check", "nope.lisp"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn config_layers_env_and_set() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".llsp.toml"), "[format]\nbody_indent = 2\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llsp"))
        .args(["config", "--format", "json"])
        .current_dir(dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("xdg"))
        .env("LLSP_FORMAT__BODY_INDENT", "4")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json(&out)["format"]["body_indent"], 4);

    let out = llsp(
        dir.path(),
        &[
            "config",
            "--format",
            "json",
            "--set",
            "format.body_indent=7",
        ],
    );
    assert_eq!(json(&out)["format"]["body_indent"], 7);

    let out = llsp(dir.path(), &["config", "--set", "fromat.x=1"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("fromat"));
}

#[test]
fn user_config_file_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg").join("llsp");
    std::fs::create_dir_all(&xdg).unwrap();
    std::fs::write(xdg.join("config.toml"), "[workspace]\nmax_files = 5\n").unwrap();
    let out = llsp(dir.path(), &["config", "--format", "json"]);
    assert_eq!(json(&out)["workspace"]["max_files"], 5);
}

#[test]
fn dialects_listed() {
    let dir = tempfile::tempdir().unwrap();
    let out = llsp(dir.path(), &["dialects", "--format", "json"]);
    let names: Vec<_> = json(&out)
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect();
    for n in [
        "common-lisp",
        "clojure",
        "scheme",
        "racket",
        "emacs-lisp",
        "fennel",
        "janet",
    ] {
        assert!(names.iter().any(|x| x == n), "{n}");
    }
}

#[test]
fn listen_rejects_non_loopback() {
    let dir = tempfile::tempdir().unwrap();
    let out = llsp(dir.path(), &["serve", "--listen", "0.0.0.0:9000"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("loopback"));
}

#[test]
fn stdio_serve_roundtrip() {
    use std::io::{BufRead, BufReader, Read, Write};
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_llsp"))
        .current_dir(dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("xdg"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let send = |w: &mut dyn Write, body: &str| {
        write!(w, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    };
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#,
    );
    let mut out = BufReader::new(child.stdout.take().unwrap());
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
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["result"]["serverInfo"]["name"], "llsp");
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#,
    );
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#,
    );
    send(&mut stdin, r#"{"jsonrpc":"2.0","method":"exit"}"#);
    assert!(child.wait().unwrap().success());
}

#[test]
fn check_runs_lints_across_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.lisp"), "(defun helper () 1)").unwrap();
    std::fs::write(
        dir.path().join("b.lisp"),
        "(helper) (nowhere) (let ((u 1)) 2)",
    )
    .unwrap();
    let out = llsp(
        dir.path(),
        &[
            "check",
            ".",
            "--set",
            "diagnostics.unresolved_call=\"warning\"",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "warnings and hints only");
    let v = json(&out);
    let codes: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["code"].as_str().unwrap(), r["severity"].as_str().unwrap()))
        .collect();
    assert_eq!(
        codes,
        [("unresolved-call", "warning"), ("unused-binding", "hint")]
    );
}

#[test]
fn format_check_and_write() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.lisp");
    std::fs::write(&file, "(defun f ()\n(g))\n").unwrap();
    std::fs::write(dir.path().join("ok.lisp"), "(defun f ()\n  (g))\n").unwrap();
    let out = llsp(dir.path(), &["format", "--check", ".", "--format", "text"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .ends_with("a.lisp")
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "(defun f ()\n(g))\n"
    );

    let out = llsp(dir.path(), &["format", "."]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "(defun f ()\n  (g))\n"
    );
    let out = llsp(dir.path(), &["format", "--check", "."]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(json(&out), serde_json::json!([]));
}

#[cfg(unix)]
#[test]
fn log_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("llsp.log");
    let out = llsp(
        dir.path(),
        &[
            "--log-file",
            log.to_str().unwrap(),
            "--log-level",
            "info",
            "config",
        ],
    );
    assert!(out.status.success());
    let mode = std::fs::metadata(&log).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn check_directory_follows_workspace_rules() {
    let dir = tempfile::tempdir().unwrap();
    let write = |rel: &str, text: &str| {
        let p = dir.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    write("src/a.lisp", "(");
    write("target/gen.lisp", "(");
    write("ignored/x.lisp", "(");
    write(".gitignore", "ignored/\n");
    write("script.lsp", "(");
    write(".llsp.toml", "[files.associations]\n\"*.lsp\" = \"emacs-lisp\"\n");
    let out = llsp(dir.path(), &["check", "."]);
    assert_eq!(out.status.code(), Some(1));
    let mut paths: Vec<String> = json(&out)
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap().replace('\\', "/"))
        .collect();
    paths.sort();
    assert_eq!(paths, ["./script.lsp", "./src/a.lisp"]);
}

#[test]
fn format_unreadable_files_exit_two() {
    let dir = tempfile::tempdir().unwrap();
    let out = llsp(dir.path(), &["format", "nope.lisp"]);
    assert_eq!(out.status.code(), Some(2));

    std::fs::write(dir.path().join("big.lisp"), "(defun f ()\n(g))\n").unwrap();
    let out = llsp(
        dir.path(),
        &["format", "big.lisp", "--set", "files.max_file_size=5"],
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("max_file_size"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("big.lisp")).unwrap(),
        "(defun f ()\n(g))\n"
    );
}
