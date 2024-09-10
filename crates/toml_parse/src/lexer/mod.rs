//! Lex TOML tokens

mod token;

pub use token::Raw;
pub use token::Token;
pub use token::TokenKind;

use winnow::stream::Compare as _;
use winnow::stream::ContainsToken as _;
use winnow::stream::FindSlice as _;
use winnow::stream::Stream as _;

pub struct Lexer<'i> {
    stream: &'i [u8],
}

impl<'i> Lexer<'i> {
    pub(crate) fn new(input: &'i str) -> Self {
        Lexer {
            stream: input.as_bytes(),
        }
    }
}

impl<'i> Iterator for Lexer<'i> {
    type Item = Token<'i>;
    fn next(&mut self) -> Option<Self::Item> {
        let token = self.stream.first()?;
        debug_assert_utf8!(self.stream, "previous iteration ended on `char` boundary");
        let token = match token {
            b'.' => unsafe { lex_ascii_char(&mut self.stream, TokenKind::Dot) },
            b'=' => unsafe { lex_ascii_char(&mut self.stream, TokenKind::Equals) },
            b',' => unsafe { lex_ascii_char(&mut self.stream, TokenKind::Comma) },
            b'[' => unsafe { lex_ascii_char(&mut self.stream, TokenKind::LeftSquareBracket) },
            b']' => unsafe { lex_ascii_char(&mut self.stream, TokenKind::RightSquareBracket) },
            b'{' => unsafe { lex_ascii_char(&mut self.stream, TokenKind::LeftCurlyBracket) },
            b'}' => unsafe { lex_ascii_char(&mut self.stream, TokenKind::RightCurlyBracket) },
            b' ' => unsafe { lex_whitespace(&mut self.stream) },
            b'\t' => unsafe { lex_whitespace(&mut self.stream) },
            b'#' => unsafe { lex_comment(&mut self.stream) },
            b'\r' => unsafe { lex_crlf(&mut self.stream) },
            b'\n' => unsafe { lex_ascii_char(&mut self.stream, TokenKind::Newline) },
            b'\'' => {
                if matches!(
                    self.stream.compare(ML_LITERAL_STRING_DELIM.as_bytes()),
                    winnow::stream::CompareResult::Ok(_)
                ) {
                    unsafe { lex_ml_literal_string(&mut self.stream) }
                } else {
                    unsafe { lex_literal_string(&mut self.stream) }
                }
            }
            b'"' => {
                if matches!(
                    self.stream.compare(ML_BASIC_STRING_DELIM.as_bytes()),
                    winnow::stream::CompareResult::Ok(_)
                ) {
                    unsafe { lex_ml_basic_string(&mut self.stream) }
                } else {
                    unsafe { lex_basic_string(&mut self.stream) }
                }
            }
            _ => unsafe { lex_atom(&mut self.stream) },
        };
        debug_assert_utf8!(
            self.stream,
            "lex's post-condition is they end on `char` boundary"
        );
        Some(token)
    }
}

/// Process an ASCII character token
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream` must be non-empty
/// - `&stream[0]` must be ASCII
unsafe fn lex_ascii_char<'i>(stream: &mut &'i [u8], kind: TokenKind) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert!(!stream.is_empty());

    let offset = 1;
    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "only called when ASCII char is in stream");
    debug_assert_utf8!(slice, "only called when ASCII char is in stream");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };

    Token::new(kind, raw)
}

/// Process Whitespace
///
/// ```bnf
/// ;; Whitespace
///
/// ws = *wschar
/// wschar =  %x20  ; Space
/// wschar =/ %x09  ; Horizontal tab
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream` must be non-empty
unsafe fn lex_whitespace<'i>(stream: &mut &'i [u8]) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert!(!stream.is_empty());

    let offset = stream
        .offset_for(|b| !WSCHAR.contains_token(b))
        .unwrap_or(stream.eof_offset());

    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "`offset` was after ASCII whitespace");
    debug_assert_utf8!(slice, "`offset` was after ASCII whitespace");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };

    Token::new(TokenKind::Whitespace, raw)
}

/// ```bnf
/// wschar =  %x20  ; Space
/// wschar =/ %x09  ; Horizontal tab
/// ```
pub(crate) const WSCHAR: (u8, u8) = (b' ', b'\t');

