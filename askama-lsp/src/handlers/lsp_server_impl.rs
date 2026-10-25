use std::{collections::HashSet, path::PathBuf};

use tower_lsp_server::{
    LanguageServer,
    jsonrpc::Result,
    ls_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, CompletionTextEdit, DidChangeTextDocumentParams,
        DidChangeWorkspaceFoldersParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
        Documentation, GotoDefinitionParams, GotoDefinitionResponse, InitializeParams,
        InitializeResult, InitializedParams, MessageType, OneOf, Position, Range, SaveOptions,
        ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
        TextDocumentSyncOptions, TextEdit,
    },
};
use tracing::{debug, trace};

use crate::{
    config::AskamaLspConfig,
    structs::{Backend, TemplateFieldInfo},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AskamaContext {
    None,
    RootBrace,
    Expression,
    Block,
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let folders = params.workspace_folders.unwrap_or_default();
        debug!(
            workspace_folder_count = folders.len(),
            "Initializing Askama LSP"
        );
        {
            let mut roots = self.workspace_folders.lock().unwrap();
            *roots = folders;
        }

        self.reload_config().await;
        self.refresh_workspace_field_cache().await;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "{".to_string(),
                        "|".to_string(),
                    ]),
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
        let position = params.text_document_position.position;
        let uri_keys = template_uri_keys(&uri);

        let document_text = {
            let docs = self.buffers.documents.read().await;
            docs.get(&uri).cloned()
        };

        let fields = {
            let fields_cache = self.buffers.fields_cache.read().await;
            let mut merged = Vec::new();
            for key in &uri_keys {
                if let Some(cached_fields) = fields_cache.get(key) {
                    merged.extend(cached_fields.clone());
                }
            }
            let mut seen = HashSet::new();
            merged.retain(|field| seen.insert((field.parent_struct.clone(), field.name.clone())));
            merged
        };

        debug!(
            uri = uri.as_str(),
            uri_keys = ?uri_keys,
            field_count = fields.len(),
            has_document = document_text.is_some(),
            "Computing completion response"
        );

        let items = completion_items(fields.as_slice(), document_text.as_deref(), position);
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text.unwrap_or_default();
        debug!(uri = uri.as_str(), bytes = content.len(), "Document saved");
        {
            let mut docs = self.buffers.documents.write().await;
            docs.insert(uri.clone(), content);
        }
        if uri.as_str().ends_with(".rs") {
            self.refresh_workspace_field_cache().await;
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let is_rust = uri.as_str().ends_with(".rs");
        debug!(
            uri = uri.as_str(),
            is_rust,
            bytes = content.len(),
            "Document opened"
        );

        {
            let mut docs = self.buffers.documents.write().await;
            docs.insert(uri, content);
        }

        if is_rust {
            self.refresh_workspace_field_cache().await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let is_rust = uri.as_str().ends_with(".rs");
        if let Some(change) = params.content_changes.last() {
            trace!(
                uri = uri.as_str(),
                bytes = change.text.len(),
                "Document changed"
            );
            let mut docs = self.buffers.documents.write().await;
            docs.insert(uri.clone(), change.text.clone());
        }

        if is_rust {
            self.refresh_workspace_field_cache().await;
        }
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        debug!(
            added = params.event.added.len(),
            removed = params.event.removed.len(),
            "Workspace folders changed"
        );
        {
            let mut folders = self.workspace_folders.lock().unwrap();
            folders.retain(|folder| {
                !params
                    .event
                    .removed
                    .iter()
                    .any(|removed| removed.uri == folder.uri)
            });
            folders.extend(params.event.added);
        }

        self.reload_config().await;
        self.refresh_workspace_field_cache().await;
    }

    async fn goto_definition(
        &self,
        _params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(None)
    }
}

