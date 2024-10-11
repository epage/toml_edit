use winnow::stream::Stream as _;

use crate::lexer::Raw;
use crate::lexer::Token;
use crate::lexer::TokenKind;
use crate::ErrorSink;
use crate::Expected;
use crate::ParseError;

/// Parse lexed tokens into [`Event`]s
pub fn parse_tokens<'i>(
    mut tokens: &[Token<'i>],
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    document(&mut tokens, receiver, error);
}

/// Parse a TOML Document
///
/// Only the order of [`Event`]s is validated and not [`Event`] content nor semantics like duplicate
/// keys.
///
/// ```bnf
/// toml = expression *( newline expression )
///
/// expression =  ws [ comment ]
/// expression =/ ws keyval ws [ comment ]
/// expression =/ ws table ws [ comment ]
///
/// ;; Key-Value pairs
///
/// keyval = key keyval-sep val
///
/// key = simple-key / dotted-key
/// simple-key = quoted-key / unquoted-key
///
/// quoted-key = basic-string / literal-string
/// dotted-key = simple-key 1*( dot-sep simple-key )
///
/// dot-sep   = ws %x2E ws  ; . Period
/// keyval-sep = ws %x3D ws ; =
///
/// val = string / boolean / array / inline-table / date-time / float / integer
///
/// ;; Array
///
/// array = array-open [ array-values ] ws-comment-newline array-close
///
/// array-open =  %x5B ; [
/// array-close = %x5D ; ]
///
/// array-values =  ws-comment-newline val ws-comment-newline array-sep array-values
/// array-values =/ ws-comment-newline val ws-comment-newline [ array-sep ]
///
/// array-sep = %x2C  ; , Comma
///
/// ;; Table
///
/// table = std-table / array-table
///
/// ;; Standard Table
///
/// std-table = std-table-open key std-table-close
///
/// ;; Inline Table
///
/// inline-table = inline-table-open [ inline-table-keyvals ] inline-table-close
///
/// inline-table-keyvals = keyval [ inline-table-sep inline-table-keyvals ]
///
/// ;; Array Table
///
/// array-table = array-table-open key array-table-close
/// ```
fn document<'i>(
    tokens: &mut &[Token<'i>],
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    while let Some(current_token) = tokens.next_token() {
        match current_token.kind() {
            TokenKind::LeftSquareBracket => on_table(tokens, current_token, receiver, error),
            TokenKind::RightSquareBracket => {
                on_missing_on_std_table(tokens, current_token, receiver, error)
            }
            TokenKind::LiteralString => on_expression_key(
                tokens,
                current_token,
                StringKind::LiteralString,
                receiver,
                error,
            ),
            TokenKind::BasicString => on_expression_key(
                tokens,
                current_token,
                StringKind::BasicString,
                receiver,
                error,
            ),
            TokenKind::MlLiteralString => on_expression_key(
                tokens,
                current_token,
                StringKind::MlLiteralString,
                receiver,
                error,
            ),
            TokenKind::MlBasicString => on_expression_key(
                tokens,
                current_token,
                StringKind::MlBasicString,
                receiver,
                error,
            ),
            TokenKind::Atom => {
                on_expression_key(tokens, current_token, StringKind::Unquoted, receiver, error)
            }
            TokenKind::Dot
            | TokenKind::Equals
            | TokenKind::Comma
            | TokenKind::RightCurlyBracket
            | TokenKind::LeftCurlyBracket => {
                on_missing_expression_key(tokens, current_token, receiver, error)
            }
            TokenKind::Whitespace | TokenKind::Newline => on_decor(current_token, receiver),
            TokenKind::Comment => on_comment(tokens, current_token, receiver, error),
        }
    }
}

