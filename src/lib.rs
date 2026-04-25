//! f45 — 45s card game engine, Rust core.
//!
//! # Module layout (Stage 0)
//!
//! - [`cards`] — Suit, Rank, Card; parse and format.
//! - [`ranker`] — strength function and trump/top-trump predicates.
//! - [`rules`] — legal-move validation and trick resolution.
//! - [`error`] — `EngineError` enum mirroring PHP rejection codes.
//!
//! State machine, dealing, scoring come in Checkpoint C2.
//! PyO3 bindings expand in Checkpoint C4.

pub mod bindings;
pub mod cards;
pub mod error;
pub mod ranker;
pub mod rules;
pub mod state;

use pyo3::prelude::*;

/// Smoke-test function retained for C1a compatibility and basic module health checks.
#[pyfunction]
fn hello() -> &'static str {
    "Hello from Rust!"
}

#[pymodule]
fn _engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hello, m)?)?;
    bindings::register(m)?;
    Ok(())
}