impl Backend {
    async fn reload_config(&self) {
        let roots = self
            .workspace_folders
            .lock()
            .unwrap()
            .iter()
            .filter_map(|folder| folder.uri.to_file_path().map(|path| path.into_owned()))
            .collect::<Vec<PathBuf>>();

        let (config, warnings) = AskamaLspConfig::from_workspace_roots(roots.as_slice());
        for warning in warnings {
            self.client.log_message(MessageType::ERROR, warning).await;
        }

        let mut current = self.config.write().await;
        *current = config;
    }
}

fn completion_items(
    fields: &[TemplateFieldInfo],
    document_text: Option<&str>,
    position: Position,
) -> Vec<CompletionItem> {
    let (context, prefix, range, has_filter_pipe) =
        completion_context(document_text.unwrap_or(""), position);
    trace!(
        ?context,
        %prefix,
        ?range,
        has_filter_pipe,
        "Resolved Askama completion context"
    );

    let mut items = Vec::new();

    match context {
        AskamaContext::Expression => {
            items.extend(field_completion_items(fields, prefix.as_str(), range));
            items.extend(filter_completion_items(
                prefix.as_str(),
                range,
                has_filter_pipe,
            ));
        }
        AskamaContext::Block => {
            items.extend(field_completion_items(fields, prefix.as_str(), range));
            items.extend(keyword_completion_items(prefix.as_str(), range));
        }
        AskamaContext::RootBrace => {
            items.extend(root_construct_items(range));
        }
        AskamaContext::None => {}
    }

    items
}

fn field_completion_items(
    fields: &[TemplateFieldInfo],
    prefix: &str,
    range: Range,
) -> Vec<CompletionItem> {
    fields
        .iter()
        .filter(|field| prefix.is_empty() || field.name.starts_with(prefix))
        .map(|field| CompletionItem {
            label: field.name.clone(),
            kind: Some(CompletionItemKind::FIELD),
            detail: Some(field.raw_type.clone()),
            documentation: Some(Documentation::String(format!(
                "{} ({})",
                field.parent_struct, field.relative_template_path
            ))),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: field.name.clone(),
            })),
            ..Default::default()
        })
        .collect()
}

fn keyword_completion_items(prefix: &str, range: Range) -> Vec<CompletionItem> {
    [
        ("if", "Start a conditional block"),
        ("else", "Fallback branch for an if block"),
        ("elif", "Else-if branch in a conditional block"),
        ("endif", "Close an if block"),
        ("for", "Start a loop block"),
        ("endfor", "Close a for block"),
        ("match", "Start a match block"),
        ("when", "Add a branch inside match"),
        ("endmatch", "Close a match block"),
        ("include", "Include another template"),
        ("extends", "Inherit from a base template"),
        ("import", "Import macros from another template"),
        ("macro", "Declare a template macro"),
        ("endmacro", "Close a macro block"),
        ("call", "Invoke a macro"),
        ("block", "Define a named block"),
        ("endblock", "Close a named block"),
        ("filter", "Start a filter block"),
        ("endfilter", "Close a filter block"),
        ("let", "Create a local variable"),
    ]
    .into_iter()
    .filter(|(label, _)| prefix.is_empty() || label.starts_with(prefix))
    .map(|(label, doc)| CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        documentation: Some(Documentation::String(doc.to_string())),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range,
            new_text: label.to_string(),
        })),
        ..Default::default()
    })
    .collect()
}

fn filter_completion_items(
    prefix: &str,
    range: Range,
    has_filter_pipe: bool,
) -> Vec<CompletionItem> {
    if !has_filter_pipe {
        return Vec::new();
    }

    [
        ("safe", "Do not escape the rendered value"),
        ("escape", "HTML-escape the rendered value"),
        ("lower", "Lowercase the rendered value"),
        ("upper", "Uppercase the rendered value"),
        ("trim", "Trim leading/trailing whitespace"),
        ("length", "Render the length of a collection"),
        ("default", "Fallback value for empty content"),
    ]
    .into_iter()
    .filter(|(label, _)| prefix.is_empty() || label.starts_with(prefix))
    .map(|(label, doc)| CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::FUNCTION),
        documentation: Some(Documentation::String(doc.to_string())),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range,
            new_text: label.to_string(),
        })),
        ..Default::default()
    })
    .collect()
}

