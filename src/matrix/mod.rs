pub mod cache;
pub mod client;
#[cfg(feature = "e2ee")]
pub mod e2ee;
#[cfg(any(test, feature = "e2ee"))]
pub mod secrets;

pub use cache::ClientCache;
pub use client::create_matrix_client;
