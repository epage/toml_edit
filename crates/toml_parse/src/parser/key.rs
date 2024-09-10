use std::ops::RangeInclusive;

use winnow::stream::ContainsToken as _;

use crate::lexer::Raw;
use crate::parser::substr_at;
use crate::parser::State;
use crate::ErrorSink;
use crate::Expected;

/// Parse unquoted key
///
/// ```bnf
/// unquoted-key = 1*( ALPHA / DIGIT / %x2D / %x5F ) ; A-Z / a-z / 0-9 / - / _
/// ```
pub fn parse_unquoted_key<'i, ES: ErrorSink<'i>>(raw: Raw<'i>, error: &mut ES) -> &'i str {
    let mut state = State {
        error,
        context: raw,
        description: "unquoted-key",
    };

    let s = raw.as_str();

    for (i, b) in s.as_bytes().iter().enumerate() {
        if !UNQUOTED_CHAR.contains_token(b) {
            let unexpected = Raw::new_unchecked(substr_at(s, i));
            state.report_error(
                &[
                    Expected::Description("letters"),
                    Expected::Description("numbers"),
                    Expected::Literal("-"),
                    Expected::Literal("_"),
                ],
                unexpected,
            );
        }
    }

    s
}

/// `unquoted-key = 1*( ALPHA / DIGIT / %x2D / %x5F ) ; A-Z / a-z / 0-9 / - / _`
pub(crate) const UNQUOTED_CHAR: (
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    u8,
    u8,
) = (b'A'..=b'Z', b'a'..=b'z', b'0'..=b'9', b'-', b'_');

#[cfg(test)]
mod test {
    use super::*;

    use snapbox::assert_data_eq;
    use snapbox::prelude::*;
    use snapbox::str;

    #[test]
    fn unquoted_keys() {
        let cases = [
            (
                "a",
                str!["a"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "hello",
                str!["hello"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "-",
                str!["-"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "_",
                str!["_"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "-hello-world-",
                str!["-hello-world-"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "_hello_world_",
                str!["_hello_world_"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
        ];

        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let actual = parse_unquoted_key(Raw::new_unchecked(input), &mut error);
            assert_data_eq!(actual, expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }
}
