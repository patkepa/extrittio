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

use diesel_migrations::{embed_migrations, EmbeddedMigrations};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
