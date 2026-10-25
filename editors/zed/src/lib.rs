use zed_extension_api::{self as zed, Result, Worktree};

struct AskamaLspExtension;

impl zed::Extension for AskamaLspExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> Result<zed::Command>
    {
        let lsp_path = worktree
            .which("askama-lsp")
            .ok_or_else(|| "askama-lsp binary not found")?;

        Ok(zed::Command {
            command: lsp_path,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(AskamaLspExtension);