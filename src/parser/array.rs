use crate::parser::trivia::ws_comment_newline;
use crate::parser::value::value;
use crate::{Array, Value};
use combine::parser::char::char;
use combine::parser::range::recognize_with_value;
use combine::stream::RangeStream;
use combine::*;

// ;; Array

// array = array-open array-values array-close
parse!(array() -> Array, {
    between(char(ARRAY_OPEN), char(ARRAY_CLOSE),
            array_values())
});

// note: we're omitting ws and newlines here, because
// they should be part of the formatted values
// array-open  = %x5B ws-newline  ; [
const ARRAY_OPEN: char = '[';
// array-close = ws-newline %x5D  ; ]
const ARRAY_CLOSE: char = ']';
// array-sep = ws %x2C ws  ; , Comma
const ARRAY_SEP: char = ',';

// note: this rule is modified
// array-values = [ ( array-value array-sep array-values ) /
//                  array-value / ws-comment-newline ]
parse!(array_values() -> Array, {
    (
        optional(
            recognize_with_value(
                sep_end_by1(array_value(), char(ARRAY_SEP))
            ).map(|(r, v): (&'a str, Array)| (v, r.ends_with(',')))
        ),
        ws_comment_newline(),
    ).map(|(array, trailing)| {
        let (mut array, comma) = array.unwrap_or_default();
        array.set_trailing_comma(comma);
        array.set_trailing(trailing);
        array
    })
});

parse!(array_value() -> Value, {
    attempt((
        ws_comment_newline(),
        value(),
        ws_comment_newline(),
    )).map(|(ws1, v, ws2)| v.decorated(ws1, ws2))
});
