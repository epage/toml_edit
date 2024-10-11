//! TOML lexer and parser

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[macro_use]
mod macros;
mod document;

mod error;

pub mod lexer;
pub mod parser;

pub use document::Document;
pub use error::ErrorSink;
pub use error::Expected;
pub use error::ParseError;
