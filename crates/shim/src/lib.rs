//! tool_shim: forwards values between the page and `tool_core::api`.
//!
//! Builder requirement: the shim decides nothing. Each function is one call
//! into the core. `crates/core/tests/boundary.rs` fails if a branch, a loop or
//! the `?` operator appears here; if a feature seems to need one, the rule
//! belongs in the core.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn start(strings_json: &str, examples_json: &str, glossary_md: &str, mode: &str) -> String {
    tool_core::api::start(strings_json, examples_json, glossary_md, mode)
}

#[wasm_bindgen]
pub fn act(state_json: &str, glossary_md: &str, name: &str, value: &str) -> String {
    tool_core::api::act(state_json, glossary_md, name, value)
}

#[wasm_bindgen]
pub fn gif(rgba: &[u8], width: u32, height: u32, delays: &str) -> Vec<u8> {
    tool_core::api::gif(rgba, width, height, delays)
}
