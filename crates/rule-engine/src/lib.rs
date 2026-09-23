//! Pure rule compilation, caching, and evaluation for Extrittio.
//!
//! This crate deliberately contains no persistence, transport, HTTP, or
//! worker dependencies. Evaluation produces [`types::PendingAction`] values;
//! the backend is responsible for committing and delivering those actions.

pub mod cache;
pub mod compiler;
pub mod evaluate;
pub mod geo;
pub mod metric;
pub mod model;
pub mod number;
pub mod types;