fn root_construct_items(range: Range) -> Vec<CompletionItem> {
    [
        ("{{  }}", "Start an Askama expression"),
        ("{%  %}", "Start an Askama block"),
        ("{#  #}", "Start an Askama comment"),
    ]
    .into_iter()
    .map(|(label, doc)| CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        documentation: Some(Documentation::String(doc.to_string())),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range,
            new_text: label.to_string(),
        })),
        ..Default::default()
    })
    .collect()
}

fn completion_context(text: &str, position: Position) -> (AskamaContext, String, Range, bool) {
    let offset = position_to_offset(text, position);
    let prefix_start = word_start(text, offset);
    let prefix = text[prefix_start..offset].to_string();
    let context = detect_context(text, offset);

    let mut range = Range {
        start: Position {
            line: position.line,
            character: position
                .character
                .saturating_sub((offset - prefix_start) as u32),
        },
        end: position,
    };

    if context == AskamaContext::RootBrace {
        range = root_brace_replacement_range(text, position, offset);
    }

    let has_filter_pipe = if context == AskamaContext::Expression {
        let start = open_tag_index(text, offset, "{{");
        if let Some(start) = start {
            text[start + 2..offset].contains('|')
        } else {
            false
        }
    } else {
        false
    };

    (context, prefix, range, has_filter_pipe)
}

fn root_brace_replacement_range(text: &str, position: Position, offset: usize) -> Range {
    if offset > 0 && text[..offset].chars().next_back() == Some('{') {
        return Range {
            start: offset_to_position(text, offset - 1),
            end: position,
        };
    }

    Range {
        start: position,
        end: position,
    }
}

fn detect_context(text: &str, offset: usize) -> AskamaContext {
    if text[..offset].ends_with('{') {
        return AskamaContext::RootBrace;
    }

    let before = &text[..offset];
    let mut last_open: Option<(usize, AskamaContext)> = None;
    for (needle, context) in [
        ("{{", AskamaContext::Expression),
        ("{%", AskamaContext::Block),
    ] {
        if let Some(index) = before.rfind(needle) {
            if last_open.is_none_or(|(last, _)| index > last) {
                last_open = Some((index, context));
            }
        }
    }

    let Some((index, context)) = last_open else {
        return AskamaContext::None;
    };

    let close = match context {
        AskamaContext::Expression => "}}",
        AskamaContext::Block => "%}",
        AskamaContext::None | AskamaContext::RootBrace => return AskamaContext::None,
    };

    if before[index + 2..].contains(close) {
        AskamaContext::None
    } else {
        context
    }
}

fn open_tag_index(text: &str, offset: usize, open: &str) -> Option<usize> {
    text[..offset].rfind(open)
}

fn position_to_offset(text: &str, position: Position) -> usize {
    let mut line = 0u32;
    let mut offset = 0usize;
    for chunk in text.split_inclusive('\n') {
        if line == position.line {
            let char_offset = chunk
                .chars()
                .take(position.character as usize)
                .map(char::len_utf8)
                .sum::<usize>();
            return offset + char_offset.min(chunk.len());
        }

        offset += chunk.len();
        line += 1;
    }
    text.len()
}

fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut line_start = 0usize;
    for chunk in text.split_inclusive('\n') {
        let line_end = line_start + chunk.len();
        if offset < line_end {
            let character = text[line_start..offset].chars().count() as u32;
            return Position { line, character };
        }
        line += 1;
        line_start = line_end;
    }

    let character = text[line_start..text.len()].chars().count() as u32;
    Position { line, character }
}