/// Process Comment
///
/// ```bnf
/// ;; Comment
///
/// comment-start-symbol = %x23 ; #
/// non-ascii = %x80-D7FF / %xE000-10FFFF
/// non-eol = %x09 / %x20-7F / non-ascii
///
/// comment = comment-start-symbol *non-eol
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream[0] == b'#'`
unsafe fn lex_comment<'i>(stream: &mut &'i [u8]) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert_eq!(stream.get(0), Some(&COMMENT_START_SYMBOL));

    let mut offset = 1; // COMMENT_START_SYMBOL
    let next = &stream[offset..];
    offset += match next.find_slice((b'\r', b'\n')) {
        Some(span) => span.start,
        None => next.eof_offset(),
    };

    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "`offset` was after ASCII whitespace");
    debug_assert_utf8!(slice, "`offset` was after ASCII whitespace");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };
    Token::new(TokenKind::Comment, raw)
}

/// `comment-start-symbol = %x23 ; #`
pub(crate) const COMMENT_START_SYMBOL: u8 = b'#';

/// Process Newline
///
/// ```bnf
/// ;; Newline
///
/// newline =  %x0A     ; LF
/// newline =/ %x0D.0A  ; CRLF
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream[0] == b'\r'`
unsafe fn lex_crlf<'i>(stream: &mut &'i [u8]) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert_eq!(stream.get(0), Some(&b'\r'));

    let has_lf = stream.get(1) == Some(&b'\n');

    let mut offset = '\r'.len_utf8();
    if has_lf {
        offset += '\n'.len_utf8();
    }

    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "`offset` was after ASCII whitespace");
    debug_assert_utf8!(slice, "`offset` was after ASCII whitespace");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };

    Token::new(TokenKind::Newline, raw)
}

/// Process literal string
///
/// ```bnf
/// ;; Literal String
///
/// literal-string = apostrophe *literal-char apostrophe
///
/// apostrophe = %x27 ; ' apostrophe
///
/// literal-char = %x09 / %x20-26 / %x28-7E / non-ascii
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream[0] == b'\''`
unsafe fn lex_literal_string<'i>(stream: &mut &'i [u8]) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert_eq!(stream.get(0), Some(&APOSTROPHE));

    let mut offset = 1; // APOSTROPHE
    let next = &stream[offset..];
    offset += match next.find_slice((APOSTROPHE, b'\n')) {
        Some(span) => {
            if next[span.start] == APOSTROPHE {
                span.end
            } else {
                span.start
            }
        }
        None => next.eof_offset(),
    };

    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "`offset` was after ASCII");
    debug_assert_utf8!(slice, "`offset` was after ASCII");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };

    Token::new(TokenKind::LiteralString, raw)
}

/// `apostrophe = %x27 ; ' apostrophe`
pub(crate) const APOSTROPHE: u8 = b'\'';

