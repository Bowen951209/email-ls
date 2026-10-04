mod complete;
pub(crate) mod editor;

use std::fmt;

use tokio::sync::Mutex;
use tower_lsp_server::{Client, LanguageServer, jsonrpc, ls_types::*};

use crate::{complete::CompleterManager, editor::Editor};

pub struct Backend {
    client: Client,
    editor: Mutex<Editor>, // TODO: make new instance every time so no mutex?
    completer_manager: CompleterManager,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            editor: Mutex::new(Editor::default()),
            completer_manager: CompleterManager::new(),
        }
    }

    async fn set_editor_text(&self, text: String) {
        let mut guard = self.editor.lock().await;
        *(&mut guard.text) = text;
    }

    async fn set_editor_cursor(&self, pos: Position) {
        let mut guard = self.editor.lock().await;
        (&mut guard).set_cursor_position_utf16(pos);
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncKind::FULL.into()), // TODO: use Increment?
                completion_provider: Some(CompletionOptions::default()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.set_editor_text(params.content_changes.pop().unwrap().text)
            .await;
        self.client
            .log_message(MessageType::LOG, "Update full document.")
            .await;
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        let position = params.text_document_position.position;
        self.set_editor_cursor(position).await;
        let editor = &(*self.editor.lock().await);

        match self.completer_manager.complete(editor).await {
            Ok(items) => Ok(Some(CompletionResponse::Array(items))),
            Err(e) => {
                let msg = fmt::from_fn(|f| write!(f, "Error in completion: {e}"));
                self.client.log_message(MessageType::ERROR, msg).await;
                Ok(None)
            }
        }
    }
}
