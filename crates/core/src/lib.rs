//! tool_core: every rule of Formula Builder.
//!
//! Builder requirement: this crate performs no input or output. It takes
//! values and returns values, so `cargo test` reaches every rule without a
//! browser. `tests/boundary.rs` enforces that, along with the dependency
//! allowlist and the reference ban on floats, hash maps and clocks.

pub mod api;
pub mod doc;
pub mod eval;
pub mod export;
pub mod font;
pub mod gif;
pub mod glossary;
pub mod layout;
pub mod page;
pub mod palette;
pub mod parts;
pub mod render;
pub mod sheet;
pub mod sheetedit;
pub mod tex;
pub mod tiles;
pub mod value;
pub mod worked;
