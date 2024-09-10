use winnow::stream::BStr;
use winnow::stream::ContainsToken;
use winnow::PResult;
use winnow::Parser;

use crate::lexer::Raw;
use crate::lexer::Token;
use crate::lexer::TokenKind;
use crate::ErrorSink;

mod key;
mod strings;
mod trivia;

pub use key::*;
pub use strings::*;
pub use trivia::*;

/// `char`-boundary aligned byte parse stream with error recovery
///
/// **warning:** `char`-boundary alignment is by convention and should be asserted by
/// [`debug_assert_utf8!`].
type BStrInput<'i, 'e, ES> = winnow::stream::Stateful<&'i BStr, State<'i, 'e, ES>>;
type TokenInput<'t, 'i, 'e, ES> = winnow::stream::Stateful<&'t [Token<'i>], State<'i, 'e, ES>>;
/// See instead [`State::report_error`]
type Error = ();

#[derive(Debug)]
struct State<'i, 'e, ES> {
    /// For error recovery
    error: &'e mut ES,
    /// See [`ParserError::context`]
    context: Raw<'i>,
    /// See [`ParserError::description`]
    description: &'static str,
}

fn substr_at(raw: &str, offset: usize) -> &str {
    debug_assert!(offset < raw.len());
    let start = (0..=offset)
        .rev()
        .find(|i| raw.is_char_boundary(*i))
        .unwrap_or(0);
    let end = (offset + 1..raw.len())
        .find(|i| raw.is_char_boundary(*i))
        .unwrap_or(raw.len());
    &raw[start..end]
}

impl<'i, 'e, ES: ErrorSink<'i>> State<'i, 'e, ES> {
    fn report_error(&mut self, expected: &'static [crate::error::Expected], unexpected: Raw<'i>) {
        self.error.report_error(crate::error::ParseError {
            context: self.context,
            description: self.description,
            expected,
            unexpected,
        });
    }
}

#[doc(hidden)]
impl<'t, 'e, 'i, ES: ErrorSink<'i>> Parser<TokenInput<'t, 'i, 'e, ES>, Token<'i>, Error>
    for TokenKind
{
    fn parse_next(&mut self, input: &mut TokenInput<'t, 'i, 'e, ES>) -> PResult<Token<'i>, Error> {
        winnow::token::any
            .verify(|t: &Token<'i>| t.kind() == *self)
            .parse_next(input)
    }
}

#[doc(hidden)]
impl<'i> ContainsToken<Token<'i>> for TokenKind {
    #[inline(always)]
    fn contains_token(&self, token: Token<'i>) -> bool {
        *self == token.kind()
    }
}

#[doc(hidden)]
impl<'i> ContainsToken<Token<'i>> for &'_ [TokenKind] {
    #[inline(always)]
    fn contains_token(&self, token: Token<'i>) -> bool {
        let kind = token.kind();
        self.contains(&kind)
    }
}

#[doc(hidden)]
impl<'i, const LEN: usize> ContainsToken<Token<'i>> for &'_ [TokenKind; LEN] {
    #[inline(always)]
    fn contains_token(&self, token: Token<'i>) -> bool {
        let kind = token.kind();
        self.contains(&kind)
    }
}

#[doc(hidden)]
impl<'i, const LEN: usize> ContainsToken<Token<'i>> for [TokenKind; LEN] {
    #[inline(always)]
    fn contains_token(&self, token: Token<'i>) -> bool {
        let kind = token.kind();
        self.contains(&kind)
    }
}
