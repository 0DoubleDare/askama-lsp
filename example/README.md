## Askama LSP example

`src/main.rs` exposes `Page { name, age }` to `templates/index.html`.

For completion debugging:

- Open `templates/index.html` and trigger completion inside `{{ ... }}` to see `name` and `age`.
- Open `templates/brace_completion.html`, type `{`, then accept the `{{  }}` completion with Tab. The final text should be `{{  }}` (no extra `{`).