use zed_extension_api::{self as zed, Result};

struct AskamaLspExtension;

impl zed::Extension for AskamaLspExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> Result<zed::Command> {
        let _ = language_server_id;

        let lsp_path = worktree.which("askama-lsp").unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/.cargo/bin/askama-lsp")
        });

        Ok(zed::Command {
            command: lsp_path,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(AskamaLspExtension);
