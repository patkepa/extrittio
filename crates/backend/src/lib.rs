#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::doc_markdown,
    clippy::similar_names,
    clippy::too_many_lines
)]

pub mod api;
#[path = "domains/identity/api_key_util.rs"]
pub mod api_key_util;
pub(crate) mod app;
pub mod auth;
pub mod background;
pub mod config;
mod database;
pub mod domains;
pub mod error;
pub mod init;
pub mod middleware;
pub mod observability;
pub mod outbound;
pub mod pagination;
pub mod persistence;
pub mod rate_limit;
#[path = "domains/rules/rule_engine/mod.rs"]
pub mod rule_engine;
pub mod security;
pub mod service;
pub mod services;
pub mod state;
pub mod tenancy;
pub mod util;
pub mod zenoh_handler;

pub mod rule_snapshots;