/// Process multi-line literal string
///
/// ```bnf
/// ;; Multiline Literal String
///
/// ml-literal-string = ml-literal-string-delim [ newline ] ml-literal-body
///                     ml-literal-string-delim
/// ml-literal-string-delim = 3apostrophe
/// ml-literal-body = *mll-content *( mll-quotes 1*mll-content ) [ mll-quotes ]
///
/// mll-content = mll-char / newline
/// mll-char = %x09 / %x20-26 / %x28-7E / non-ascii
/// mll-quotes = 1*2apostrophe
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream.starts_with(ML_LITERAL_STRING_DELIM)`
unsafe fn lex_ml_literal_string<'i>(stream: &mut &'i [u8]) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert_eq!(stream.get(0), Some(&APOSTROPHE));

    let mut offset = ML_LITERAL_STRING_DELIM.len();
    let next = &stream[offset..];
    offset += match next.find_slice(ML_LITERAL_STRING_DELIM) {
        Some(span) => span.end,
        None => next.eof_offset(),
    };
    if stream.get(offset) == Some(&APOSTROPHE) {
        offset += 1;
    }
    if stream.get(offset) == Some(&APOSTROPHE) {
        offset += 1;
    }

    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "`offset` was after ASCII");
    debug_assert_utf8!(slice, "`offset` was after ASCII");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };

    Token::new(TokenKind::MlLiteralString, raw)
}

/// `ml-literal-string-delim = 3apostrophe`
pub(crate) const ML_LITERAL_STRING_DELIM: &str = "'''";

/// Process basic string
///
/// ```bnf
/// ;; Basic String
///
/// basic-string = quotation-mark *basic-char quotation-mark
///
/// quotation-mark = %x22            ; "
///
/// basic-char = basic-unescaped / escaped
/// basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
/// escaped = escape escape-seq-char
///
/// escape = %x5C                   ; \
/// escape-seq-char =  %x22         ; "    quotation mark  U+0022
/// escape-seq-char =/ %x5C         ; \    reverse solidus U+005C
/// escape-seq-char =/ %x62         ; b    backspace       U+0008
/// escape-seq-char =/ %x66         ; f    form feed       U+000C
/// escape-seq-char =/ %x6E         ; n    line feed       U+000A
/// escape-seq-char =/ %x72         ; r    carriage return U+000D
/// escape-seq-char =/ %x74         ; t    tab             U+0009
/// escape-seq-char =/ %x75 4HEXDIG ; uXXXX                U+XXXX
/// escape-seq-char =/ %x55 8HEXDIG ; UXXXXXXXX            U+XXXXXXXX
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream[0] == b'"'`
unsafe fn lex_basic_string<'i>(stream: &mut &'i [u8]) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert_eq!(stream.get(0), Some(&QUOTATION_MARK));

    let mut offset = 1; // QUOTATION_MARK
    let next = &stream[offset..];
    offset += match next.find_slice((QUOTATION_MARK, ESCAPE, b'\n')) {
        Some(span) => {
            if next[span.start] == QUOTATION_MARK {
                span.end
            } else {
                span.start
            }
        }
        None => next.eof_offset(),
    };
    while stream.get(offset) == Some(&ESCAPE) {
        offset += 1; // ESCAPE
        let peek = stream.get(offset);
        match peek {
            Some(&ESCAPE) | Some(&QUOTATION_MARK) => offset += 1,
            _ => {}
        }
        let next = &stream[offset..];
        offset += match next.find_slice((QUOTATION_MARK, ESCAPE, b'\n')) {
            Some(span) => {
                if next[span.start] == QUOTATION_MARK {
                    span.end
                } else {
                    span.start
                }
            }
            None => next.eof_offset(),
        };
    }

    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "`offset` was after ASCII");
    debug_assert_utf8!(slice, "`offset` was after ASCII");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };

    Token::new(TokenKind::BasicString, raw)
}

/// `quotation-mark = %x22            ; "`
pub(crate) const QUOTATION_MARK: u8 = b'"';

/// `escape = %x5C                   ; \`
pub(crate) const ESCAPE: u8 = b'\\';