/// Start a table from the open token
///
/// This eats to EOL
///
/// ```bnf
/// ;; Table
///
/// table = std-table / array-table
///
/// ;; Standard Table
///
/// std-table = std-table-open key std-table-close
///
/// ;; Array Table
///
/// array-table = array-table-open key array-table-close
/// ```
fn on_table<'i>(
    tokens: &mut &[Token<'i>],
    open_token: Token<'i>,
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    let (is_array_table, open_raw) =
        if let Some(second_open_token) = next_token_if(tokens, TokenKind::LeftSquareBracket) {
            let raw = unsafe { open_token.raw().append(second_open_token.raw()) };
            receiver.array_table_open(raw);
            let is_array_table = true;
            (is_array_table, raw)
        } else {
            let raw = open_token.raw();
            receiver.std_table_open(raw);
            let is_array_table = false;
            (is_array_table, raw)
        };

    let last_key_token = table_key(tokens, open_raw, receiver, error);

    opt_whitespace(tokens, receiver);

    let mut success = false;
    if let Some(last_key_token) = last_key_token {
        if let Some(close_token) = next_token_if(tokens, TokenKind::RightSquareBracket) {
            if is_array_table {
                if let Some(second_close_token) =
                    next_token_if(tokens, TokenKind::RightSquareBracket)
                {
                    let raw = unsafe { close_token.raw().append(second_close_token.raw()) };
                    receiver.array_table_close(raw);
                    success = true;
                } else {
                    let context = unsafe { open_token.raw().append(close_token.raw()) };
                    error.report_error(ParseError {
                        context,
                        description: "array table",
                        expected: &[Expected::Literal("]")],
                        unexpected: close_token.raw().after(),
                    });
                }
            } else {
                receiver.std_table_close(close_token.raw());
                success = true;
            }
        } else {
            let context = unsafe { open_token.raw().append(last_key_token.raw()) };
            if is_array_table {
                error.report_error(ParseError {
                    context,
                    description: "array table",
                    expected: &[Expected::Literal("]]")],
                    unexpected: last_key_token.raw().after(),
                });
            } else {
                error.report_error(ParseError {
                    context,
                    description: "table",
                    expected: &[Expected::Literal("]")],
                    unexpected: last_key_token.raw().after(),
                });
            }
        }
    }

    if success {
        ws_comment_nl(tokens, receiver, error);
    } else {
        ignore_to_newline(tokens, receiver);
    }
}

/// Start an expression from a key compatible token  type
fn on_expression_key<'i>(
    tokens: &mut &[Token<'i>],
    key_token: Token<'i>,
    kind: StringKind,
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    if on_key(tokens, key_token, kind, receiver, error).is_none() {
        ignore_to_newline(tokens, receiver);
        return;
    }
}

/// Parse table header keys
///
/// This eats the leading whitespace
fn table_key<'i>(
    tokens: &mut &[Token<'i>],
    previous_raw: Raw<'i>,
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) -> Option<Token<'i>> {
    while let Some(current_token) = tokens.next_token() {
        let kind = match current_token.kind() {
            TokenKind::Dot
            | TokenKind::RightSquareBracket
            | TokenKind::Comment
            | TokenKind::Equals
            | TokenKind::Comma
            | TokenKind::LeftSquareBracket
            | TokenKind::LeftCurlyBracket
            | TokenKind::RightCurlyBracket
            | TokenKind::Newline => {
                on_missing_table_key(current_token, receiver, error);
                return None;
            }
            TokenKind::LiteralString => StringKind::LiteralString,
            TokenKind::BasicString => StringKind::BasicString,
            TokenKind::MlLiteralString => StringKind::MlLiteralString,
            TokenKind::MlBasicString => StringKind::MlBasicString,
            TokenKind::Atom => StringKind::Unquoted,
            TokenKind::Whitespace => {
                on_decor(current_token, receiver);
                continue;
            }
        };
        let success = on_key(tokens, current_token, kind, receiver, error);
        return success;
    }

    error.report_error(ParseError {
        context: previous_raw,
        description: "table",
        expected: &[Expected::Description("key")],
        unexpected: previous_raw.after(),
    });
    None
}

