use std::borrow::Cow;
use std::ops::RangeInclusive;

use winnow::combinator::opt;
use winnow::combinator::repeat;
use winnow::combinator::trace;
use winnow::error::ErrMode;
use winnow::prelude::*;
use winnow::stream::BStr;
use winnow::stream::ContainsToken as _;
use winnow::stream::Stream as _;
use winnow::token::take_while;

use crate::lexer::Raw;
use crate::lexer::TokenKind;
use crate::lexer::APOSTROPHE;
use crate::lexer::ML_BASIC_STRING_DELIM;
use crate::lexer::ML_LITERAL_STRING_DELIM;
use crate::lexer::QUOTATION_MARK;
use crate::parser::newline;
use crate::parser::substr_at;
use crate::parser::BStrInput;
use crate::parser::Error;
use crate::parser::State;
use crate::parser::NON_ASCII;
use crate::parser::WSCHAR;
use crate::ErrorSink;
use crate::Expected;

/// Parse literal string
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
pub fn parse_literal_string<'i, ES: ErrorSink<'i>>(raw: Raw<'i>, error: &mut ES) -> &'i str {
    let mut state = State {
        error,
        context: raw,
        description: TokenKind::LiteralString.description(),
    };

    let s = raw.as_str();
    let s = if let Some(stripped) = s.strip_prefix(APOSTROPHE as char) {
        stripped
    } else {
        state.report_error(&[Expected::Literal("'")], raw.before());
        s
    };
    let s = if let Some(stripped) = s.strip_suffix(APOSTROPHE as char) {
        stripped
    } else {
        state.report_error(&[Expected::Literal("'")], raw.after());
        s
    };

    for (i, b) in s.as_bytes().iter().enumerate() {
        if !LITERAL_CHAR.contains_token(b) {
            let unexpected = Raw::new_unchecked(substr_at(s, i));
            state.report_error(&[], unexpected);
        }
    }

    s
}

/// `literal-char = %x09 / %x20-26 / %x28-7E / non-ascii`
pub(crate) const LITERAL_CHAR: (
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (0x9, 0x20..=0x26, 0x28..=0x7E, NON_ASCII);

/// Parse multi-line literal string
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
/// mll-quotes = 1*2apostrophe
/// ```
pub fn parse_ml_literal_string<'i, ES: ErrorSink<'i>>(raw: Raw<'i>, error: &mut ES) -> &'i str {
    let mut state = State {
        error,
        context: raw,
        description: TokenKind::MlLiteralString.description(),
    };

    let s = raw.as_str();
    let s = if let Some(stripped) = s.strip_prefix(ML_LITERAL_STRING_DELIM) {
        stripped
    } else {
        state.report_error(&[Expected::Literal("'")], raw.before());
        s
    };
    let s = strip_start_newline(s);
    let s = if let Some(stripped) = s.strip_suffix(ML_LITERAL_STRING_DELIM) {
        stripped
    } else {
        state.report_error(&[Expected::Literal("'")], raw.after());
        s.trim_end_matches('\'')
    };

    for (i, b) in s.as_bytes().iter().enumerate() {
        if *b == b'\'' || *b == b'\n' {
        } else if *b == b'\r' {
            if s.as_bytes().get(i + 1) != Some(&b'\n') {
                let unexpected = Raw::new_unchecked(substr_at(s, i + 1));
                state.report_error(&[Expected::Description("`\n`")], unexpected);
            }
        } else if !MLL_CHAR.contains_token(b) {
            let unexpected = Raw::new_unchecked(substr_at(s, i));
            state.report_error(&[], unexpected);
        }
    }

    s
}

/// `mll-char = %x09 / %x20-26 / %x28-7E / non-ascii`
const MLL_CHAR: (
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (0x9, 0x20..=0x26, 0x28..=0x7E, NON_ASCII);

/// Parse basic string
///
/// ```bnf
/// ;; Basic String
///
/// basic-string = quotation-mark *basic-char quotation-mark
/// ```
pub fn parse_basic_string<'i, ES: ErrorSink<'i>>(raw: Raw<'i>, error: &mut ES) -> Cow<'i, str> {
    let mut state = State {
        error,
        context: raw,
        description: TokenKind::BasicString.description(),
    };

    let s = raw.as_str();
    let s = if let Some(stripped) = s.strip_prefix(QUOTATION_MARK as char) {
        stripped
    } else {
        state.report_error(&[Expected::Literal("\"")], raw.before());
        s
    };
    let s = if let Some(stripped) = s.strip_suffix(QUOTATION_MARK as char) {
        stripped
    } else {
        state.report_error(&[Expected::Literal("\"")], raw.after());
        s
    };

    let mut input = BStrInput {
        input: BStr::new(s),
        state,
    };
    let c = match basic_char(&mut input) {
        Ok(c) => {
            debug_assert!(input.is_empty(), "remaining content: {:?}", input);
            c
        }
        Err(_) => {
            #[cfg(debug_assertions)]
            unreachable!(
                "errors should be reported deeper in the parser where more context exists"
            );
            #[cfg_attr(debug_assertions, allow(unreachable_code))]
            Cow::Borrowed(s)
        }
    };

    c
}

/// `basic-char = basic-unescaped / escaped`
///
/// # Safety
///
/// - `stream` must be UTF-8
fn basic_char<'i, 'e, ES: ErrorSink<'i>>(
    input: &mut BStrInput<'i, 'e, ES>,
) -> PResult<Cow<'i, str>, Error> {
    trace("basic-char", |input: &mut BStrInput<'i, 'e, ES>| {
        debug_assert_utf8!(input.input, "caller must start on `char` boundary");

        let s = basic_unescaped(input)?;
        let mut basic_chars = Cow::Borrowed(s);
        while !input.input.is_empty() {
            let c = escaped(input)?;
            basic_chars.to_mut().push(c);
            let s = basic_unescaped(input)?;
            basic_chars.to_mut().push_str(s);
        }
        debug_assert_utf8!(input.input, "nested parsers must end on `char` boundary");

        Ok(basic_chars)
    })
    .parse_next(input)
}

/// `basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii`
///
/// # Safety
///
/// - `stream` must be UTF-8
fn basic_unescaped<'i, 'e, ES: ErrorSink<'i>>(
    input: &mut BStrInput<'i, 'e, ES>,
) -> PResult<&'i str, Error> {
    trace("basic-unescaped", |input: &mut BStrInput<'i, 'e, ES>| {
        debug_assert_utf8!(input.input, "caller must start on `char` boundary");

        let s = take_while(0.., BASIC_UNESCAPED).parse_next(input)?;
        debug_assert_utf8!(
            input.input,
            "`BASIC_UNESCAPED` matches multi-byte UTF-8 `char`s"
        );

        debug_assert_utf8!(s, "`BASIC_UNESCAPED` matches multi-byte UTF-8 `char`s");
        let s = unsafe { std::str::from_utf8_unchecked(s) };

        Ok(s)
    })
    .parse_next(input)
}

/// `basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii`
#[allow(clippy::type_complexity)]
pub(crate) const BASIC_UNESCAPED: (
    (u8, u8),
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (WSCHAR, 0x21, 0x23..=0x5B, 0x5D..=0x7E, NON_ASCII);

/// `escaped = escape escape-seq-char`
///
/// # Safety
///
/// - `stream` must be UTF-8
fn escaped<'i, 'e, ES: ErrorSink<'i>>(input: &mut BStrInput<'i, 'e, ES>) -> PResult<char, Error> {
    trace("escaped", |input: &mut BStrInput<'i, 'e, ES>| {
        debug_assert_utf8!(input.input, "caller must start on `char` boundary");

        let start = input.checkpoint();
        #[allow(const_item_mutation)]
        let escape: PResult<_, Error> = ESCAPE.parse_next(input);
        if escape.is_err() {
            input.reset(&start);
            debug_assert_utf8!(
                input.input,
                "`start` was captured at fn start which is a `char` boundary"
            );

            let unexpected = Raw::new_unchecked(substr_at(
                unsafe { std::str::from_utf8_unchecked(input.input) },
                0,
            ));
            input.state.report_error(&[], unexpected);

            let _ = input.next_slice(unexpected.len());
            debug_assert_utf8!(input.input, "`substr_at` must end on `char` boundary");

            return Ok(' ');
        }

        let unescaped = escape_seq_char(input)?;
        debug_assert_utf8!(input.input, "nested parsers must end on `char` boundary");

        Ok(unescaped)
    })
    .parse_next(input)
}

/// `escape = %x5C                    ; \`
const ESCAPE: u8 = b'\\';

/// ```bnf
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
fn escape_seq_char<'i, 'e, ES: ErrorSink<'i>>(
    input: &mut BStrInput<'i, 'e, ES>,
) -> PResult<char, Error> {
    trace("escape-seq-char", |input: &mut BStrInput<'i, 'e, ES>| {
        debug_assert_utf8!(input.input, "caller must start on `char` boundary");

        let start = input.checkpoint();
        let id = match input.next_token() {
            Some(id) => id,
            None => {
                input.reset(&start);
                debug_assert_utf8!(
                    input.input,
                    "`start` was captured at fn start which is a `char` boundary"
                );
                let unexpected = Raw::new_unchecked(
                    &unsafe { std::str::from_utf8_unchecked(input.input) }[0..0],
                );
                input.state.report_error(&[], unexpected);
                b'\"'
            }
        };
        let unescaped = match id {
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let result: PResult<_, Error> = hexescape::<ES, 4>(input);
                match result {
                    Ok(c) => c,
                    Err(_) => {
                        debug_assert_utf8!(
                            input.input,
                            "nested parsers must end on `char` boundary"
                        );
                        let unexpected = Raw::new_unchecked(substr_at(
                            unsafe { std::str::from_utf8_unchecked(input.input) },
                            0,
                        ));
                        input.state.report_error(
                            &[Expected::Description("unicode 4-digit hex code")],
                            unexpected,
                        );
                        ' '
                    }
                }
            }
            b'U' => {
                let result: PResult<_, Error> = hexescape::<ES, 8>(input);
                match result {
                    Ok(c) => c,
                    Err(_) => {
                        debug_assert_utf8!(
                            input.input,
                            "nested parsers must end on `char` boundary"
                        );
                        let unexpected = Raw::new_unchecked(substr_at(
                            unsafe { std::str::from_utf8_unchecked(input.input) },
                            0,
                        ));
                        input.state.report_error(
                            &[Expected::Description("unicode 8-digit hex code")],
                            unexpected,
                        );
                        ' '
                    }
                }
            }
            b'\\' => '\\',
            b'"' => '"',
            _ => {
                input.reset(&start);
                debug_assert_utf8!(
                    input.input,
                    "`start` was captured at fn start which is a `char` boundary"
                );
                let unexpected = Raw::new_unchecked(substr_at(
                    unsafe { std::str::from_utf8_unchecked(input.input) },
                    0,
                ));
                input.state.report_error(
                    &[
                        Expected::Literal("b"),
                        Expected::Literal("f"),
                        Expected::Literal("n"),
                        Expected::Literal("r"),
                        Expected::Literal("\\"),
                        Expected::Literal("\""),
                        Expected::Literal("u"),
                        Expected::Literal("U"),
                    ],
                    unexpected,
                );
                ' '
            }
        };
        debug_assert_utf8!(input.input, "nested parsers must end on `char` boundary");

        Ok(unescaped)
    })
    .parse_next(input)
}

