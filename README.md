# llsp

Fast, static language server for Lisp dialects, written in Rust.

Supports Common Lisp, Clojure (clj/cljs/cljc/edn), Scheme (R7RS, Guile, Chicken), Racket,
Emacs Lisp, Fennel, Janet and the two Lispico dialects `lispico-clojure` and `lispico-cl`.
The Lispico dialects claim no extensions: llsp picks them up by language ID
(`lispico-clojure`, `lispico-cl`) or through `files.associations`. Each dialect is a TOML
file in [`dialects/`](dialects) that you
can override, extend, or build on with `extends`.

llsp never evaluates your code: no reader macros, build tools, subprocesses or network access.

Indexing 1000 files with 50k definitions takes about 143 ms cold; definition and hover answer
in about 70 µs (`task bench`, shared cloud VM).

## Features

- Diagnostics: syntax errors plus `unused-binding`, `duplicate-definition` and
  `unresolved-call` lints (also via `llsp check`)
- Go to definition, references, highlights, rename, with namespaces and aliases resolved
- Completion, signature help, hover
- Document outline, workspace symbols, semantic highlighting
- Folding ranges, expand selection
- Formatting that only re-indents

## Install

```sh
curl -fsSL https://github.com/victorzhuk/llsp/releases/latest/download/install.sh | sh
```

Installs a checksum-verified binary into `~/.local/bin` on Linux and macOS. Set
`LLSP_VERSION` or `LLSP_INSTALL_DIR` to change the release or directory. Other builds are on
the [releases page](https://github.com/victorzhuk/llsp/releases), or build from source:

```sh
cargo install --locked --git https://github.com/victorzhuk/llsp
```

## Usage

`llsp` with no arguments speaks LSP over stdio. Neovim:

```lua
vim.lsp.config('llsp', {
  cmd = { 'llsp' },
  filetypes = { 'lisp', 'clojure', 'scheme', 'racket', 'elisp', 'fennel', 'janet' },
  root_markers = { '.llsp.toml', '.git' },
})
vim.lsp.enable('llsp')
```

```
llsp [--config PATH] [--set KEY=VALUE]... [--log-level LEVEL] [--log-file PATH] [COMMAND]

  serve      serve LSP (default); --listen 127.0.0.1:PORT for loopback TCP
  check      report diagnostics; exit 1 on errors, 2 on I/O errors
  format     re-indent files in place; --check exits 1 if anything would change
  config     print the effective configuration
  dialects   list dialects and their file extensions
```

`check`, `format`, `config` and `dialects` accept `--format text|json`. By default the output
is text on a terminal and JSON otherwise.

## Configuration

Later sources win: built-in defaults → `$XDG_CONFIG_HOME/llsp/config.toml` (or `--config`,
`LLSP_CONFIG`) → `.llsp.toml` in the workspace root → `LLSP_<SECTION>__<KEY>` env vars →
`--set` → editor `initializationOptions` → `workspace/didChangeConfiguration`.

`llsp config` prints the effective settings. Unknown keys are rejected. Lint severities are
`off`, `hint`, `info`, `warning` or `error`.

```toml
[diagnostics]
unresolved_call = "warning"       # off by default

[format]
body_indent = 2

[dialects.common-lisp.indent]
my-with-macro = 1

[dialects.lfe]
extends = "common-lisp"
extensions = ["lfe"]
```

Dialect keys include `extends`, `extensions`, `language_ids`, `case_sensitive`, `reader`,
`defs`, `bindings`, `indent`, `indent_prefixes`, `special_forms` and `builtins`; see
[`dialects/`](dialects) for complete definitions.

The dialect of a file comes from the first match of: `files.associations` (glob → dialect),
the editor's `languageId`, a `#lang` line or `-*- mode: X -*-` modeline, the file extension,
`files.default_dialect`.

For a Lispico project whose files use the `.lpc` extension, associate them in
`settings.json`:

```json
{
  "files.associations": {
    "*.lpc": "lispico-cl"
  }
}
```

## Security

- No evaluation, macro expansion, reader macros (`#.`, `#=`), build tools or subprocesses.
- No network except the loopback-only `--listen` socket. Indexing stays inside the workspace
  roots and doesn't follow symlinks. File size, file count and syntax errors per file are
  capped.
- `.llsp.toml` comes from the repository you open. It can change behavior and limits, but it
  cannot make llsp run anything, and its `[log]` section is ignored.
- Logs never contain document text; log files are created with mode 0600.

## Development

```sh
task build   # release binary
task test    # tests
task lint    # rustfmt + clippy
task bench   # criterion benchmarks
task ci      # everything CI runs
```

Changes are specified first under [`openspec/`](openspec).

To release, move the `Unreleased` notes in `CHANGELOG.md` under the new version, bump
`version` in `Cargo.toml`, merge, then push a `vX.Y.Z` tag. The release workflow builds and
publishes the binaries with checksums and `install.sh`.

## License

Apache-2.0
