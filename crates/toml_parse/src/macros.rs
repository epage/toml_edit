macro_rules! debug_assert_utf8 {
    ($utf8: expr) => {{
        debug_assert_utf8!($utf8,);
    }};
    ($utf8: expr , $($arg:tt)*) => {{
        let utf8: &[u8] = $utf8;
        // Doing equality check to have message written for us
        debug_assert_eq!(std::str::from_utf8(utf8).map(|s| s.as_bytes()), Ok(utf8), $($arg)*);
    }};
}