/// # Safety
///
/// - `stream` must be UTF-8
fn hexescape<'i, 'e, ES: ErrorSink<'i>, const N: usize>(
    input: &mut BStrInput<'i, 'e, ES>,
) -> PResult<char, Error> {
    debug_assert_utf8!(input.input, "caller must start on `char` boundary");

    let value = take_while(0..=N, HEXDIG)
        .verify(|b: &[u8]| b.len() == N)
        .parse_next(input)?;
    debug_assert_utf8!(input.input, "`HEXDIG` is ASCII only");
    debug_assert_utf8!(value, "`HEXDIG` is ASCII only");

    let value = unsafe { std::str::from_utf8_unchecked(value) };
    let value = u32::from_str_radix(value, 16).map_err(|_| ErrMode::Backtrack(()))?;
    let value = char::from_u32(value).ok_or(ErrMode::Backtrack(()))?;

    Ok(value)
}

/// `HEXDIG = DIGIT / "A" / "B" / "C" / "D" / "E" / "F"`
const HEXDIG: (RangeInclusive<u8>, RangeInclusive<u8>, RangeInclusive<u8>) =
    (DIGIT, b'A'..=b'F', b'a'..=b'f');

/// `DIGIT = %x30-39 ; 0-9`
const DIGIT: RangeInclusive<u8> = b'0'..=b'9';

