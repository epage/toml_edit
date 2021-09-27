use std::borrow::Cow;
use std::char;

use nom::{
    branch::*, bytes::complete::*, character::complete::*, combinator::*, error::context, multi::*,
    sequence::*, AsChar, IResult,
};

use crate::parser::errors::CustomError;
use crate::parser::trivia::{is_non_ascii, is_wschar, newline, ws, ws_newlines};

// ;; String

// string = ml-basic-string / basic-string / ml-literal-string / literal-string
parse!(string() -> String, {
    choice((
        ml_basic_string(),
        basic_string(),
        ml_literal_string(),
        literal_string().map(|s: &'a str| s.into()),
    ))
});

// ;; Basic String

// basic-string = quotation-mark *basic-char quotation-mark
parse!(basic_string() -> String, {
    between(
        char(QUOTATION_MARK), char(QUOTATION_MARK),
        many(basic_chars())
    )
    .message("While parsing a Basic String")
});

// quotation-mark = %x22            ; "
const QUOTATION_MARK: char = '"';

// basic-char = basic-unescaped / escaped
parse!(basic_chars() -> Cow<'a, str>, {
    choice((
        // Deviate from the official grammar by batching the unescaped chars so we build a string a
        // chunk at a time, rather than a `char` at a time.
        take_while1(is_basic_unescaped).map(Cow::Borrowed),
        satisfy(|c| c == ESCAPE)
            .then(|_| parser(move |input| {
                escape().parse_stream(input).into_result().map(|(c, e)| (Cow::Owned(String::from(c)), e))
            }))
    ))
});

// basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
#[inline]
fn is_basic_unescaped(c: impl AsChar) -> bool {
    let c = c.as_char();
    is_wschar(c)
        | matches!(c, '\u{21}' | '\u{23}'..='\u{5B}' | '\u{5D}'..='\u{7E}')
        | is_non_ascii(c)
}

// escape = %x5C                    ; \
const ESCAPE: char = '\\';

parse!(escape() -> char, {
    satisfy(is_escape_seq_char)
        .message("While parsing escape sequence")
        .then(|c| {
            parser(move |input| {
                match c {
                    'b'  => Ok(('\u{8}', Commit::Peek(()))),
                    'f'  => Ok(('\u{c}', Commit::Peek(()))),
                    'n'  => Ok(('\n',    Commit::Peek(()))),
                    'r'  => Ok(('\r',    Commit::Peek(()))),
                    't'  => Ok(('\t',    Commit::Peek(()))),
                    'u'  => hexescape(4).parse_stream(input).into_result(),
                    'U'  => hexescape(8).parse_stream(input).into_result(),
                    // ['\\', '"',]
                    _    => Ok((c,       Commit::Peek(()))),
                }
            })
        })
});

// escape-seq-char =  %x22         ; "    quotation mark  U+0022
// escape-seq-char =/ %x5C         ; \    reverse solidus U+005C
// escape-seq-char =/ %x62         ; b    backspace       U+0008
// escape-seq-char =/ %x66         ; f    form feed       U+000C
// escape-seq-char =/ %x6E         ; n    line feed       U+000A
// escape-seq-char =/ %x72         ; r    carriage return U+000D
// escape-seq-char =/ %x74         ; t    tab             U+0009
// escape-seq-char =/ %x75 4HEXDIG ; uXXXX                U+XXXX
// escape-seq-char =/ %x55 8HEXDIG ; UXXXXXXXX            U+XXXXXXXX
#[inline]
fn is_escape_seq_char(c: impl AsChar) -> bool {
    let c = c.as_char();
    matches!(c, '"' | '\\' | 'b' | 'f' | 'n' | 'r' | 't' | 'u' | 'U')
}

parse!(hexescape(n: usize) -> char, {
    take(*n)
        .and_then(|s| u32::from_str_radix(s, 16))
        .and_then(|h| char::from_u32(h).ok_or(CustomError::InvalidHexEscape(h)))
});

// ;; Multiline Basic String

// ml-basic-string = ml-basic-string-delim ml-basic-body ml-basic-string-delim
parse!(ml_basic_string() -> String, {
    between(range(ML_BASIC_STRING_DELIM),
            range(ML_BASIC_STRING_DELIM),
            ml_basic_body())
        .message("While parsing a Multiline Basic String")
});

// ml-basic-string-delim = 3quotation-mark
const ML_BASIC_STRING_DELIM: &str = "\"\"\"";