/// Start a key from the first key compatible token type
///
/// Returns the last key on success
///
/// This will swallow the trailing [`TokenKind::Whitespace`]
fn on_key<'i>(
    tokens: &mut &[Token<'i>],
    key_token: Token<'i>,
    kind: StringKind,
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) -> Option<Token<'i>> {
    receiver.simple_key(key_token.raw(), kind);

    opt_whitespace(tokens, receiver);

    let mut success = Some(key_token);
    while let Some(dot_token) = next_token_if(tokens, TokenKind::Dot) {
        receiver.key_sep(dot_token.raw());

        opt_whitespace(tokens, receiver);

        if let Some(current_token) = tokens.next_token() {
            let kind = match current_token.kind() {
                TokenKind::Dot
                | TokenKind::Equals
                | TokenKind::Comma
                | TokenKind::LeftSquareBracket
                | TokenKind::RightSquareBracket
                | TokenKind::LeftCurlyBracket
                | TokenKind::RightCurlyBracket
                | TokenKind::Comment
                | TokenKind::Whitespace
                | TokenKind::Newline => {
                    receiver.error(current_token.raw());
                    let context = unsafe { key_token.raw().append(dot_token.raw()) };
                    error.report_error(ParseError {
                        context,
                        description: "dotted key",
                        expected: &[Expected::Description("key")],
                        unexpected: current_token.raw().before(),
                    });
                    success = None;
                    break;
                }
                TokenKind::LiteralString => StringKind::LiteralString,
                TokenKind::BasicString => StringKind::BasicString,
                TokenKind::MlLiteralString => StringKind::MlLiteralString,
                TokenKind::MlBasicString => StringKind::MlBasicString,
                TokenKind::Atom => StringKind::Unquoted,
            };
            debug_assert!(
                success.is_some(),
                "unconditionally overwriting due to the assumption its always in the success case"
            );
            success = Some(key_token);
            receiver.simple_key(key_token.raw(), kind);
        } else {
            let context = unsafe { key_token.raw().append(dot_token.raw()) };
            error.report_error(ParseError {
                context,
                description: "dotted key",
                expected: &[Expected::Description("key")],
                unexpected: dot_token.raw().after(),
            });
            success = None;
            break;
        }
    }

    success
}

/// Start decor from a decor token
fn on_decor<'i>(decor_token: Token<'i>, receiver: &mut dyn EventReceiver<'i>) {
    receiver.decor(decor_token.raw());
}

/// Parse whitespace, if present
fn opt_whitespace<'i>(tokens: &mut &[Token<'i>], receiver: &mut dyn EventReceiver<'i>) {
    if let Some(ws_token) = next_token_if(tokens, TokenKind::Whitespace) {
        on_decor(ws_token, receiver);
    }
}

/// Parse EOL decor, if present
///
/// ```bnf
/// toml = expression *( newline expression )
///
/// expression =  ws [ on_comment ]
/// expression =/ ws keyval ws [ on_comment ]
/// expression =/ ws table ws [ on_comment ]
///
/// ;; Comment
///
/// comment = comment-start-symbol *non-eol
///
/// ;; Array
///
/// ws-comment-newline = *( wschar / [ comment ] newline )
/// ```
fn ws_comment_nl<'i>(
    tokens: &mut &[Token<'i>],
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    let mut first = None;
    let mut last = None;
    let mut first_bad = None;
    let mut last_bad = None;
    while let Some(current_token) = tokens.next_token() {
        first.get_or_insert(current_token);
        last = Some(current_token);
        match current_token.kind() {
            TokenKind::Dot
            | TokenKind::Equals
            | TokenKind::Comma
            | TokenKind::LeftSquareBracket
            | TokenKind::RightSquareBracket
            | TokenKind::LeftCurlyBracket
            | TokenKind::RightCurlyBracket
            | TokenKind::LiteralString
            | TokenKind::BasicString
            | TokenKind::MlLiteralString
            | TokenKind::MlBasicString
            | TokenKind::Atom => {
                first_bad.get_or_insert(current_token);
                last_bad = Some(current_token);
                receiver.error(current_token.raw());
            }
            TokenKind::Comment => {
                on_comment(tokens, current_token, receiver, error);
                break;
            }
            TokenKind::Whitespace => {
                on_decor(current_token, receiver);
                continue;
            }
            TokenKind::Newline => {
                on_decor(current_token, receiver);
                break;
            }
        }
    }
    if let (Some(first), Some(last), Some(first_bad), Some(last_bad)) =
        (first, last, first_bad, last_bad)
    {
        let context = unsafe { first.raw().append(last.raw()) };
        let bad = unsafe { first_bad.raw().append(last_bad.raw()) };
        error.report_error(ParseError {
            context,
            description: "newline",
            expected: &[],
            unexpected: bad,
        });
    }
}

