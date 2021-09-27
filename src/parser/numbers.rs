use nom::{
    branch::*, bytes::complete::*, character::complete::*, combinator::*, error::context, multi::*,
    sequence::*, AsChar, IResult,
};

// ;; Boolean

// boolean = true / false
pub(crate) fn boolean(input: &str) -> IResult<&str, bool> {
    alt((map(tag("true"), |_| true), map(tag("false"), |_| false)))(input)
}

// ;; Integer

// integer = dec-int / hex-int / oct-int / bin-int
pub(crate) fn integer(input: &str) -> IResult<&str, i64> {
    alt((
        hex_int,
        oct_int,
        bin_int,
        context(
            "While parsing an Integer",
            map_res(dec_int, |s| s.replace("_", "").parse()),
        ),
    ))(input)
}

// dec-int = [ minus / plus ] unsigned-dec-int
// unsigned-dec-int = DIGIT / digit1-9 1*( DIGIT / underscore DIGIT )
pub(crate) fn dec_int(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        opt(alt((char('+'), char('-')))),
        alt((
            char('0'),
            map(
                tuple((
                    satisfy(|c| ('1'..='9').contains(&c)),
                    take_while(is_dec_digit_with_sep),
                )),
                |t| t.0,
            ),
        )),
    )))(input)
}
#[inline]
fn is_dec_digit_with_sep(i: impl AsChar + Copy) -> bool {
    i.is_dec_digit() || i.as_char() == '_'
}

// hex-prefix = %x30.78               ; 0x
// hex-int = hex-prefix HEXDIG *( HEXDIG / underscore HEXDIG )
pub(crate) fn hex_int(input: &str) -> IResult<&str, i64> {
    context(
        "While parsing a hexadecimal Integer",
        map_res(
            tuple((
                tag("0x"),
                recognize(tuple((
                    satisfy(is_hex_digit),
                    take_while(is_hex_digit_with_sep),
                ))),
            )),
            |t: (&str, &str)| {
                let s = t.0;
                i64::from_str_radix(&s.replace("_", ""), 16)
            },
        ),
    )(input)
}
#[inline]
fn is_hex_digit(i: impl AsChar + Copy) -> bool {
    i.is_hex_digit()
}
#[inline]
fn is_hex_digit_with_sep(i: impl AsChar + Copy) -> bool {
    i.is_hex_digit() || i.as_char() == '_'
}

// oct-prefix = %x30.6F               ; 0o
// oct-int = oct-prefix digit0-7 *( digit0-7 / underscore digit0-7 )
pub(crate) fn oct_int(input: &str) -> IResult<&str, i64> {
    context(
        "While parsing an octal Integer",
        map_res(
            tuple((
                tag("0o"),
                recognize(tuple((
                    satisfy(is_oct_digit),
                    take_while(is_oct_digit_with_sep),
                ))),
            )),
            |t: (&str, &str)| {
                let s = t.0;
                i64::from_str_radix(&s.replace("_", ""), 8)
            },
        ),
    )(input)
}
#[inline]
fn is_oct_digit(i: impl AsChar + Copy) -> bool {
    i.is_oct_digit()
}
#[inline]
fn is_oct_digit_with_sep(i: impl AsChar + Copy) -> bool {
    i.is_oct_digit() || i.as_char() == '_'
}

// bin-prefix = %x30.62               ; 0b
// bin-int = bin-prefix digit0-1 *( digit0-1 / underscore digit0-1 )
pub(crate) fn bin_int(input: &str) -> IResult<&str, i64> {
    context(
        "While parsing a binary Integer",
        map_res(
            tuple((tag("0b"), recognize(tuple((one_of("01"), one_of("01_")))))),
            |t: (&str, &str)| {
                let s = t.0;
                i64::from_str_radix(&s.replace("_", ""), 2)
            },
        ),
    )(input)
}

// ;; Float

// float = float-int-part ( exp / frac [ exp ] )
// float =/ special-float
// float-int-part = dec-int
pub(crate) fn float(input: &str) -> IResult<&str, f64> {
    context(
        "While parsing a Float",
        alt((
            map_res(parse_float, |s| s.replace(" ", "").parse()),
            special_float,
        )),
    )(input)
}

pub(crate) fn parse_float(input: &str) -> IResult<&str, &str> {
    recognize(tuple((dec_int, opt(frac), exp)))(input)
}

// frac = decimal-point zero-prefixable-int
// decimal-point = %x2E               ; .
pub(crate) fn frac(input: &str) -> IResult<&str, &str> {
    recognize(tuple((char('.'), parse_zero_prefixable_int)))(input)
}

// zero-prefixable-int = DIGIT *( DIGIT / underscore DIGIT )
pub(crate) fn parse_zero_prefixable_int(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        satisfy(is_dec_digit),
        take_while(is_dec_digit_with_sep),
    )))(input)
}
#[inline]
fn is_dec_digit(i: impl AsChar + Copy) -> bool {
    i.is_dec_digit()
}

// exp = "e" float-exp-part
// float-exp-part = [ minus / plus ] zero-prefixable-int
pub(crate) fn exp(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        one_of("eE"),
        opt(one_of("+-")),
        parse_zero_prefixable_int,
    )))(input)
}

// special-float = [ minus / plus ] ( inf / nan )
pub(crate) fn special_float(input: &str) -> IResult<&str, f64> {
    map(
        tuple((opt(one_of("+-")), alt((nan, inf)))),
        |(s, f)| match s {
            Some('+') | None => f,
            Some('-') => -f,
            _ => unreachable!("one_of should prevent this"),
        },
    )(input)
}

// inf = %x69.6e.66  ; inf
pub(crate) fn inf(input: &str) -> IResult<&str, f64> {
    map(tag("inf"), |_| f64::INFINITY)(input)
}

// nan = %x6e.61.6e  ; nan
pub(crate) fn nan(input: &str) -> IResult<&str, f64> {
    map(tag("nan"), |_| f64::NAN)(input)
}
