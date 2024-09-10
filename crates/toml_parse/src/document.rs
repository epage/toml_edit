use winnow::stream::Offset as _;

use crate::lexer::Lexer;
use crate::lexer::Raw;

pub struct Document<'i> {
    input: &'i str,
}

impl<'i> Document<'i> {
    pub fn new(input: &'i str) -> Self {
        Self { input }
    }

    pub fn lex(&self) -> Lexer<'i> {
        Lexer::new(self.input)
    }

    pub fn input(&self) -> &'i str {
        self.input
    }

    /// Byte-span for the given [`Token `]
    ///
    /// # Panic
    ///
    /// If `token` was not taken from [`Document::input`]
    pub fn span(&self, raw: Raw<'i>) -> std::ops::Range<usize> {
        let start = raw.inner.offset_from(&self.input());
        let end = start + raw.len();
        start..end
    }
}
