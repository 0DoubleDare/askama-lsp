use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use askama_lsp::{
    config::AskamaLspConfig,
    structs::{Backend, Buffers, DocumentBuffers, TemplateBuffers},
};
use tokio::sync::RwLock;
use tower_lsp_server::{LspService, Server};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .compact()
        .with_ansi(false)
        .init();

    tracing::info!("Initialize logging");

    let config = AskamaLspConfig::default();

    tracing::info!("Initialize default config: {:#?}", config);

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        config: Arc::new(RwLock::new(config)),
        buffers: Buffers {
            fields_cache: Arc::new(RwLock::new(TemplateBuffers::with_capacity(512))),
            documents: Arc::new(RwLock::new(DocumentBuffers::with_capacity(512))),
        },
        workspace_folders: Mutex::new(Vec::new()),
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}