// ml-basic-body = *( ( escape ws-newline ) / ml-basic-char / newline )
parse!(ml_basic_body() -> String, {
    //  A newline immediately following the opening delimiter will be trimmed.
    optional(newline())
        .skip(try_eat_escaped_newline())
        .with(
            many(
                not_followed_by(range(ML_BASIC_STRING_DELIM).map(Info::Range))
                    .with(
                        choice((
                            // `TOML parsers should feel free to normalize newline
                            //  to whatever makes sense for their platform.`
                            newline(),
                            mlb_char(),
                        ))
                    )
                    .skip(try_eat_escaped_newline())
            )
        )
});
// ml-basic-body = *mlb-content *( mlb-quotes 1*mlb-content ) [ mlb-quotes ]
pub(crate) fn ml_basic_body(input: &str) -> IResult<&str, &str> {
    map(
        tuple((
            many0(mll_content),
            many0(map(tuple((mll_quotes, many1(mll_content))), |(q, c)| {
                let mut total = q.to_owned();
                total.push_str(&c);
                total
            })),
            opt(mll_quotes),
        )),
        |(mut c, qc, q)| {
            c.push_str(&qc);
            c.push_str(q);
            c
        },
    )(input)
}

// mlb-content = mlb-char / newline / mlb-escaped-nl
// mlb-char = mlb-unescaped / escaped
pub(crate) fn mlb_content(input: &str) -> IResult<&str, &str> {
    alt((
        // Deviate from the official grammar by batching the unescaped chars so we build a string a
        // chunk at a time, rather than a `char` at a time.
        take_while1(is_mlb_unescaped),
        escaped,
        map(newline, |_| "\n"),
        map(mlb_escaped_nl, |_| ""),
    ))(input)
}

// mlb-quotes = 1*2quotation-mark
pub(crate) fn mlb_quotes(input: &str) -> IResult<&str, &str> {
    alt((tag("\""), tag("\"")))(input)
}

// mlb-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
#[inline]
fn is_mlb_unescaped(c: impl AsChar) -> bool {
    let c = c.as_char();
    is_wschar(c)
        | matches!(c, '\u{21}' | '\u{23}'..='\u{5B}' | '\u{5D}'..='\u{7E}')
        | is_non_ascii(c)
}

// mlb-escaped-nl = escape ws newline *( wschar / newline )
// When the last non-whitespace character on a line is a \,
// it will be trimmed along with all whitespace
// (including newlines) up to the next non-whitespace
// character or closing delimiter.
pub(crate) fn mlb_escaped_nl(input: &str) -> IResult<&str, ()> {
    map(many0_count(tuple((char(ESCAPE), ws, ws_newlines))), |_| ())(input)
}

// ;; Literal String

// literal-string = apostrophe *literal-char apostrophe
pub(crate) fn literal_string(input: &str) -> IResult<&str, &str> {
    delimited(
        char(APOSTROPHE),
        take_while(is_literal_char),
        char(APOSTROPHE),
    )(input)
}

// apostrophe = %x27 ; ' apostrophe
const APOSTROPHE: char = '\'';

// literal-char = %x09 / %x20-26 / %x28-7E / non-ascii
#[inline]
fn is_literal_char(c: impl AsChar) -> bool {
    let c = c.as_char();
    matches!(c, '\u{09}' | '\u{20}'..='\u{26}' | '\u{28}'..='\u{7E}') | is_non_ascii(c)
}

// ;; Multiline Literal String

// ml-literal-string = ml-literal-string-delim [ newline ] ml-literal-body
//                     ml-literal-string-delim
pub(crate) fn ml_literal_string(input: &str) -> IResult<&str, String> {
    delimited(
        tag(ML_LITERAL_STRING_DELIM),
        map(tuple((opt(newline), ml_literal_body)), |(_, b)| {
            b.replace("\r\n", "\n")
        }),
        tag(ML_LITERAL_STRING_DELIM),
    )(input)
}

// ml-literal-string-delim = 3apostrophe
const ML_LITERAL_STRING_DELIM: &str = "'''";

/// ml-literal-body = *mll-content *( mll-quotes 1*mll-content ) [ mll-quotes ]
pub(crate) fn ml_literal_body(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        many0_count(mll_content),
        many0_count(tuple((mll_quotes, many1_count(mll_content)))),
        opt(mll_quotes),
    )))(input)
}

// mll-content = mll-char / newline
pub(crate) fn mll_content(input: &str) -> IResult<&str, char> {
    alt((satisfy(is_mll_char), newline))(input)
}

// mll-char = %x09 / %x20-26 / %x28-7E / non-ascii
#[inline]
fn is_mll_char(c: impl AsChar) -> bool {
    let c = c.as_char();
    matches!(c, '\u{09}' | '\u{20}'..='\u{26}' | '\u{28}'..='\u{7E}') | is_non_ascii(c)
}

// mll-quotes = 1*2apostrophe
pub(crate) fn mll_quotes(input: &str) -> IResult<&str, &str> {
    alt((tag("''"), tag("'")))(input)
}