fn template_uri_keys(uri: &tower_lsp_server::ls_types::Uri) -> Vec<String> {
    let mut keys = vec![uri.to_string()];

    if let Some(path) = uri.to_file_path() {
        let normalized = std::fs::canonicalize(path.as_ref()).unwrap_or(path.into_owned());
        if let Some(normalized_uri) = tower_lsp_server::ls_types::Uri::from_file_path(normalized) {
            let normalized_key = normalized_uri.to_string();
            if !keys.contains(&normalized_key) {
                keys.push(normalized_key);
            }
        }
    }

    keys
}

fn word_start(text: &str, mut offset: usize) -> usize {
    while offset > 0 {
        let ch = text[..offset].chars().next_back().unwrap();
        if ch.is_alphanumeric() || ch == '_' {
            offset -= ch.len_utf8();
        } else {
            break;
        }
    }
    offset
}

#[cfg(test)]
mod tests {
    use tower_lsp_server::ls_types::Position;

    use crate::structs::TemplateFieldInfo;

    use super::{AskamaContext, completion_items, detect_context, root_brace_replacement_range};

    fn sample_fields() -> Vec<TemplateFieldInfo> {
        vec![
            TemplateFieldInfo {
                parent_struct: "Page".to_string(),
                relative_template_path: "index.html".to_string(),
                raw_type: "String".to_string(),
                name: "name".to_string(),
                line: 1,
                character: 1,
            },
            TemplateFieldInfo {
                parent_struct: "Page".to_string(),
                relative_template_path: "index.html".to_string(),
                raw_type: "u32".to_string(),
                name: "age".to_string(),
                line: 2,
                character: 1,
            },
        ]
    }

    #[test]
    fn detects_expression_context_in_incomplete_template() {
        let text = "{{ na";
        let offset = text.len();
        assert_eq!(detect_context(text, offset), AskamaContext::Expression);
    }

    #[test]
    fn suggests_rust_fields_inside_expression() {
        let items = completion_items(
            sample_fields().as_slice(),
            Some("{{ na }}"),
            Position::new(0, 5),
        );

        assert!(items.iter().any(|item| item.label == "name"));
        assert!(!items.iter().any(|item| item.label == "age"));
    }

    #[test]
    fn suggests_askama_keywords_inside_block() {
        let items = completion_items(
            sample_fields().as_slice(),
            Some("{% en %}"),
            Position::new(0, 5),
        );

        assert!(items.iter().any(|item| item.label == "endif"));
        assert!(items.iter().any(|item| item.label == "endfor"));
    }

    #[test]
    fn suggests_root_constructs_after_opening_brace() {
        let items = completion_items(sample_fields().as_slice(), Some("{"), Position::new(0, 1));
        assert!(items.iter().any(|item| item.label == "{{  }}"));
        assert!(items.iter().any(|item| item.label == "{%  %}"));
    }

    #[test]
    fn root_brace_completion_replaces_typed_opening_brace() {
        let range = root_brace_replacement_range("{", Position::new(0, 1), 1);
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, 1));

        let items = completion_items(sample_fields().as_slice(), Some("{"), Position::new(0, 1));
        let expression = items
            .iter()
            .find(|item| item.label == "{{  }}")
            .and_then(|item| item.text_edit.as_ref())
            .expect("root expression completion item should have text edit");

        let tower_lsp_server::ls_types::CompletionTextEdit::Edit(edit) = expression else {
            panic!("expected regular text edit for root expression completion");
        };

        assert_eq!(edit.range.start, Position::new(0, 0));
        assert_eq!(edit.range.end, Position::new(0, 1));
        assert_eq!(edit.new_text, "{{  }}");
    }

    #[test]
    fn suggests_filters_after_pipe() {
        let items = completion_items(
            sample_fields().as_slice(),
            Some("{{ name | up }}"),
            Position::new(0, 12),
        );
        assert!(items.iter().any(|item| item.label == "upper"));
    }
}
