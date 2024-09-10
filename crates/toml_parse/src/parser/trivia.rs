use std::ops::RangeInclusive;

use winnow::prelude::*;
use winnow::stream::ContainsToken as _;
use winnow::stream::Stream as _;

use crate::lexer::Raw;
use crate::lexer::TokenKind;
use crate::parser::substr_at;
use crate::parser::BStrInput;
use crate::parser::Error;
use crate::ErrorSink;
use crate::Expected;
use crate::ParseError;

pub(crate) use crate::lexer::COMMENT_START_SYMBOL;
pub(crate) use crate::lexer::WSCHAR;

/// Parse Whitespace
///
/// ```bnf
/// ;; Whitespace
///
/// ws = *wschar
/// wschar =  %x20  ; Space
/// wschar =/ %x09  ; Horizontal tab
/// ```
#[inline(always)]
pub fn parse_whitespace<'i, ES: ErrorSink<'i>>(raw: Raw<'i>, _error: &mut ES) -> &'i str {
    // Handled in `lex_whitespace`
    raw.as_str()
}

/// Parse Comment
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
pub fn parse_comment<'i, ES: ErrorSink<'i>>(raw: Raw<'i>, error: &mut ES) -> &'i str {
    let rest = raw.as_str().as_bytes();
    let rest = if let Some((&COMMENT_START_SYMBOL, rest)) = rest.split_first() {
        rest
    } else {
        error.report_error(ParseError {
            context: raw,
            description: TokenKind::Comment.description(),
            expected: &[Expected::Literal("#")],
            unexpected: raw.before(),
        });
        rest
    };

    for (i, b) in rest.iter().enumerate() {
        if !NON_EOL.contains_token(b) {
            let offset = i + 1; // COMMENT_START_SYMBOL
            error.report_error(ParseError {
                context: raw,
                description: TokenKind::Comment.description(),
                expected: &[],
                unexpected: Raw::new_unchecked(substr_at(raw.as_str(), offset)),
            });
        }
    }

    raw.as_str()
}

/// `non-ascii = %x80-D7FF / %xE000-10FFFF`
/// - ASCII is 0xxxxxxx
/// - First byte for UTF-8 is 11xxxxxx
/// - Subsequent UTF-8 bytes are 10xxxxxx
pub(crate) const NON_ASCII: RangeInclusive<u8> = 0x80..=0xff;

// non-eol = %x09 / %x20-7E / non-ascii
pub(crate) const NON_EOL: (u8, RangeInclusive<u8>, RangeInclusive<u8>) =
    (0x09, 0x20..=0x7E, NON_ASCII);

/// Parse Newline
///
/// ```bnf
/// ;; Newline
///
/// newline =  %x0A     ; LF
/// newline =/ %x0D.0A  ; CRLF
/// ```
pub fn parse_newline<'i, ES: ErrorSink<'i>>(raw: Raw<'i>, error: &mut ES) -> &'i str {
    match raw.as_str() {
        "\n" | "\r\n" => {}
        "\r" => {
            error.report_error(ParseError {
                context: raw,
                description: TokenKind::Newline.description(),
                expected: &[Expected::Description("linefeed (`\\n')")],
                unexpected: raw.after(),
            });
        }
        _ => {
            error.report_error(ParseError {
                context: raw,
                description: TokenKind::Newline.description(),
                expected: &[Expected::Description("linefeed (`\\n')")],
                unexpected: raw,
            });
        }
    }
    raw.as_str()
}

pub(super) fn newline<'i, 'e, ES: ErrorSink<'i>>(
    input: &mut BStrInput<'i, 'e, ES>,
) -> PResult<&'i str, Error> {
    let s = match input.input.first() {
        Some(b'\n') => input.next_slice(1),
        Some(b'\r') => match input.input.get(1) {
            Some(b'\n') => input.next_slice(2),
            Some(_) | None => {
                let unexpected = &input.input[1..1];
                debug_assert_utf8!(unexpected, "`newline` matches ASCII `char`s");
                let unexpected =
                    Raw::new_unchecked(unsafe { std::str::from_utf8_unchecked(unexpected) });
                input
                    .state
                    .report_error(&[Expected::Description("linefeed (`\\n')")], unexpected);
                input.next_slice(1)
            }
        },
        Some(_) | None => {
            return Err(winnow::error::ErrMode::Backtrack(()));
        }
    };
    debug_assert_utf8!(input.input, "`newline` matches ASCII `char`s");

    debug_assert_utf8!(s, "`newline` matches ASCII `char`s");
    let s = unsafe { std::str::from_utf8_unchecked(s) };

    Ok(s)
}
