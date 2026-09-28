// musl's allocator serializes threads and makes the parallel workspace scan ~20x slower.
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::io::{IsTerminal, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use llsp::analysis::Analysis;
use llsp::config::{Layers, Settings};
use llsp::diagnostics::{self, Severity};
use llsp::document::{Encoding, path_to_uri, to_position};
use llsp::syntax::Tree;
use llsp::workspace::{self, FileSummary, Index};
use lsp_server::Connection;

#[derive(Parser)]
#[command(version, about = "Fast, static language server for Lisp dialects")]
struct Cli {
    /// Configuration file used instead of the user config file.
    #[arg(long, global = true, env = "LLSP_CONFIG", value_name = "PATH")]
    config: Option<PathBuf>,

    /// Override a setting, e.g. `--set format.body_indent=4`. Repeatable.
    #[arg(long = "set", global = true, value_name = "KEY=VALUE")]
    sets: Vec<String>,

    /// Log level: error, warn, info, debug, trace.
    #[arg(long, global = true, env = "LLSP_LOG", value_name = "LEVEL")]
    log_level: Option<String>,

    /// Write logs to this file instead of stderr.
    #[arg(long, global = true, value_name = "PATH")]
    log_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serve LSP over stdio (default) or a loopback TCP address.
    Serve {
        /// Listen on a loopback address, e.g. 127.0.0.1:9257.
        #[arg(long, value_name = "ADDR")]
        listen: Option<SocketAddr>,
        /// Accepted for editor compatibility; stdio is the default.
        #[arg(long, hide = true)]
        stdio: bool,
    },
    /// Report diagnostics for files or directories.
    Check {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
        format: OutputFormat,
    },
    /// Re-indent files in place, or list files that would change with --check.
    Format {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Only report files that would change; exit 1 if any.
        #[arg(long)]
        check: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
        format: OutputFormat,
    },
    /// Print the effective configuration.
    Config {
        #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
        format: OutputFormat,
    },
    /// List available dialects.
    Dialects {
        #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
        format: OutputFormat,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Auto,
    Text,
    Json,
}

impl OutputFormat {
    fn json(self) -> bool {
        match self {
            Self::Auto => !std::io::stdout().is_terminal(),
            Self::Text => false,
            Self::Json => true,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("llsp: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let mut layers = Layers::startup(cli.config.as_deref(), &cli.sets)?;
    let cwd = std::env::current_dir()?;
    let command = cli.command.unwrap_or(Command::Serve {
        listen: None,
        stdio: true,
    });
    if !matches!(command, Command::Serve { .. }) {
        layers.load_project(&cwd)?;
    }
    let settings = layers.resolve()?;
    init_logging(&settings, cli.log_level.as_deref(), cli.log_file.as_deref())?;

    match command {
        Command::Serve { listen, .. } => serve(layers, listen),
        Command::Check { paths, format } => check(&settings, &paths, format.json()),
        Command::Format {
            paths,
            check,
            format,
        } => format_files(&settings, &paths, check, format.json()),
        Command::Config { format } => {
            let out = if format.json() {
                serde_json::to_string_pretty(&settings.config)?
            } else {
                toml::to_string_pretty(&settings.config)?
            };
            println!("{out}");
            Ok(ExitCode::SUCCESS)
        }
        Command::Dialects { format } => {
            let rows: Vec<_> = settings
                .dialects
                .iter()
                .map(|d| serde_json::json!({"name": d.name, "extensions": d.extensions}))
                .collect();
            if format.json() {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                for d in settings.dialects.iter() {
                    println!("{:<12} {}", d.name, d.extensions.join(" "));
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn init_logging(settings: &Settings, level: Option<&str>, file: Option<&Path>) -> Result<()> {
    let level = level.unwrap_or(&settings.config.log.level);
    let mut builder = env_logger::Builder::new();
    builder.parse_filters(level);
    let file = file.or(settings.config.log.file.as_deref());
    if let Some(path) = file {
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let f = opts
            .open(path)
            .with_context(|| format!("open log file {}", path.display()))?;
        builder.target(env_logger::Target::Pipe(Box::new(f)));
    }
    builder.try_init().ok();
    Ok(())
}

fn serve(layers: Layers, listen: Option<SocketAddr>) -> Result<ExitCode> {
    let (conn, io) = match listen {
        Some(addr) if !addr.ip().is_loopback() => {
            bail!("--listen: only loopback addresses are allowed, got {addr}")
        }
        Some(addr) => Connection::listen(addr).with_context(|| format!("listen on {addr}"))?,
        None => Connection::stdio(),
    };
    log::info!("llsp {} started", env!("CARGO_PKG_VERSION"));
    let clean = llsp::server::run(conn, layers)?;
    io.join()?;
    Ok(if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

#[derive(serde::Serialize)]
struct Report {
    path: String,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    severity: Severity,
    code: &'static str,
    message: String,
}

fn check(settings: &Settings, paths: &[PathBuf], json: bool) -> Result<ExitCode> {
    let mut io_error = false;
    let mut files = Vec::new();
    for path in collect_files(settings, paths) {
        match read_limited(&path, settings.config.files.max_file_size) {
            Ok(text) => {
                let dialect = settings.detect(Some(&path), None, &text);
                let tree = Tree::parse(text, &dialect);
                let analysis = Analysis::new(&tree, &dialect);
                files.push((path, dialect, tree, analysis));
            }
            Err(e) => {
                eprintln!("llsp: {}: {e:#}", path.display());
                io_error = true;
            }
        }
    }
    let mut index = Index::default();
    for (path, dialect, tree, analysis) in &files {
        if let Some(uri) = path_to_uri(path) {
            index.insert(FileSummary::new(uri, dialect.clone(), tree, analysis));
        }
    }

    let mut reports = Vec::new();
    for (path, dialect, tree, analysis) in &files {
        let is_defined = |key: &str| {
            index
                .defs_named(key)
                .any(|(f, _)| f.dialect.name == dialect.name)
        };
        let lines = line_index::LineIndex::new(tree.text());
        let found = diagnostics::check(
            tree,
            analysis,
            dialect,
            &settings.config.diagnostics,
            is_defined,
        );
        for d in found {
            let start = to_position(&lines, tree.text(), d.start, Encoding::Utf8);
            let end = to_position(&lines, tree.text(), d.end, Encoding::Utf8);
            reports.push(Report {
                path: path.display().to_string(),
                line: start.line + 1,
                column: start.character + 1,
                end_line: end.line + 1,
                end_column: end.character + 1,
                severity: d.severity,
                code: d.code,
                message: d.message,
            });
        }
    }

    let mut out = std::io::stdout().lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &reports)?;
        writeln!(out)?;
    } else {
        for r in &reports {
            writeln!(
                out,
                "{}:{}:{}: {}[{}]: {}",
                r.path,
                r.line,
                r.column,
                r.severity.as_str(),
                r.code,
                r.message
            )?;
        }
    }
    Ok(if io_error {
        ExitCode::from(2)
    } else if reports.iter().any(|r| r.severity == Severity::Error) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn format_files(
    settings: &Settings,
    paths: &[PathBuf],
    check: bool,
    json: bool,
) -> Result<ExitCode> {
    let mut changed = Vec::new();
    let mut io_error = false;
    for path in collect_files(settings, paths) {
        let text = match read_limited(&path, settings.config.files.max_file_size) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("llsp: {}: {e:#}", path.display());
                io_error = true;
                continue;
            }
        };
        let dialect = settings.detect(Some(&path), None, &text);
        let tree = Tree::parse(text, &dialect);
        let analysis = Analysis::new(&tree, &dialect);
        let hints = |key: &str| {
            analysis
                .defs
                .iter()
                .filter(|d| d.key == key)
                .find_map(|d| d.indent)
        };
        let edits = llsp::format::format(&tree, &dialect, &settings.config.format, &hints, None);
        if edits.is_empty() {
            continue;
        }
        if !check {
            let out = llsp::format::apply(tree.text(), &edits);
            if let Err(e) = std::fs::write(&path, out) {
                eprintln!("llsp: {}: {e}", path.display());
                io_error = true;
                continue;
            }
        }
        changed.push(path.display().to_string());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&changed)?);
    } else {
        for p in &changed {
            println!("{p}");
        }
    }
    Ok(if io_error {
        ExitCode::from(2)
    } else if check && !changed.is_empty() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn collect_files(settings: &Settings, paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if !path.is_dir() {
            out.push(path.clone());
            continue;
        }
        let (files, truncated) = workspace::discover(settings, std::slice::from_ref(path));
        if truncated {
            eprintln!(
                "llsp: {}: over workspace.max_files ({}); only the first {} files are processed",
                path.display(),
                settings.config.workspace.max_files,
                files.len()
            );
        }
        out.extend(files);
    }
    out.sort();
    out.dedup();
    out
}

fn read_limited(path: &Path, max: u64) -> Result<String> {
    let len = std::fs::metadata(path)?.len();
    if len > max {
        bail!("file is {len} bytes, over files.max_file_size ({max})");
    }
    Ok(std::fs::read_to_string(path)?)
}
