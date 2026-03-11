#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::doc_markdown,
    clippy::similar_names,
    clippy::too_many_lines
)]

pub mod api;
pub mod auth;
pub mod background;
pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod pagination;
pub mod rate_limit;
pub mod repositories;
pub mod services;
pub mod shadow_utils;
pub mod state;
pub mod zenoh_handler;

use diesel_migrations::{EmbeddedMigrations, embed_migrations};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
