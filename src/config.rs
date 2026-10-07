use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::dialect::{Dialect, Dialects};
use crate::tables::merge_tables;

pub const PROJECT_FILE: &str = ".llsp.toml";
const ENV_PREFIX: &str = "LLSP_";
const RESERVED_ENV: &[&str] = &["LLSP_CONFIG", "LLSP_LOG"];
/// Upper bounds for settings whose values feed allocation sizes or timers.
pub const MAX_DEBOUNCE_MS: u64 = 60_000;
pub const MAX_INDENT: u32 = 1000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub files: Files,
    pub workspace: Workspace,
    pub completion: Completion,
    pub diagnostics: Diagnostics,
    pub format: Format,
    pub log: Log,
    pub dialects: toml::Table,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Files {
    /// Glob (matched against the path) to dialect name.
    pub associations: BTreeMap<String, String>,
    pub default_dialect: String,
    pub max_file_size: u64,
}

impl Default for Files {
    fn default() -> Self {
        Self {
            associations: BTreeMap::new(),
            default_dialect: "common-lisp".into(),
            max_file_size: 8 << 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Workspace {
    pub index: bool,
    pub exclude: Vec<String>,
    pub max_files: usize,
    pub max_symbols: usize,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            index: true,
            exclude: vec![
                "**/node_modules/**".into(),
                "**/target/**".into(),
                "**/.cpcache/**".into(),
                "**/.shadow-cljs/**".into(),
                "**/.clj-kondo/**".into(),
            ],
            max_files: 20_000,
            max_symbols: 256,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Completion {
    pub max_items: usize,
    pub builtins: bool,
}

impl Default for Completion {
    fn default() -> Self {
        Self {
            max_items: 200,
            builtins: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Off,
    Hint,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Diagnostics {
    pub enable: bool,
    pub debounce_ms: u64,
    pub unused_binding: Level,
    pub duplicate_definition: Level,
    pub unresolved_call: Level,
    pub ignore_prefix: String,
    pub known_symbols: Vec<String>,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            enable: true,
            debounce_ms: 100,
            unused_binding: Level::Hint,
            duplicate_definition: Level::Warning,
            unresolved_call: Level::Off,
            ignore_prefix: "_".into(),
            known_symbols: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Format {
    pub body_indent: u32,
    pub distinguished_indent: u32,
    pub trim_trailing_whitespace: bool,
}

impl Default for Format {
    fn default() -> Self {
        Self {
            body_indent: 2,
            distinguished_indent: 4,
            trim_trailing_whitespace: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Log {
    pub level: String,
    pub file: Option<PathBuf>,
}

impl Default for Log {
    fn default() -> Self {
        Self {
            level: "warn".into(),
            file: None,
        }
    }
}

/// Configuration sources, lowest precedence first.
#[derive(Debug, Clone, Default)]
pub struct Layers {
    pub user: toml::Table,
    pub project: toml::Table,
    pub env: toml::Table,
    pub cli: toml::Table,
    pub init: toml::Table,
    pub client: toml::Table,
}

impl Layers {
    /// Reads the user file, environment and `--set` pairs. The project layer is
    /// loaded later, once the workspace root is known.
    pub fn startup(config_path: Option<&Path>, sets: &[String]) -> Result<Self> {
        let user = match config_path {
            Some(p) => read_table(p)?,
            None => match user_config_path() {
                Some(p) if p.is_file() => read_table(&p)?,
                _ => toml::Table::new(),
            },
        };
        let mut cli = toml::Table::new();
        for set in sets {
            let Some((key, value)) = set.split_once('=') else {
                bail!("--set expects key=value, got {set:?}");
            };
            insert_path(
                &mut cli,
                key.split('.').map(str::trim),
                parse_value(value.trim()),
            )?;
        }
        Ok(Self {
            user,
            env: env_table(std::env::vars())?,
            cli,
            ..Self::default()
        })
    }

    /// A checked-out repository is untrusted, so its file cannot redirect logging.
    pub fn load_project(&mut self, root: &Path) -> Result<()> {
        let path = root.join(PROJECT_FILE);
        self.project = if path.is_file() {
            read_table(&path)?
        } else {
            toml::Table::new()
        };
        if self.project.remove("log").is_some() {
            log::warn!("{}: [log] is ignored in project files", path.display());
        }
        Ok(())
    }

    pub fn merged(&self) -> toml::Table {
        let mut out = toml::Table::new();
        for layer in [
            &self.user,
            &self.project,
            &self.env,
            &self.cli,
            &self.init,
            &self.client,
        ] {
            merge_tables(&mut out, layer.clone());
        }
        out
    }

    pub fn resolve(&self) -> Result<Settings> {
        Settings::from_table(self.merged())
    }
}

/// Validated configuration plus the dialects it defines.
#[derive(Debug, Clone)]
pub struct Settings {
    pub config: Config,
    pub dialects: Dialects,
    pub associations: Vec<(globset::GlobMatcher, String)>,
}

impl Settings {
    pub fn from_table(table: toml::Table) -> Result<Self> {
        let config: Config = table
            .try_into()
            .map_err(|e| anyhow::anyhow!("config: {e}"))?;
        if config.diagnostics.debounce_ms > MAX_DEBOUNCE_MS {
            bail!(
                "diagnostics.debounce_ms: {} exceeds the maximum of {MAX_DEBOUNCE_MS}",
                config.diagnostics.debounce_ms
            );
        }
        for (key, value) in [
            ("format.body_indent", config.format.body_indent),
            (
                "format.distinguished_indent",
                config.format.distinguished_indent,
            ),
        ] {
            if value > MAX_INDENT {
                bail!("{key}: {value} exceeds the maximum of {MAX_INDENT}");
            }
        }
        let dialects = Dialects::load(&config.dialects)?;
        let mut associations = Vec::new();
        for (glob, name) in &config.files.associations {
            if dialects.get(name).is_none() {
                bail!("files.associations: unknown dialect {name:?} for {glob:?}");
            }
            let matcher = globset::GlobBuilder::new(glob)
                .literal_separator(false)
                .build()
                .with_context(|| format!("files.associations: bad glob {glob:?}"))?
                .compile_matcher();
            associations.push((matcher, name.clone()));
        }
        if dialects.get(&config.files.default_dialect).is_none() {
            bail!(
                "files.default_dialect: unknown dialect {:?}",
                config.files.default_dialect
            );
        }
        Ok(Self {
            config,
            dialects,
            associations,
        })
    }

    /// Picks a dialect: associations, client language id, `#lang`/modeline,
    /// extension, then the configured default.
    pub fn detect(
        &self,
        path: Option<&Path>,
        language_id: Option<&str>,
        text: &str,
    ) -> Arc<Dialect> {
        let d = &self.dialects;
        let by_assoc = || {
            let path = path?;
            let (_, name) = self.associations.iter().find(|(g, _)| g.is_match(path))?;
            d.get(name)
        };
        let by_first_line = || {
            let line = text.lines().next()?;
            if let Some(lang) = line.strip_prefix("#lang ") {
                let lang = lang.trim().split('/').next()?;
                return d.by_modeline(lang).or_else(|| d.by_modeline("racket"));
            }
            let (_, rest) = line.split_once("-*-")?;
            let (inner, _) = rest.split_once("-*-")?;
            let mode = inner
                .split(';')
                .find_map(|kv| kv.trim().strip_prefix("mode:"))
                .unwrap_or(inner)
                .trim();
            d.by_modeline(mode)
        };
        by_assoc()
            .or_else(|| language_id.and_then(|id| d.by_language_id(id)))
            .or_else(by_first_line)
            .or_else(|| d.by_extension(path?.extension()?.to_str()?))
            .or_else(|| d.get(&self.config.files.default_dialect))
            .or_else(|| d.iter().next())
            .cloned()
            .expect("at least one dialect")
    }
}

pub fn user_config_path() -> Option<PathBuf> {
    use etcetera::BaseStrategy;
    let base = etcetera::choose_base_strategy().ok()?;
    Some(base.config_dir().join("llsp").join("config.toml"))
}

fn read_table(path: &Path) -> Result<toml::Table> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

pub fn env_table(vars: impl IntoIterator<Item = (String, String)>) -> Result<toml::Table> {
    let mut out = toml::Table::new();
    for (key, value) in vars {
        if !key.starts_with(ENV_PREFIX) || RESERVED_ENV.contains(&key.as_str()) {
            continue;
        }
        let path = key[ENV_PREFIX.len()..]
            .split("__")
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>();
        insert_path(
            &mut out,
            path.iter().map(String::as_str),
            parse_value(&value),
        )
        .with_context(|| format!("env {key}"))?;
    }
    Ok(out)
}

fn parse_value(raw: &str) -> toml::Value {
    toml::from_str::<toml::Table>(&format!("v = {raw}"))
        .ok()
        .and_then(|mut t| t.remove("v"))
        .unwrap_or_else(|| toml::Value::String(raw.to_owned()))
}

fn insert_path<'a>(
    table: &mut toml::Table,
    path: impl Iterator<Item = &'a str>,
    value: toml::Value,
) -> Result<()> {
    let keys: Vec<&str> = path.collect();
    let Some((last, parents)) = keys.split_last() else {
        bail!("empty key");
    };
    if keys.iter().any(|k| k.is_empty()) {
        bail!("empty key segment in {}", keys.join("."));
    }
    let mut cur = table;
    for k in parents {
        let entry = cur
            .entry((*k).to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let toml::Value::Table(t) = entry else {
            bail!("{k} is not a table");
        };
        cur = t;
    }
    cur.insert((*last).to_owned(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::json_to_table;

    fn table(src: &str) -> toml::Table {
        toml::from_str(src).unwrap()
    }

    #[test]
    fn defaults_are_valid() {
        let s = Layers::default().resolve().unwrap();
        assert_eq!(s.config.format.body_indent, 2);
        assert_eq!(s.config.files.default_dialect, "common-lisp");
        assert!(s.config.workspace.index);
        assert_eq!(s.config.completion.max_items, 200);
        assert!(s.config.completion.builtins);
    }

    #[test]
    fn unknown_key_is_named() {
        let err = Settings::from_table(table("[fromat]\nx = 1")).unwrap_err();
        assert!(format!("{err:#}").contains("fromat"), "{err:#}");
    }

    #[test]
    fn invalid_severity_is_named() {
        let err =
            Settings::from_table(table("[diagnostics]\nunused_binding = \"loud\"")).unwrap_err();
        assert!(format!("{err:#}").contains("unused_binding"), "{err:#}");
        let s = Settings::from_table(table("[diagnostics]\nunresolved_call = \"error\"")).unwrap();
        assert_eq!(s.config.diagnostics.unresolved_call, Level::Error);
    }

    #[test]
    fn wrong_type_is_named() {
        let err = Settings::from_table(table("[format]\nbody_indent = \"x\"")).unwrap_err();
        assert!(format!("{err:#}").contains("body_indent"), "{err:#}");
    }

    #[test]
    fn numeric_bounds_are_enforced() {
        let err = Settings::from_table(table("[diagnostics]\ndebounce_ms = 9223372036854775807"))
            .unwrap_err();
        assert!(format!("{err:#}").contains("debounce_ms"), "{err:#}");
        let err = Settings::from_table(table("[format]\nbody_indent = 4000000000")).unwrap_err();
        assert!(format!("{err:#}").contains("body_indent"), "{err:#}");
        let err = Settings::from_table(table("[format]\ndistinguished_indent = 1001")).unwrap_err();
        assert!(
            format!("{err:#}").contains("distinguished_indent"),
            "{err:#}"
        );
        let s = Settings::from_table(table(
            "[diagnostics]\ndebounce_ms = 60000\n[format]\nbody_indent = 1000",
        ))
        .unwrap();
        assert_eq!(s.config.diagnostics.debounce_ms, 60_000);
        assert_eq!(s.config.format.body_indent, 1000);
        let s = Settings::from_table(table("[diagnostics]\ndebounce_ms = 0")).unwrap();
        assert_eq!(s.config.diagnostics.debounce_ms, 0);
    }

    #[test]
    fn precedence() {
        let layers = Layers {
            project: table("[format]\nbody_indent = 2\ndistinguished_indent = 6"),
            env: env_table([("LLSP_FORMAT__BODY_INDENT".into(), "4".into())]).unwrap(),
            ..Layers::default()
        };
        let s = layers.resolve().unwrap();
        assert_eq!(s.config.format.body_indent, 4);
        assert_eq!(s.config.format.distinguished_indent, 6);

        let layers = Layers {
            cli: table("[format]\nbody_indent = 3"),
            init: json_to_table(serde_json::json!({"format": {"body_indent": 5}})).unwrap(),
            ..Layers::default()
        };
        assert_eq!(layers.resolve().unwrap().config.format.body_indent, 5);
    }

    #[test]
    fn env_mapping() {
        let t = env_table([
            ("LLSP_WORKSPACE__INDEX".into(), "false".into()),
            ("LLSP_LOG__FILE".into(), "/var/log/llsp.log".into()),
            ("LLSP_CONFIG".into(), "ignored".into()),
            ("LLSP_LOG".into(), "debug".into()),
            ("OTHER".into(), "x".into()),
        ])
        .unwrap();
        assert_eq!(
            t,
            table("[workspace]\nindex = false\n[log]\nfile = \"/var/log/llsp.log\"")
        );
        let s = Settings::from_table(t).unwrap();
        assert!(!s.config.workspace.index);
    }

    #[test]
    fn scalar_then_nested_env_conflict_is_named() {
        let err = env_table([
            ("LLSP_FORMAT".into(), "3".into()),
            ("LLSP_FORMAT__BODY_INDENT".into(), "4".into()),
        ])
        .unwrap_err();
        assert!(format!("{err:#}").contains("format"), "{err:#}");
    }

    #[test]
    fn cli_sets() {
        let l = Layers::startup(
            Some(Path::new("/nonexistent/llsp.toml")),
            &["format.body_indent=3".into()],
        );
        assert!(l.is_err(), "missing explicit config file is an error");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        std::fs::write(&path, "[workspace]\nmax_files = 10").unwrap();
        let l = Layers::startup(Some(&path), &["format.body_indent = 3".into()]).unwrap();
        let s = l.resolve().unwrap();
        assert_eq!(s.config.format.body_indent, 3);
        assert_eq!(s.config.workspace.max_files, 10);
        assert!(Layers::startup(Some(&path), &["novalue".into()]).is_err());
    }

    #[test]
    fn project_file_cannot_set_log() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(PROJECT_FILE),
            "[log]\nfile = \"/tmp/x\"\nlevel = \"trace\"\n[format]\nbody_indent = 3",
        )
        .unwrap();
        let mut l = Layers::default();
        l.load_project(dir.path()).unwrap();
        let s = l.resolve().unwrap();
        assert_eq!(s.config.format.body_indent, 3);
        assert!(s.config.log.file.is_none());
        assert_eq!(s.config.log.level, "warn");
    }

    #[test]
    fn json_nulls_dropped() {
        let t = json_to_table(serde_json::json!({"log": {"file": null, "level": "info"}})).unwrap();
        assert_eq!(t, table("[log]\nlevel = \"info\""));
        assert!(json_to_table(serde_json::json!([1])).is_err());
        assert!(json_to_table(serde_json::Value::Null).unwrap().is_empty());
    }

    #[test]
    fn unknown_association_dialect() {
        let err =
            Settings::from_table(table("[files.associations]\n\"*.x\" = \"nope\"")).unwrap_err();
        assert!(err.to_string().contains("nope"), "{err}");
    }

    #[test]
    fn dialect_override_through_config() {
        let s = Settings::from_table(table(
            "[dialects.clojure.defs]\ndefroute = { kind = \"function\" }",
        ))
        .unwrap();
        assert!(
            s.dialects
                .get("clojure")
                .unwrap()
                .def_spec("defroute")
                .is_some()
        );
    }

    #[test]
    fn detection_order() {
        let layers = Layers {
            project: toml::from_str("[files.associations]\n\"*.lsp\" = \"emacs-lisp\"").unwrap(),
            ..Layers::default()
        };
        let s = layers.resolve().unwrap();
        let name = |p: Option<&str>, id: Option<&str>, text: &str| {
            s.detect(p.map(Path::new), id, text).name.clone()
        };
        assert_eq!(name(Some("/a.lsp"), Some("lisp"), ""), "emacs-lisp");
        assert_eq!(name(Some("/a.lisp"), Some("clojure"), ""), "clojure");
        assert_eq!(
            name(Some("/script"), Some("x"), ";; -*- mode: clojure -*-\n"),
            "clojure"
        );
        assert_eq!(name(Some("/script"), None, ";; -*- Scheme -*-\n"), "scheme");
        assert_eq!(name(Some("/m"), None, "#lang racket/base\n"), "racket");
        assert_eq!(name(Some("/a.fnl"), None, ""), "fennel");
        assert_eq!(name(Some("/a.unknown"), None, ""), "common-lisp");
        assert_eq!(name(None, None, ""), "common-lisp");
    }
}
