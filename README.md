# Askama LSP

Language Server Protocol implementation for Rust Askama templates.

## Features

- Rust template-context field completion inside `{{ ... }}` and `{% ... %}` blocks.
- Core Askama construct completion (`if`, `for`, `include`, `extends`, `import`, `macro`, etc.).
- Filter completion after `|` in expressions.
- Workspace-aware config loading with defaults, Cargo metadata, and config-file overrides.

## Configuration

The server merges configuration in this order:

1. Built-in defaults
2. `[package.metadata.askama-lsp]` from `Cargo.toml`
3. `askama-lsp.toml` (or `.askama-lsp.toml`) in the workspace root

`askama-lsp.toml` format:

```toml
[askama-lsp]
templates = ["./templates"]
templates_extensions = ["html", "askama", "j2"]
source_directories = ["./src"]
```

You can also place these keys at top level (without `[askama-lsp]`).