fn strip_start_newline(s: &str) -> &str {
    s.strip_prefix('\n')
        .or_else(|| s.strip_prefix("\r\n"))
        .unwrap_or(s)
}

/// Parse multi-line basic string
///
/// ```bnf
/// ;; Multiline Basic String
///
/// ml-basic-string = ml-basic-string-delim [ newline ] ml-basic-body
///                   ml-basic-string-delim
/// ml-basic-string-delim = 3quotation-mark
/// ```
pub fn parse_ml_basic_string<'i, ES: ErrorSink<'i>>(raw: Raw<'i>, error: &mut ES) -> Cow<'i, str> {
    let mut state = State {
        error,
        context: raw,
        description: TokenKind::MlBasicString.description(),
    };

    let s = raw.as_str();
    let s = if let Some(stripped) = s.strip_prefix(ML_BASIC_STRING_DELIM) {
        stripped
    } else {
        state.report_error(&[Expected::Literal("\"")], raw.before());
        s
    };
    let s = strip_start_newline(s);
    let s = if let Some(stripped) = s.strip_suffix(ML_BASIC_STRING_DELIM) {
        stripped
    } else {
        state.report_error(&[Expected::Literal("\"")], raw.after());
        s.trim_end_matches('"')
    };

    let mut input = BStrInput {
        input: BStr::new(s),
        state,
    };
    let c = match ml_basic_body(&mut input) {
        Ok(c) => {
            debug_assert!(input.is_empty(), "remaining content: {:?}", input);
            c
        }
        Err(_) => {
            #[cfg(debug_assertions)]
            unreachable!(
                "errors should be reported deeper in the parser where more context exists"
            );
            #[cfg_attr(debug_assertions, allow(unreachable_code))]
            Cow::Borrowed(s)
        }
    };

    c
}

/// ```bnf
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
fn ml_basic_body<'i, 'e, ES: ErrorSink<'i>>(
    input: &mut BStrInput<'i, 'e, ES>,
) -> PResult<Cow<'i, str>, Error> {
    trace("ml-basic-body", |input: &mut BStrInput<'i, 'e, ES>| {
        debug_assert_utf8!(input.input, "caller must start on `char` boundary");

        let s = mlb_unescaped(input)?;
        let mut basic_chars = Cow::Borrowed(s);
        while let Some(b) = input.input.first().copied() {
            match b {
                b'\r' => {
                    let s = newline(input)?;
                    basic_chars.to_mut().push_str(s);
                }
                b'\\' => {
                    if opt(mlb_escaped_nl).parse_next(input)?.is_some() {
                        // Ignore, nothing to add
                    } else {
                        let c = escaped(input)?;
                        basic_chars.to_mut().push(c);
                    }
                }
                _ => {
                    let unexpected = Raw::new_unchecked(substr_at(
                        unsafe { std::str::from_utf8_unchecked(input.input) },
                        0,
                    ));
                    input.state.report_error(&[], unexpected);
                    let _ = input.next_slice(unexpected.len());
                }
            }

            let s = mlb_unescaped(input)?;
            basic_chars.to_mut().push_str(s);
        }
        debug_assert_utf8!(input.input, "nested parsers must end on `char` boundary");

        Ok(basic_chars)
    })
    .parse_next(input)
}