/// Process multi-line basic string
///
/// ```bnf
/// ;; Multiline Basic String
///
/// ml-basic-string = ml-basic-string-delim [ newline ] ml-basic-body
///                   ml-basic-string-delim
/// ml-basic-string-delim = 3quotation-mark
/// ml-basic-body = *mlb-content *( mlb-quotes 1*mlb-content ) [ mlb-quotes ]
///
/// mlb-content = mlb-char / newline / mlb-escaped-nl
/// mlb-char = mlb-unescaped / escaped
/// mlb-quotes = 1*2quotation-mark
/// mlb-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
/// mlb-escaped-nl = escape ws newline *( wschar / newline )
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream.starts_with(ML_BASIC_STRING_DELIM)`
unsafe fn lex_ml_basic_string<'i>(stream: &mut &'i [u8]) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert_eq!(stream.get(0), Some(&QUOTATION_MARK));

    let mut offset = ML_BASIC_STRING_DELIM.len();
    let next = &stream[offset..];
    offset += match next.find_slice((ML_BASIC_STRING_DELIM, "\\")) {
        Some(span) => {
            if next[span.start] == QUOTATION_MARK {
                span.end
            } else {
                span.start
            }
        }
        None => next.eof_offset(),
    };
    while stream.get(offset) == Some(&ESCAPE) {
        offset += 1; // ESCAPE
        let peek = stream.get(offset);
        match peek {
            Some(&ESCAPE) | Some(&QUOTATION_MARK) => offset += 1,
            _ => {}
        }
        let next = &stream[offset..];
        offset += match next.find_slice((QUOTATION_MARK, ESCAPE, b'\n')) {
            Some(span) => {
                if next[span.start] == QUOTATION_MARK {
                    span.end
                } else {
                    span.start
                }
            }
            None => next.eof_offset(),
        };
    }
    if stream.get(offset) == Some(&QUOTATION_MARK) {
        offset += 1;
    }
    if stream.get(offset) == Some(&QUOTATION_MARK) {
        offset += 1;
    }

    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "`offset` was after ASCII");
    debug_assert_utf8!(slice, "`offset` was after ASCII");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };

    Token::new(TokenKind::MlBasicString, raw)
}

/// `ml-basic-string-delim = 3quotation-mark`
pub(crate) const ML_BASIC_STRING_DELIM: &str = "\"\"\"";

/// Process Atom
///
/// This is everything else
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream` must be non-empty
unsafe fn lex_atom<'i>(stream: &mut &'i [u8]) -> Token<'i> {
    debug_assert_utf8!(stream, "caller must start on `char` boundary");
    debug_assert!(!stream.is_empty());

    let mut offset = stream.eof_offset();
    for (i, b) in stream.iter().enumerate() {
        const TOKEN_START: &[u8] = b".=,[]{} \t#\r\n)'\"";
        if TOKEN_START.contains_token(b) {
            offset = i;
            break;
        }
    }

    let slice = stream.next_slice(offset);
    debug_assert_utf8!(stream, "`offset` was after ASCII");
    debug_assert_utf8!(slice, "`offset` was after ASCII");
    let raw = unsafe { std::str::from_utf8_unchecked(slice) };

    Token::new(TokenKind::Atom, raw)
}

#[cfg(test)]
mod test {
    use super::*;

    use snapbox::assert_data_eq;
    use snapbox::prelude::*;
    use snapbox::str;

