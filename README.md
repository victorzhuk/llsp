# llsp

Fast, static language server for Lisp dialects, written in Rust.

- **Dialects:** Common Lisp, Clojure (clj/cljs/cljc/edn), Scheme (R7RS, Guile, Chicken),
  Racket, Emacs Lisp, Fennel and Janet. Each is a TOML data file, so you can extend or override
  it, or add a new dialect with `extends`.
- **Static:** llsp never evaluates your code. It runs no reader macros, no `#.`, no build tools
  and no subprocesses, and opens no network connections.
- **Configurable:** every setting can come from a file, an environment variable, the command
  line or the editor.

## Features

- Diagnostics:
  - syntax errors: unbalanced, mismatched or unexpected delimiters, unterminated strings and
    comments
  - lints, each with its own severity: `unused-binding`, `duplicate-definition`, and
    `unresolved-call` (off by default)
  - the same diagnostics are available from `llsp check`
- Document outline and fuzzy workspace symbol search
- Go to definition, find references, document highlights, for locals (with shadowing) and
  workspace names (namespaces and aliases taken into account)
- Rename, both local and across the workspace, with validation of the new name
- Completion: visible locals, workspace definitions, dialect special forms and builtins, with
  fuzzy ranking; `ns/`, `pkg:` and alias prefixes narrow to that namespace
- Signature help that picks the matching arity and understands lambda-list markers
  (`&optional`, `&rest`, `&`)
- Hover with signature, kind, namespace, docstring and location
- Semantic highlighting: definitions, parameters, locals, macros, special forms, builtins,
  keywords, namespaces, regexes, and datum comments shown as comments
- Folding ranges and expand selection

## Install

```sh
cargo install --git https://github.com/victorzhuk/llsp
```

## Editor setup

Start `llsp` with no arguments. It speaks LSP over stdio. For example, in Neovim:

```lua
vim.lsp.config('llsp', {
  cmd = { 'llsp' },
  filetypes = { 'lisp', 'clojure', 'scheme', 'racket', 'elisp', 'fennel', 'janet' },
  root_markers = { '.llsp.toml', '.git' },
})
vim.lsp.enable('llsp')
```

`llsp serve --listen 127.0.0.1:9257` serves a single client over TCP. Only loopback addresses
are accepted.

## Command line

```
llsp [--config PATH] [--set KEY=VALUE]... [--log-level LEVEL] [--log-file PATH] [COMMAND]

  serve      serve LSP (default)
  check      report diagnostics for files or directories; exit 1 on errors, 2 on I/O errors
  config     print the effective configuration
  dialects   list dialects and their file extensions
```

`check`, `config` and `dialects` accept `--format text|json`. When `--format` isn't given, the
output is text on a terminal and JSON otherwise, so both scripts and CI can parse it.

## Configuration

Sources are merged in this order, with later sources winning. Tables merge key by key; any
other value is replaced.

1. Built-in defaults (`llsp config` prints the effective result)
2. User file: `$XDG_CONFIG_HOME/llsp/config.toml`, or `--config PATH` / `LLSP_CONFIG`
3. Project file: `.llsp.toml` in the workspace root
4. Environment: `LLSP_<SECTION>__<KEY>=value`, for example `LLSP_FORMAT__BODY_INDENT=4`.
   Values are parsed as TOML literals and fall back to strings.
5. Command line: `--set format.body_indent=4`
6. `initializationOptions` sent by the editor
7. `workspace/didChangeConfiguration` settings, either as the whole object or under an
   `llsp` key

Unknown keys are rejected. At startup the server exits with an error. When the editor sends
a bad setting, llsp shows a warning and keeps the previous configuration.

```toml
[files]
default_dialect = "common-lisp"   # used when nothing else matches
max_file_size = 8388608           # larger files are not analyzed

[files.associations]              # glob -> dialect; checked first
"*.lsp" = "emacs-lisp"

[workspace]
index = true
exclude = ["**/node_modules/**", "**/target/**"]
max_files = 20000
max_symbols = 256                 # workspace/symbol result cap

[completion]
max_items = 200
builtins = true                   # offer dialect special forms and builtins

[diagnostics]
enable = true
debounce_ms = 100
unused_binding = "hint"           # off | hint | info | warning | error
duplicate_definition = "warning"
unresolved_call = "off"           # calls to names defined nowhere in the workspace
ignore_prefix = "_"               # bindings starting with this are never "unused"
known_symbols = []                # extra names unresolved-call accepts

[format]
body_indent = 2
distinguished_indent = 4
trim_trailing_whitespace = true

[log]
level = "warn"                    # or --log-level / LLSP_LOG
# file = "/path/to/llsp.log"      # created with mode 0600

# Extend a built-in dialect:
[dialects.clojure.defs]
defroute = { kind = "function", params = "vector" }

# Add a dialect:
[dialects.lfe]
extends = "common-lisp"
extensions = ["lfe"]
case_sensitive = true
```

### Dialect detection

For each file, llsp takes the first of these that matches:

1. `files.associations`
2. The editor's `languageId`
3. A `#lang` line or `-*- mode: X -*-` modeline on the first line
4. The file extension
5. `files.default_dialect`

The built-in dialect definitions live in [`dialects/`](dialects). A dialect definition covers:

- **Reader rules:** brackets, comments, character literals, prefixes and how many forms each
  prefix takes
- **Definition forms:** where the name and parameters are
- **Binding forms**
- **Indentation specs**
- **Special forms and builtins**

## Development

```sh
task build   # release binary in target/release/llsp
task test    # tests (5 min timeout, 4 threads)
task lint    # rustfmt + clippy
task bench   # criterion benchmarks
task spec    # validate OpenSpec specs and changes
```

Changes are specified first under [`openspec/`](openspec).

## License

Apache-2.0
