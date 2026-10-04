use strum::{EnumMessage, IntoEnumIterator};
use strum_macros::{self, Display, EnumIter, EnumMessage, EnumString};
use tower_lsp_server::ls_types::CompletionItem;

#[derive(Display, EnumString, EnumIter, EnumMessage)]
#[strum(serialize_all = "Train-Case")]
pub enum Header {
    #[strum(message = "The addresses you want to send to.")]
    To,
    #[strum(message = "The subject of the mail.")]
    Subject,
    #[strum(message = "The addresses you want to have carbon copy to.")]
    Cc,
    #[strum(message = "The addresses you want to have blind carbon copy to.")]
    Bcc,
    #[strum(message = "The address who writes this mail.")]
    From,
    #[strum(message = "The address you want them to reply to.")]
    ReplyTo,
    #[strum(disabled)]
    UnknownOrCustom,
}

impl Header {
    pub fn is_recipient(&self) -> bool {
        match self {
            Self::To | Self::Cc | Self::Bcc => true,
            _ => false,
        }
    }

    pub fn completion_items() -> Vec<CompletionItem> {
        Self::iter().map(Self::into).collect()
        // TODO: config custom headers
    }

    pub fn detect_from(s: &str) -> Option<Self> {
        let (header_part, _rest) = s.split_once(':')?;
        Some(header_part.parse().unwrap_or(Self::UnknownOrCustom))
    }
}

impl Into<CompletionItem> for Header {
    fn into(self) -> CompletionItem {
        CompletionItem::new_simple(format!("{self}: "), self.get_message().unwrap().to_owned())
    }
}
