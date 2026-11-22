//! Arto's frontend bundles, now published as `arto_markdown::frontend`.
//!
//! The bundles moved into `arto-markdown`, behind its `frontend` feature, so
//! that a host depends on one crate whose renderer and frontend cannot drift
//! apart. This crate re-exports that module for hosts that still name it and
//! is no longer developed; depend on `arto-markdown` with
//! `features = ["frontend"]` instead.

pub use arto_markdown::frontend::*;