/// Start EOL from [`TokenKind::Comment`]
fn on_comment<'i>(
    tokens: &mut &[Token<'i>],
    comment_token: Token<'i>,
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    on_decor(comment_token, receiver);
    let mut first = None;
    let mut last = None;
    let mut first_bad = None;
    let mut last_bad = None;
    while let Some(current_token) = tokens.next_token() {
        first.get_or_insert(current_token);
        last = Some(current_token);
        match current_token.kind() {
            TokenKind::Dot
            | TokenKind::Equals
            | TokenKind::Comma
            | TokenKind::LeftSquareBracket
            | TokenKind::RightSquareBracket
            | TokenKind::LeftCurlyBracket
            | TokenKind::RightCurlyBracket
            | TokenKind::Whitespace
            | TokenKind::Comment
            | TokenKind::LiteralString
            | TokenKind::BasicString
            | TokenKind::MlLiteralString
            | TokenKind::MlBasicString
            | TokenKind::Atom => {
                first_bad.get_or_insert(current_token);
                last_bad = Some(current_token);
                receiver.error(current_token.raw());
            }
            TokenKind::Newline => {
                on_decor(current_token, receiver);
                break;
            }
        }
    }
    if let (Some(first_bad), Some(last_bad)) = (first_bad, last_bad) {
        let bad = unsafe { first_bad.raw().append(last_bad.raw()) };
        error.report_error(ParseError {
            context: comment_token.raw(),
            description: "comment",
            expected: &[],
            unexpected: bad,
        });
    }
    if let (Some(first), Some(last), Some(first_bad), Some(last_bad)) =
        (first, last, first_bad, last_bad)
    {
        let context = unsafe { first.raw().append(last.raw()) };
        let bad = unsafe { first_bad.raw().append(last_bad.raw()) };
        error.report_error(ParseError {
            context,
            description: "comment",
            expected: &[],
            unexpected: bad,
        });
    }
}

// Don't bother recovering until [`TokenKind::Newline`]
#[cold]
fn ignore_to_newline<'i>(tokens: &mut &[Token<'i>], receiver: &mut dyn EventReceiver<'i>) {
    while let Some(current_token) = tokens.next_token() {
        if matches!(current_token.kind(), TokenKind::Newline) {
            on_decor(current_token, receiver);
            break;
        } else {
            receiver.error(current_token.raw());
        }
    }
}

#[cold]
fn on_missing_table_key<'i>(
    token: Token<'i>,
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    receiver.error(token.raw());
    error.report_error(ParseError {
        context: token.raw(),
        description: "table",
        expected: &[Expected::Description("key")],
        unexpected: token.raw().before(),
    });
}

#[cold]
fn on_missing_expression_key<'i>(
    tokens: &mut &[Token<'i>],
    token: Token<'i>,
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    receiver.error(token.raw());
    error.report_error(ParseError {
        context: token.raw(),
        description: "key-value pair",
        expected: &[Expected::Description("key")],
        unexpected: token.raw().before(),
    });
    ignore_to_newline(tokens, receiver);
}

#[cold]
fn on_missing_on_std_table<'i>(
    tokens: &mut &[Token<'i>],
    token: Token<'i>,
    receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    receiver.error(token.raw());
    error.report_error(ParseError {
        context: token.raw(),
        description: "table",
        expected: &[Expected::Literal("[")],
        unexpected: token.raw().before(),
    });
    ws_comment_nl(tokens, receiver, error);
}

fn next_token_if<'i>(tokens: &mut &[Token<'i>], kind: TokenKind) -> Option<Token<'i>> {
    match tokens.first() {
        Some(next) if next.kind() == kind => {
            let _ = tokens.next_token();
            Some(*next)
        }
        _ => None,
    }
}

