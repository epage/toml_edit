//! A [TOML]-parsing library
//!
//! This library implements a [TOML] v0.5.0 compatible parser,
//! primarily supporting the [`serde`] library for encoding/decoding
//! various types in Rust.
//!
//! TOML itself is a simple, ergonomic, and readable configuration format:
//!
//! ```toml
//! [package]
//! name = "toml"
//! version = "0.4.2"
//! authors = ["Alex Crichton <alex@alexcrichton.com>"]
//!
//! [dependencies]
//! serde = "1.0"
//! ```
//!
//! The TOML format tends to be relatively common throughout the Rust community
//! for configuration, notably being used by [Cargo], Rust's package manager.
//!
//! ## TOML values
//!
//! A value in TOML is represented with the [`Value`] enum in this crate.
//!
//! TOML is similar to JSON with the notable addition of a [`Datetime`]
//! type. In general, TOML and JSON are interchangeable in terms of
//! formats.
//!
//! ## Parsing TOML
//!
//! The easiest way to parse a TOML document is via the [`Value`] type:
//!
//! ```rust
//! use toml_edit::easy::Value;
//!
//! let value = "foo = 'bar'".parse::<Value>().unwrap();
//!
//! assert_eq!(value["foo"].as_str(), Some("bar"));
//! ```
//!
//! The [`Value`] type implements a number of convenience methods and
//! traits; the example above uses [`FromStr`] to parse a [`str`] into a
//! [`Value`].
//!
//! ## Deserialization and Serialization
//!
//! This crate supports [`serde`] 1.0 with a number of
//! implementations of the `Deserialize`, `Serialize`, `Deserializer`, and
//! `Serializer` traits. Namely, you'll find:
//!
//! * `Deserialize for Value`
//! * `Serialize for Value`
//! * `Deserialize for Datetime`
//! * `Serialize for Datetime`
//! * `Deserializer for de::Deserializer`
//! * `Serializer for ser::Serializer`
//! * `Deserializer for Value`
//!
//! This means that you can use Serde to deserialize/serialize the
//! [`Value`] type as well as the [`Datetime`] type in this crate. You can also
//! use the [`Deserializer`], [`Serializer`], or [`Value`] type itself to act as
//! a deserializer/serializer for arbitrary types.
//!
//! An example of deserializing with TOML is:
//!
//! ```rust
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Config {
//!     ip: String,
//!     port: Option<u16>,
//!     keys: Keys,
//! }
//!
//! #[derive(Deserialize)]
//! struct Keys {
//!     github: String,
//!     travis: Option<String>,
//! }
//!
//! let config: Config = toml_edit::easy::from_str(r#"
//!     ip = '127.0.0.1'
//!
//!     [keys]
//!     github = 'xxxxxxxxxxxxxxxxx'
//!     travis = 'yyyyyyyyyyyyyyyyy'
//! "#).unwrap();
//!
//! assert_eq!(config.ip, "127.0.0.1");
//! assert_eq!(config.port, None);
//! assert_eq!(config.keys.github, "xxxxxxxxxxxxxxxxx");
//! assert_eq!(config.keys.travis.as_ref().unwrap(), "yyyyyyyyyyyyyyyyy");
//! ```
//!
//! You can serialize types in a similar fashion:
//!
//! ```rust
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Config {
//!     ip: String,
//!     port: Option<u16>,
//!     keys: Keys,
//! }
//!
//! #[derive(Serialize)]
//! struct Keys {
//!     github: String,
//!     travis: Option<String>,
//! }
//!
//! let config = Config {
//!     ip: "127.0.0.1".to_string(),
//!     port: None,
//!     keys: Keys {
//!         github: "xxxxxxxxxxxxxxxxx".to_string(),
//!         travis: Some("yyyyyyyyyyyyyyyyy".to_string()),
//!     },
//! };
//!
//! let toml = toml_edit::easy::to_string(&config).unwrap();
//! ```
//!
//! [TOML]: https://github.com/toml-lang/toml
//! [Cargo]: https://crates.io/
//! [`serde`]: https://serde.rs/

mod datetime;

pub mod de;
#[doc(hidden)]
pub mod macros;
pub mod map;
pub mod ser;
pub mod value;

pub use crate::toml;
#[doc(no_inline)]
pub use de::{from_document, from_slice, from_str, Deserializer};
#[doc(no_inline)]
pub use ser::{to_document, to_string, to_string_pretty, to_vec, Serializer};
#[doc(no_inline)]
pub use value::Value;
