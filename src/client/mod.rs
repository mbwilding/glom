//! GitHub client modules
//!
//! This module provides a well-structured, testable GitHub API client
//! split into focused components following single responsibility principle.

pub mod api;
pub mod config;
pub mod error;
pub mod poller;
pub mod service;

// Re-export main types for convenience
pub use api::GithubApi;
pub use config::ClientConfig;
pub use error::ClientError;
pub use poller::GithubPoller;
pub use service::GithubService;

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, ClientError>;