pub trait EventReceiver<'i> {
    fn std_table_open(&mut self, raw: Raw<'i>);
    fn std_table_close(&mut self, raw: Raw<'i>);
    fn array_table_open(&mut self, raw: Raw<'i>);
    fn array_table_close(&mut self, raw: Raw<'i>);
    fn inline_table_open(&mut self, raw: Raw<'i>);
    fn inline_table_close(&mut self, raw: Raw<'i>);
    fn array_open(&mut self, raw: Raw<'i>);
    fn array_close(&mut self, raw: Raw<'i>);
    fn simple_key(&mut self, raw: Raw<'i>, kind: StringKind);
    fn key_sep(&mut self, raw: Raw<'i>);
    fn key_val_sep(&mut self, raw: Raw<'i>);
    fn value(&mut self, raw: Raw<'i>, kind: StringKind);
    fn value_sep(&mut self, raw: Raw<'i>);
    fn decor(&mut self, raw: Raw<'i>);
    fn error(&mut self, raw: Raw<'i>);
}

impl<'i> EventReceiver<'i> for dyn FnMut(Event<'i>) {
    fn std_table_open(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::StdTableOpen,
            raw,
        });
    }
    fn std_table_close(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::StdTableClose,
            raw,
        });
    }
    fn array_table_open(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::ArrayTableOpen,
            raw,
        });
    }
    fn array_table_close(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::ArrayTableClose,
            raw,
        });
    }
    fn inline_table_open(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::InlineTableOpen,
            raw,
        });
    }
    fn inline_table_close(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::InlineTableClose,
            raw,
        });
    }
    fn array_open(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::ArrayOpen,
            raw,
        });
    }
    fn array_close(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::ArrayClose,
            raw,
        });
    }
    fn simple_key(&mut self, raw: Raw<'i>, kind: StringKind) {
        (self)(Event {
            kind: EventKind::SimpleKey(kind),
            raw,
        });
    }
    fn key_sep(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::KeySep,
            raw,
        });
    }
    fn key_val_sep(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::KeyValSep,
            raw,
        });
    }
    fn value(&mut self, raw: Raw<'i>, kind: StringKind) {
        (self)(Event {
            kind: EventKind::Value(kind),
            raw,
        });
    }
    fn value_sep(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::ValueSep,
            raw,
        });
    }
    fn decor(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::Decor,
            raw,
        });
    }
    fn error(&mut self, raw: Raw<'i>) {
        (self)(Event {
            kind: EventKind::Error,
            raw,
        });
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Event<'i> {
    kind: EventKind,
    raw: Raw<'i>,
}

impl<'i> Event<'i> {
    #[inline(always)]
    pub fn kind(&self) -> EventKind {
        self.kind
    }

    #[inline(always)]
    pub fn raw(&self) -> Raw<'i> {
        self.raw
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EventKind {
    StdTableOpen,
    StdTableClose,
    ArrayTableOpen,
    ArrayTableClose,
    InlineTableOpen,
    InlineTableClose,
    ArrayOpen,
    ArrayClose,
    SimpleKey(StringKind),
    KeySep,
    KeyValSep,
    Value(StringKind),
    ValueSep,
    Decor,
    Error,
}

impl EventKind {
    pub fn description(&self) -> &'static str {
        match self {
            EventKind::StdTableOpen => "std-table open",
            EventKind::StdTableClose => "std-table close",
            EventKind::ArrayTableOpen => "array-table open",
            EventKind::ArrayTableClose => "array-table close",
            EventKind::InlineTableOpen => "inline-table open",
            EventKind::InlineTableClose => "inline-table close",
            EventKind::ArrayOpen => "array open",
            EventKind::ArrayClose => "array close",
            EventKind::SimpleKey(_) => "key",
            EventKind::KeySep => "key separator",
            EventKind::KeyValSep => "key-value separator",
            EventKind::Value(_) => "value",
            EventKind::ValueSep => "value separator",
            EventKind::Decor => "decor",
            EventKind::Error => "error",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StringKind {
    LiteralString,
    BasicString,
    MlLiteralString,
    MlBasicString,
    Unquoted,
}

impl StringKind {
    pub fn description(&self) -> &'static str {
        match self {
            StringKind::LiteralString => TokenKind::LiteralString.description(),
            StringKind::BasicString => TokenKind::BasicString.description(),
            StringKind::MlLiteralString => TokenKind::MlLiteralString.description(),
            StringKind::MlBasicString => TokenKind::MlBasicString.description(),
            StringKind::Unquoted => "unquoted string",
        }
    }
}
