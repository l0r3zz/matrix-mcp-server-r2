//! Matrix MCP Server R2
//! 
//! A drop-in Rust replacement for the TypeScript matrix-mcp-server.
//! Provides Matrix Client-Server API integration via MCP protocol.

pub mod config;
pub mod error;
pub mod mcp;
pub mod matrix;
pub mod tools;

pub use config::Config;
pub use error::{AppError, Result};
