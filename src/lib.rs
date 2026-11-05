pub mod auth;
pub mod config;
pub mod error;
pub mod health;
pub mod matrix;
pub mod mcp;

pub use config::Config;
pub use error::{AppError, Result};
