//! Lex TOML tokens

use super::APOSTROPHE;
use super::COMMENT_START_SYMBOL;
use super::QUOTATION_MARK;
use super::WSCHAR;

/// An unvalidated TOML Token
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Token<'i> {
    pub(super) kind: TokenKind,
    pub(super) raw: Raw<'i>,
}

impl<'i> Token<'i> {
    pub(super) fn new(kind: TokenKind, raw: &'i str) -> Self {
        Self {
            kind,
            raw: Raw::new_unchecked(raw),
        }
    }

    #[inline(always)]
    pub fn kind(&self) -> TokenKind {
        self.kind
    }

    #[inline(always)]
    pub fn raw(&self) -> Raw<'i> {
        self.raw
    }

    pub fn to_error(self, expected: &'static [crate::Expected]) -> crate::ParseError<'i> {
        crate::ParseError {
            context: self.raw(),
            description: self.kind().description(),
            expected,
            unexpected: self.raw(),
        }
    }
}

impl<'i> std::fmt::Display for Token<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.raw.fmt(f)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum TokenKind {
    /// Either for dotted-key or float
    Dot = b'.',
    /// Key-value separator
    Equals = b'=',
    /// Value separator
    Comma = b',',
    /// Either array or standard-table start
    LeftSquareBracket = b'[',
    /// Either array or standard-table end
    RightSquareBracket = b']',
    /// Inline table start
    LeftCurlyBracket = b'{',
    /// Inline table end
    RightCurlyBracket = b'}',
    Whitespace = WSCHAR.0,
    Comment = COMMENT_START_SYMBOL,
    Newline = b'\n',
    LiteralString = APOSTROPHE,
    BasicString = QUOTATION_MARK,
    MlLiteralString = 1,
    MlBasicString,

    /// Anything else
    Atom,
}

impl TokenKind {
    pub fn description(&self) -> &'static str {
        match self {
            TokenKind::Dot => "`.`",
            TokenKind::Equals => "`=`",
            TokenKind::Comma => "`,`",
            TokenKind::LeftSquareBracket => "`[`",
            TokenKind::RightSquareBracket => "`]`",
            TokenKind::LeftCurlyBracket => "`{`",
            TokenKind::RightCurlyBracket => "`}`",
            TokenKind::Whitespace => "whitedpace",
            TokenKind::Comment => "comment",
            TokenKind::Newline => "newline",
            TokenKind::LiteralString => "literal string",
            TokenKind::BasicString => "basic string",
            TokenKind::MlLiteralString => "multi-line literal string",
            TokenKind::MlBasicString => "multi-line basic string",
            TokenKind::Atom => "token",
        }
    }
}

/// Unparsed TOML text
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Raw<'i> {
    pub(crate) inner: &'i str,
}

impl<'i> Raw<'i> {
    pub(crate) fn new_unchecked(inner: &'i str) -> Self {
        Self { inner }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn before(&self) -> Self {
        Self {
            inner: &self.inner[0..0],
        }
    }

    pub fn after(&self) -> Self {
        let len = self.len();
        Self {
            inner: &self.inner[len..len],
        }
    }

    pub fn as_str(&self) -> &'i str {
        self.inner
    }

    /// Extend this `Raw` to the end of `after`
    ///
    /// # Safety
    ///
    /// `after` must come from the same `&str`
    pub(crate) unsafe fn append(&self, after: Self) -> Self {
        let start = self.inner.as_ptr();
        let end = after.inner.as_ptr() as usize + after.inner.len();
        debug_assert!(start as usize <= end, "arguments swapped");
        let len = end - start as usize;
        // SAFETY: callers must ensure these come from same `&str`
        let slice = unsafe { std::slice::from_raw_parts(start, len) };
        // SAFETY: if these come from the same `&str` then they've already been validated
        let new_inner = unsafe { std::str::from_utf8_unchecked(slice) };
        Self::new_unchecked(new_inner)
    }
}

impl<'i> std::fmt::Debug for Raw<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<'i> std::fmt::Display for Raw<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