/// ```bnf
/// mlb-escaped-nl = escape ws newline *( wschar / newline )
/// ```
fn mlb_escaped_nl<'i, 'e, ES: ErrorSink<'i>>(
    input: &mut BStrInput<'i, 'e, ES>,
) -> PResult<(), Error> {
    trace("mlb-escaped-nl", |input: &mut BStrInput<'i, 'e, ES>| {
        debug_assert_utf8!(input.input, "caller must start on `char` boundary");

        let _ = (
            ESCAPE,
            repeat(1.., (take_while(0.., WSCHAR), newline)).map(|()| ()),
            take_while(0.., WSCHAR),
        )
            .parse_next(input)?;
        debug_assert_utf8!(input.input, "`ESCAPE`, `WSCHAR`, `newline` are only ASCII");

        Ok(())
    })
    .parse_next(input)
}

/// `mlb-unescaped` extended with `mlb-quotes` and `LF`
///
/// **warning:** `newline` is not validated
///
/// ```bnf
/// ml-basic-body = *mlb-content *( mlb-quotes 1*mlb-content ) [ mlb-quotes ]
///
/// mlb-content = mlb-char / newline / mlb-escaped-nl
/// mlb-char = mlb-unescaped / escaped
/// mlb-quotes = 1*2quotation-mark
/// mlb-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
fn mlb_unescaped<'i, 'e, ES: ErrorSink<'i>>(
    input: &mut BStrInput<'i, 'e, ES>,
) -> PResult<&'i str, Error> {
    trace("mlb-unescaped", |input: &mut BStrInput<'i, 'e, ES>| {
        debug_assert_utf8!(input.input, "caller must start on `char` boundary");

        let s = take_while(0.., (MLB_UNESCAPED, b'"', b'\n')).parse_next(input)?;
        debug_assert_utf8!(
            input.input,
            "`MLB_UNESCAPED` matches multi-byte UTF-8 `char`s"
        );
        debug_assert_utf8!(s, "`MLB_UNESCAPED` matches all of multi-byte UTF-8 `char`s");
        let s = unsafe { std::str::from_utf8_unchecked(s) };

        Ok(s)
    })
    .parse_next(input)
}

