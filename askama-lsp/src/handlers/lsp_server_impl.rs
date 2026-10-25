use tower_lsp_server::{
    LanguageServer,
    jsonrpc::Result,
    ls_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, DidOpenTextDocumentParams, DidSaveTextDocumentParams, Documentation,
        GotoDefinitionParams, GotoDefinitionResponse, InitializeParams, InitializeResult,
        InitializedParams, MessageType, OneOf, SaveOptions, ServerCapabilities,
        TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    },
};

use crate::structs::Backend;

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(workspace_folders) = params.workspace_folders {
            let mut root = self.workspace_folders.lock().unwrap();
            *root = workspace_folders;
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), "{".to_string()]),
                    ..Default::default()
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(
                            tower_lsp_server::ls_types::TextDocumentSyncSaveOptions::SaveOptions(
                                SaveOptions {
                                    include_text: Some(true),
                                },
                            ),
                        ),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Initialized LSP server for Askama")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let _position = params.text_document_position.position;

        tracing::info!("Completion trigger");
        let mut items: Vec<CompletionItem> = Vec::with_capacity(512);

        let fields_cache_reader = self.buffers.fields_cache.read().await;

        self.client
            .log_message(MessageType::INFO, format!("Uri: {}", uri.to_string()))
            .await;
        self.client
            .log_message(
                MessageType::INFO,
                format!("Field path: {:#?}", self.buffers.fields_cache),
            )
            .await;

        if let Some(annotation) = fields_cache_reader.get(&uri.to_string()) {
            for field in annotation {
                items.push(CompletionItem {
                    label: field.name.clone(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some(field.raw_type.clone()),
                    documentation: Some(Documentation::String(field.parent_struct.clone())),
                    ..Default::default()
                });
            }
        }

        // let items: Vec<CompletionItem> = vec![CompletionItem {
        //     label: "name".to_string(),
        //     kind: Some(CompletionItemKind::FIELD),
        //     detail: Some("Жопа".to_string()),
        //     documentation: Some(Documentation::String("Не беси меня далбаеб".to_string())),
        //     ..Default::default()
        // }];

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        tracing::info!("SAVE");
        let uri = params.text_document.uri;
        let content = params.text.unwrap_or_default();

        // {
        //     let mut docs = self.buffers.documents.write().await;
        //     docs.insert(uri.clone(), content.clone());
        // }

        self.collect_fields_info(&uri, &content).await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;

        // {
        //     let mut docs = self.buffers.documents.write().await;
        //     docs.insert(uri.clone(), content.clone());
        // }

        self.collect_fields_info(&uri, &content).await;
        // if uri.path().to_string().ends_with(".rs") {
        //     let mut field_cache_guard = self.buffers.fields_cache.write().await;
        //     if let Err(e) = self.parse_rust_file(content, &mut field_cache_guard) {
        //         tracing::error!(
        //             "Failed to parse {:?} file. Error: {}",
        //             uri,
        //             e
        //         )
        //     }
        // }
    }

    async fn goto_definition(
        &self,
        _params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(None)
    }
}
