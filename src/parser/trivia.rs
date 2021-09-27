use nom::{
    branch::*, bytes::complete::*, character::complete::*, combinator::*, multi::*, sequence::*,
    AsChar, IResult,
};

// wschar = ( %x20 /              ; Space
//            %x09 )              ; Horizontal tab
#[inline]
pub(crate) fn is_wschar(c: impl AsChar) -> bool {
    let c = c.as_char();
    matches!(c, ' ' | '\t')
}

// ws = *wschar
pub(crate) fn ws(input: &str) -> IResult<&str, &str> {
    take_while(is_wschar)(input)
}

// non-ascii = %x80-D7FF / %xE000-10FFFF
#[inline]
pub(crate) fn is_non_ascii(c: impl AsChar) -> bool {
    let c = c.as_char();
    matches!(c, '\u{80}'..='\u{D7FF}' | '\u{E000}'..='\u{10FFFF}')
}

// non-eol = %x09 / %x20-7E / non-ascii
#[inline]
pub(crate) fn is_non_eol(c: impl AsChar) -> bool {
    let c = c.as_char();
    matches!(c, '\u{09}' | '\u{20}'..='\u{7E}') | is_non_ascii(c)
}

// comment-start-symbol = %x23 ; #
const COMMENT_START_SYMBOL: std::primitive::char = '#';

// comment = comment-start-symbol *non-eol
pub(crate) fn comment(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        map(char(COMMENT_START_SYMBOL), |_| ()),
        map(take_while(is_non_eol), |_| ()),
    )))(input)
}

// newline = ( %x0A /              ; LF
//             %x0D.0A )           ; CRLF
pub(crate) fn newline(input: &str) -> IResult<&str, std::primitive::char> {
    map(alt((tag("\n"), tag("\r\n"))), |_| '\n')(input)
}

// ws-newline       = *( wschar / newline )
pub(crate) fn ws_newline(input: &str) -> IResult<&str, &str> {
    recognize(many0_count(alt((
        map(newline, |_| "\n"),
        take_while1(is_wschar),
    ))))(input)
}

// ws-newlines      = newline *( wschar / newline )
pub(crate) fn ws_newlines(input: &str) -> IResult<&str, &str> {
    recognize(tuple((newline, ws_newlines)))(input)
}

// note: this rule is not present in the original grammar
// ws-comment-newline = *( ws-newline-nonempty / comment )
pub(crate) fn ws_comment_newline(input: &str) -> IResult<&str, &str> {
    recognize(many0_count(alt((
        many1_count(alt((take_while1(is_wschar), map(newline, |_| "\n")))),
        map(comment, |_| 0),
    ))))(input)
}

// note: this rule is not present in the original grammar
// line-ending = newline / eof
pub(crate) fn line_ending(input: &str) -> IResult<&str, &str> {
    alt((map(newline, |_| "\n"), map(eof, |_| "")))(input)
}

// note: this rule is not present in the original grammar
// line-trailing = ws [comment] skip-line-ending
pub(crate) fn line_trailing(input: &str) -> IResult<&str, &str> {
    map(
        tuple((recognize(tuple((ws, opt(comment)))), line_ending)),
        |t| t.1,
    )(input)
}