/// `mlb-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii`
#[allow(clippy::type_complexity)]
pub(crate) const MLB_UNESCAPED: (
    (u8, u8),
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (WSCHAR, 0x21, 0x23..=0x5B, 0x5D..=0x7E, NON_ASCII);

#[cfg(test)]
mod test {
    use super::*;

    use snapbox::assert_data_eq;
    use snapbox::prelude::*;
    use snapbox::str;

    #[test]
    fn literal_string() {
        let cases = [
            (
                r"'C:\Users\nodejs\templates'",
                str![[r#"C:\Users\nodejs\templates"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r"'\\ServerX\admin$\system32\'",
                str![[r#"\\ServerX\admin$\system32\"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"'Tom "Dubs" Preston-Werner'"#,
                str![[r#"Tom "Dubs" Preston-Werner"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r"'<\i\c*\s*>'",
                str![[r#"<\i\c*\s*>"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
        ];
        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let actual = parse_literal_string(Raw::new_unchecked(input), &mut error);
            assert_data_eq!(actual, expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }

    #[test]
    fn ml_literal_string() {
        let cases = [
            (
                r"'''I [dw]on't need \d{2} apples'''",
                str![[r#"I [dw]on't need \d{2} apples"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"''''one_quote''''"#,
                str!["'one_quote'"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"'''
The first newline is
trimmed in raw strings.
   All other whitespace
   is preserved.
'''"#,
                str![[r#"
The first newline is
trimmed in raw strings.
   All other whitespace
   is preserved.

"#]]
                .raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
        ];
        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let actual = parse_ml_literal_string(Raw::new_unchecked(input), &mut error);
            assert_data_eq!(actual, expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }

    #[test]
    fn basic_string() {
        let cases = [
            (
                r#""""#,
                str![""].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""content\"trailing""#,
                str![[r#"content"trailing"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""content\""#,
                str![[r#"content""#]].raw(),
                str![[r#"
[
    ParseError {
        context: "\"content\\\"",
        description: "basic string",
        expected: [],
        unexpected: "",
    },
]

"#]]
                .raw(),
            ),
            (
                r#""content
trailing""#,
                str!["content trailing"].raw(),
                str![[r#"
[
    ParseError {
        context: "\"content\ntrailing\"",
        description: "basic string",
        expected: [],
        unexpected: "\n",
    },
]

"#]]
                .raw(),
            ),
            (
                r#""I'm a string. \"You can quote me\". Name\tJos\u00E9\nLocation\tSF. \U0002070E""#,
                str![[r#"
I'm a string. "You can quote me". Name	José
Location	SF. 𠜎
"#]]
                .raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
        ];
        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let actual = parse_basic_string(Raw::new_unchecked(input), &mut error);
            assert_data_eq!(actual.as_ref(), expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }

    #[test]
    fn ml_basic_string() {
        let cases = [
            (
                r#""""
Roses are red
Violets are blue""""#,
                str![[r#"
Roses are red
Violets are blue
"#]]
                .raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"""" \""" """"#,
                str![[r#" """ "#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"""" \\""""#,
                str![[r#" \"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""
The quick brown \


  fox jumps over \
    the lazy dog.""""#,
                str!["The quick brown fox jumps over the lazy dog."].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""\
       The quick brown \
       fox jumps over \
       the lazy dog.\
       """"#,
                str!["The quick brown fox jumps over the lazy dog."].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""\
       """"#,
                str![""].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""
\
  \
""""#,
                str![""].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""  """#,
                str!["  "].raw(),
                str![[r#"
[
    ParseError {
        context: "\"\"\"  \"\"",
        description: "multi-line basic string",
        expected: [
            Literal(
                "\"",
            ),
        ],
        unexpected: "",
    },
]

"#]]
                .raw(),
            ),
            (
                r#""""  \""""#,
                str![[r#"  ""#]].raw(),
                str![[r#"
[
    ParseError {
        context: "\"\"\"  \\\"\"\"",
        description: "multi-line basic string",
        expected: [],
        unexpected: "",
    },
]

"#]]
                .raw(),
            ),
        ];
        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let actual = parse_ml_basic_string(Raw::new_unchecked(input), &mut error);
            assert_data_eq!(actual.as_ref(), expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }
}
