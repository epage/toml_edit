//! TOML lexer and parser

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[macro_use]
mod macros;
mod document;

pub mod lexer;

pub use document::Document;
