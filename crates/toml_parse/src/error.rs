use crate::lexer::Raw;

pub trait ErrorSink<'i>: std::fmt::Debug {
    fn report_error(&mut self, error: ParseError<'i>);
}

impl<'i, S: ErrorSink<'i>> ErrorSink<'i> for &mut S {
    fn report_error(&mut self, error: ParseError<'i>) {
        S::report_error(self, error)
    }
}

impl<'i> ErrorSink<'i> for () {
    fn report_error(&mut self, _error: ParseError<'i>) {}
}

impl<'i> ErrorSink<'i> for Option<ParseError<'i>> {
    fn report_error(&mut self, error: ParseError<'i>) {
        self.get_or_insert(error);
    }
}

impl<'i> ErrorSink<'i> for Vec<ParseError<'i>> {
    fn report_error(&mut self, error: ParseError<'i>) {
        self.push(error)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub struct ParseError<'i> {
    pub context: Raw<'i>,
    pub description: &'static str,
    pub expected: &'static [Expected],
    pub unexpected: Raw<'i>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Expected {
    Literal(&'static str),
    Description(&'static str),
}
