mod header;

use std::{marker::PhantomData, process::Stdio};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use tower_lsp_server::ls_types::CompletionItem;

use crate::{
    complete::header::Header,
    editor::{Editor, Section},
};

pub struct CompleterManager {
    header_completer: HeaderCompleter,
    template_completer: TemplateCompleter,
    contact_completer: ContactCompeter,
}

impl CompleterManager {
    pub fn new() -> Self {
        Self {
            header_completer: HeaderCompleter::new(),
            template_completer: TemplateCompleter::new(),
            contact_completer: ContactCompeter::new(),
        }
    }

    pub async fn complete(&self, editor: &Editor) -> anyhow::Result<Vec<CompletionItem>> {
        let Some(completer) = self.determine_completer(editor) else {
            return Ok(Vec::new());
        };

        match completer {
            CompleterToUse::Header => self.header_completer.complete(editor).await,
            CompleterToUse::Contact => self.contact_completer.complete(editor).await,
            CompleterToUse::Template => self.template_completer.complete(editor).await,
        }
    }

    fn determine_completer(&self, editor: &Editor) -> Option<CompleterToUse> {
        match editor.cursor_section() {
            Section::Header => match Header::detect_from(editor.line_before_cursor()) {
                Some(header) if header.is_recipient() => Some(CompleterToUse::Contact),
                None => Some(CompleterToUse::Header),
                _ => None,
            },
            Section::Body => Some(CompleterToUse::Template),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompleterToUse {
    Header,
    Contact,
    Template,
}

struct Completer<B, S>
where
    B: CompletionBackend,
    S: Span,
{
    backend: B,
    _data: PhantomData<S>,
}

impl<M, S> Completer<M, S>
where
    M: CompletionBackend,
    S: Span,
{
    async fn complete(&self, editor: &Editor) -> anyhow::Result<Vec<CompletionItem>> {
        match S::extract_from(editor) {
            Some(c) => self.backend.complete(c).await,
            None => Ok(Vec::new()),
        }
    }

    fn with_backend(backend: M) -> Self {
        Self {
            backend,
            _data: PhantomData,
        }
    }
}

type HeaderCompleter = Completer<InMemoryBackend, LineSpan>;
impl HeaderCompleter {
    pub fn new() -> Self {
        Self::with_backend(InMemoryBackend::new(Header::completion_items()))
    }
}

type TemplateCompleter = Completer<InMemoryBackend, WordSpan>;
impl TemplateCompleter {
    pub fn new() -> Self {
        Self::with_backend(InMemoryBackend::new(
            // TODO: load from config
            vec![
                CompletionItem::new_simple(
                    "Thank you for your time.\nBest regards,\nBowen Ho".into(),
                    "".into(),
                ),
                CompletionItem::new_simple("I love you.\nBowen Ho.".into(), "".into()),
            ],
        ))
    }
}

type ContactCompeter = Completer<ExternalProcessBackend, ReceipientSpan>;
impl ContactCompeter {
    pub fn new() -> Self {
        Self::with_backend(ExternalProcessBackend::new())
    }
}

trait Span {
    fn extract_from(editor: &Editor) -> Option<&str>;
}

pub struct LineSpan;
impl Span for LineSpan {
    fn extract_from(editor: &Editor) -> Option<&str> {
        Some(editor.line_before_cursor())
    }
}

pub struct WordSpan;
impl Span for WordSpan {
    fn extract_from(editor: &Editor) -> Option<&str> {
        let line = editor.line_before_cursor();
        Some(line.rsplit_once(' ').map(|(_, w)| w).unwrap_or(line))
    }
}

pub struct ReceipientSpan;
impl Span for ReceipientSpan {
    fn extract_from(editor: &Editor) -> Option<&str> {
        let line = editor.line_before_cursor();
        line.split_once(':').map(|(_, reciepients)| {
            let reciepient = reciepients
                .rsplit_once(',')
                .map(|s| s.1)
                .unwrap_or(reciepients)
                .trim_start();
            reciepient.strip_prefix('"').unwrap_or(reciepient)
        })
    }
}

trait CompletionBackend {
    async fn complete(&self, context: &str) -> anyhow::Result<Vec<CompletionItem>>;
}

pub struct InMemoryBackend {
    items: Vec<CompletionItem>,
}

impl InMemoryBackend {
    fn new(items: Vec<CompletionItem>) -> Self {
        Self { items }
    }
}

impl CompletionBackend for InMemoryBackend {
    async fn complete(&self, prefix: &str) -> anyhow::Result<Vec<CompletionItem>> {
        Ok(self
            .items
            .iter()
            .filter(|item| item.label.starts_with(prefix))
            .cloned()
            .collect())
    }
}

struct ExternalProcessBackend;

impl ExternalProcessBackend {
    fn new() -> Self {
        Self
    }
}

impl CompletionBackend for ExternalProcessBackend {
    async fn complete(&self, context: &str) -> anyhow::Result<Vec<CompletionItem>> {
        let child = Command::new("khard")
            .args(&["email", "--parsable", context])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.unwrap();
        let mut lines = BufReader::new(stdout).lines();
        let mut items = Vec::new();

        // skip first line (mutt format)
        lines.next_line().await?;

        while let Some(line) = lines.next_line().await? {
            let mut infos = line.split_whitespace();
            let email_address = infos
                .next()
                .expect("Unexpected process output. Should have email.");
            let name = infos
                .next()
                .expect("Unexpected process output. Should have name.");

            items.push(CompletionItem::new_simple(
                format!("\"{name}\" <{email_address}>"),
                "TODO".into(),
            ));
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipient_span() {
        assert_span("From: John$0", "John");
        assert_span("From: \"John$0", "John");
        assert_span("From: John <john@example.com>, Bob$0", "Bob");
        assert_span("From: John <john@example.com>, \"Bob$0", "Bob");
        assert_span("From: John <john@example.com>  ,    \"Bob$0   ", "Bob");
    }

    fn assert_span(cursor_string: &str, expected_span: &str) {
        let editor = Editor::parse_from_cursor_string(cursor_string);
        let receipient = ReceipientSpan::extract_from(&editor).unwrap();
        assert_eq!(receipient, expected_span);
    }
}