    #[test]
    fn test_lex_ascii_char() {
        let cases = [(
            ".trailing",
            str![[r#"
Token {
    kind: Dot,
    raw: ".",
}

"#]],
            str!["trailing"],
        )];
        for (stream, expected_tokens, expected_stream) in cases {
            dbg!(stream);
            let mut stream = stream.as_bytes();
            let actual_tokens = unsafe { lex_ascii_char(&mut stream, TokenKind::Dot) };
            assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
            let stream = std::str::from_utf8(stream).unwrap();
            assert_data_eq!(stream, expected_stream.raw());
        }
    }

    #[test]
    fn test_lex_whitespace() {
        let cases = [
            (
                " ",
                str![[r#"
Token {
    kind: Whitespace,
    raw: " ",
}

"#]],
                str![],
            ),
            (
                " \t  \t  \t ",
                str![[r#"
Token {
    kind: Whitespace,
    raw: " \t  \t  \t ",
}

"#]],
                str![],
            ),
            (
                " \n",
                str![[r#"
Token {
    kind: Whitespace,
    raw: " ",
}

"#]],
                str![[r#"


"#]],
            ),
            (
                " #",
                str![[r#"
Token {
    kind: Whitespace,
    raw: " ",
}

"#]],
                str!["#"],
            ),
            (
                " a",
                str![[r#"
Token {
    kind: Whitespace,
    raw: " ",
}

"#]],
                str!["a"],
            ),
        ];
        for (stream, expected_tokens, expected_stream) in cases {
            dbg!(stream);
            let mut stream = stream.as_bytes();
            let actual_tokens = unsafe { lex_whitespace(&mut stream) };
            assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
            let stream = std::str::from_utf8(stream).unwrap();
            assert_data_eq!(stream, expected_stream.raw());
        }
    }

    #[test]
    fn test_lex_comment() {
        let cases = [
            (
                "#",
                str![[r##"
Token {
    kind: Comment,
    raw: "#",
}

"##]],
                str![""],
            ),
            (
                "# content",
                str![[r##"
Token {
    kind: Comment,
    raw: "# content",
}

"##]],
                str![""],
            ),
            (
                "# content \ntrailing",
                str![[r##"
Token {
    kind: Comment,
    raw: "# content ",
}

"##]],
                str![[r#"

trailing
"#]],
            ),
            (
                "# content \r\ntrailing",
                str![[r##"
Token {
    kind: Comment,
    raw: "# content ",
}

"##]],
                str![[r#"

trailing
"#]],
            ),
        ];
        for (stream, expected_tokens, expected_stream) in cases {
            dbg!(stream);
            let mut stream = stream.as_bytes();
            let actual_tokens = unsafe { lex_comment(&mut stream) };
            assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
            let stream = std::str::from_utf8(stream).unwrap();
            assert_data_eq!(stream, expected_stream.raw());
        }
    }

    #[test]
    fn test_lex_crlf() {
        let cases = [
            (
                "\r\ntrailing",
                str![[r#"
Token {
    kind: Newline,
    raw: "\r\n",
}

"#]],
                str!["trailing"],
            ),
            (
                "\rtrailing",
                str![[r#"
Token {
    kind: Newline,
    raw: "\r",
}

"#]],
                str!["trailing"],
            ),
        ];
        for (stream, expected_tokens, expected_stream) in cases {
            dbg!(stream);
            let mut stream = stream.as_bytes();
            let actual_tokens = unsafe { lex_crlf(&mut stream) };
            assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
            let stream = std::str::from_utf8(stream).unwrap();
            assert_data_eq!(stream, expected_stream.raw());
        }
    }

    #[test]
    fn test_lex_literal_string() {
        let cases = [
            (
                "''",
                str![[r#"
Token {
    kind: LiteralString,
    raw: "''",
}

"#]],
                str![""],
            ),
            (
                "''trailing",
                str![[r#"
Token {
    kind: LiteralString,
    raw: "''",
}

"#]],
                str!["trailing"],
            ),
            (
                "'content'trailing",
                str![[r#"
Token {
    kind: LiteralString,
    raw: "'content'",
}

"#]],
                str!["trailing"],
            ),
            (
                "'content",
                str![[r#"
Token {
    kind: LiteralString,
    raw: "'content",
}

"#]],
                str![""],
            ),
            (
                "'content\ntrailing",
                str![[r#"
Token {
    kind: LiteralString,
    raw: "'content",
}

"#]],
                str![[r#"

trailing
"#]],
            ),
        ];
        for (stream, expected_tokens, expected_stream) in cases {
            dbg!(stream);
            let mut stream = stream.as_bytes();
            let actual_tokens = unsafe { lex_literal_string(&mut stream) };
            assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
            let stream = std::str::from_utf8(stream).unwrap();
            assert_data_eq!(stream, expected_stream.raw());
        }
    }

    #[test]
    fn test_lex_ml_literal_string() {
        let cases = [
            (
                "''''''",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "''''''",
}

"#]],
                str![""],
            ),
            (
                "''''''trailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "''''''",
}

"#]],
                str!["trailing"],
            ),
            (
                "'''content'''trailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''content'''",
}

"#]],
                str!["trailing"],
            ),
            (
                "'''content",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''content",
}

"#]],
                str![""],
            ),
            (
                "'''content'",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''content'",
}

"#]],
                str![""],
            ),
            (
                "'''content''",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''content''",
}

"#]],
                str![""],
            ),
            (
                "'''content\ntrailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''content\ntrailing",
}

"#]],
                str![""],
            ),
            (
                "'''''''trailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''''''",
}

"#]],
                str!["trailing"],
            ),
            (
                "''''''''trailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "''''''''",
}

"#]],
                str!["trailing"],
            ),
            (
                "'''''''''trailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "''''''''",
}

"#]],
                str!["'trailing"],
            ),
            (
                "'''''content''''trailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''''content''''",
}

"#]],
                str!["trailing"],
            ),
            (
                "'''''content'''''trailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''''content'''''",
}

"#]],
                str!["trailing"],
            ),
            (
                "'''''content''''''trailing",
                str![[r#"
Token {
    kind: MlLiteralString,
    raw: "'''''content'''''",
}

"#]],
                str!["'trailing"],
            ),
        ];
        for (stream, expected_tokens, expected_stream) in cases {
            dbg!(stream);
            let mut stream = stream.as_bytes();
            let actual_tokens = unsafe { lex_ml_literal_string(&mut stream) };
            assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
            let stream = std::str::from_utf8(stream).unwrap();
            assert_data_eq!(stream, expected_stream.raw());
        }
    }

    #[test]
    fn test_lex_basic_string() {
        let cases = [
            (
                r#""""#,
                str![[r#"
Token {
    kind: BasicString,
    raw: "\"\"",
}

"#]],
                str![],
            ),
            (
                r#"""trailing"#,
                str![[r#"
Token {
    kind: BasicString,
    raw: "\"\"",
}

"#]],
                str!["trailing"],
            ),
            (
                r#""content"trailing"#,
                str![[r#"
Token {
    kind: BasicString,
    raw: "\"content\"",
}

"#]],
                str!["trailing"],
            ),
            (
                r#""content"#,
                str![[r#"
Token {
    kind: BasicString,
    raw: "\"content",
}

"#]],
                str![],
            ),
            (
                r#""content\ntrailing"#,
                str![[r#"
Token {
    kind: BasicString,
    raw: "\"content\\ntrailing",
}

"#]],
                str![],
            ),
        ];
        for (stream, expected_tokens, expected_stream) in cases {
            dbg!(stream);
            let mut stream = stream.as_bytes();
            let actual_tokens = unsafe { lex_basic_string(&mut stream) };
            assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
            let stream = std::str::from_utf8(stream).unwrap();
            assert_data_eq!(stream, expected_stream.raw());
        }
    }

    #[test]
    fn test_lex_atom() {
        let cases = [
            (
                "hello",
                str![[r#"
Token {
    kind: Atom,
    raw: "hello",
}

"#]],
                str![""],
            ),
            (
                "hello = world",
                str![[r#"
Token {
    kind: Atom,
    raw: "hello",
}

"#]],
                str![" = world"],
            ),
            (
                "1.100e100 ]",
                str![[r#"
Token {
    kind: Atom,
    raw: "1",
}

"#]],
                str![".100e100 ]"],
            ),
            (
                "a.b.c = 5",
                str![[r#"
Token {
    kind: Atom,
    raw: "a",
}

"#]],
                str![".b.c = 5"],
            ),
            (
                "true ]",
                str![[r#"
Token {
    kind: Atom,
    raw: "true",
}

"#]],
                str![" ]"],
            ),
        ];
        for (stream, expected_tokens, expected_stream) in cases {
            dbg!(stream);
            let mut stream = stream.as_bytes();
            let actual_tokens = unsafe { lex_atom(&mut stream) };
            assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
            let stream = std::str::from_utf8(stream).unwrap();
            assert_data_eq!(stream, expected_stream.raw());
        }
    }
}
