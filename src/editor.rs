use tower_lsp_server::ls_types::Position as Utf16Position;

#[derive(Debug, Default)]
pub struct Editor {
    pub text: String,
    cursor_position_utf16: Utf16Position,
}

impl Editor {
    pub fn cursor_line(&self) -> &str {
        self.text
            .lines()
            .nth(self.cursor_position_utf16.line as usize)
            .unwrap_or("")
    }

    pub fn cursor_position(&self) -> Position {
        Position {
            line: self.cursor_position_utf16.line as usize,
            character: str_indices::utf16::to_byte_idx(
                &self.cursor_line(),
                self.cursor_position_utf16.character as usize,
            ),
        }
    }

    pub fn cursor_section(&self) -> Section {
        // TODO: tree-sitter locating
        let cursor_line = self.cursor_position().line;

        if self
            .text
            .lines()
            .position(str::is_empty)
            .map(|first_blank_line_index| cursor_line <= first_blank_line_index.try_into().unwrap())
            .unwrap_or(true)
        {
            Section::Header
        } else {
            Section::Body
        }
    }

    pub fn line_before_cursor(&self) -> &str {
        &self.cursor_line()[..self.cursor_position().character]
    }

    pub fn set_cursor_position_utf16(&mut self, pos: Utf16Position) {
        self.cursor_position_utf16 = pos;
    }

    #[cfg(test)]
    pub fn parse_from_cursor_string(s: &str) -> Editor {
        use tower_lsp_server::ls_types::Position;

        let (before, after) = s
            .split_once("$0")
            .expect("string should contain exactly one $0 cursor marker.");

        assert!(
            !after.contains("$0"),
            "string should contain ONLY ONE $0 cursor marker."
        );

        let line = before.matches('\n').count() as u32;

        let index = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let character = before[index..].encode_utf16().count() as u32;

        Editor {
            text: format!("{before}{after}"),
            cursor_position_utf16: Position::new(line, character),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Header,
    Body,
}

// utf8 position
pub struct Position {
    pub line: usize,
    pub character: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn cursor_string_basic_and_edges() {
        // Cursor in the very beginning
        let editor = Editor::parse_from_cursor_string("$0ABCD\nEFGH\n");
        assert_eq!(editor.text, "ABCD\nEFGH\n");
        assert_eq!(editor.cursor_position().line, 0);
        assert_eq!(editor.cursor_position().character, 0);

        // Cursor in somewhere middle
        let editor = Editor::parse_from_cursor_string("ABCD\nEF$0GH\n");
        assert_eq!(editor.text, "ABCD\nEFGH\n");
        assert_eq!(editor.cursor_position().line, 1);
        assert_eq!(editor.cursor_position().character, 2);

        // Cursor in the very end (new line at end)
        let editor = Editor::parse_from_cursor_string("ABCD\nEFGH\n$0");
        assert_eq!(editor.text, "ABCD\nEFGH\n");
        assert_eq!(editor.cursor_position().line, 2);
        assert_eq!(editor.cursor_position().character, 0);

        // Cursor at the end of a line without trailing newline
        let editor = Editor::parse_from_cursor_string("ABCD\nEFGH$0");
        assert_eq!(editor.text, "ABCD\nEFGH");
        assert_eq!(editor.cursor_position().line, 1);
        assert_eq!(editor.cursor_position().character, 4);

        // Empty file with only cursor
        let editor = Editor::parse_from_cursor_string("$0");
        assert_eq!(editor.text, "");
        assert_eq!(editor.cursor_position().line, 0);
        assert_eq!(editor.cursor_position().character, 0);

        // Consecutive newlines (cursor on an empty line in between)
        let editor = Editor::parse_from_cursor_string("ABCD\n\n$0\nEFGH");
        assert_eq!(editor.text, "ABCD\n\n\nEFGH");
        assert_eq!(editor.cursor_position().line, 2);
        assert_eq!(editor.cursor_position().character, 0);
    }

    #[test]
    fn cursor_string_crlf_windows() {
        // CRLF line endings (\r\n) - ensure \r is correctly preserved in text
        let editor = Editor::parse_from_cursor_string("ABCD\r\nEF$0GH\r\n");
        assert_eq!(editor.text, "ABCD\r\nEFGH\r\n");
        assert_eq!(editor.cursor_position().line, 1);
        assert_eq!(editor.cursor_position().character, 2);
    }

    #[test]
    #[should_panic]
    fn cursor_string_missing_marker() {
        Editor::parse_from_cursor_string("ABCD\nEFGH");
    }

    #[test]
    #[should_panic]
    fn cursor_string_multiple_markers() {
        Editor::parse_from_cursor_string("AB$0CD\nEF$0GH");
    }

    #[test]
    fn cursor_in_header_section() {
        // Cursor in the very beginning
        let editor = Editor::parse_from_cursor_string(indoc! {"
                $0From: Alice alice@example.org
                To: Bob bob@example.org

                This is body :)
        "});
        assert_eq!(editor.cursor_section(), Section::Header);

        // Cursor in somewhere in the middle
        let editor = Editor::parse_from_cursor_string(indoc! {"
                From: Alice alice@example.org
                To: Bob$0 bob@example.org

                This is body :)
            "});
        assert_eq!(editor.cursor_section(), Section::Header);

        // Cursor in the blank line
        let editor = Editor::parse_from_cursor_string(indoc! {"
                From: Alice alice@example.org
                To: Bob bob@example.org
                $0
                This is body :)
            "});
        assert_eq!(editor.cursor_section(), Section::Header);

        // Cursor in body
        let editor = Editor::parse_from_cursor_string(indoc! {"
                From: Alice alice@example.org
                To: Bob bob@example.org
                
                This is body :)$0
            "});
        assert_eq!(editor.cursor_section(), Section::Body);

        // Only headers. No blank line
        let editor = Editor::parse_from_cursor_string(indoc! {"
                From: Alice alice@example.org
                To:$0 Bob bob@example.org
            "});
        assert_eq!(editor.cursor_section(), Section::Header);

        // Empty file
        let editor = Editor::parse_from_cursor_string("$0");
        assert_eq!(editor.cursor_section(), Section::Header);

        // The first line is blank
        let editor = Editor::parse_from_cursor_string(indoc! {"
                $0
                This is body :)
            "});
        assert_eq!(editor.cursor_section(), Section::Header);

        // Body has other blank lines
        let editor = Editor::parse_from_cursor_string(indoc! {"
                From: Alice alice@example.org

                Body line 1

                Body line $02
            "});
        assert_eq!(editor.cursor_section(), Section::Body);
    }
}
