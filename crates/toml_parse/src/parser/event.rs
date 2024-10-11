use winnow::stream::Stream as _;

use crate::lexer::Raw;
use crate::lexer::Token;
use crate::lexer::TokenKind;
use crate::ErrorSink;

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
    _receiver: &mut dyn EventReceiver<'i>,
    error: &mut dyn ErrorSink<'i>,
) {
    while let Some(token) = tokens.next_token() {
        error.report_error(token.to_error(&[]))
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
