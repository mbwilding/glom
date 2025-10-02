//! Configuration management for GitHub client

use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, Utc};
use compact_str::CompactString;

use super::error::{ClientError, Result};
use crate::glom_app::GlomConfig;

/// Main configuration for GitHub client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// GitHub instance base URL
    pub base_url: CompactString,
    /// Private access token
    pub private_token: CompactString,
    /// Optional search filter for projects
    pub search_filter: Option<CompactString>,
    /// Polling configuration
    pub polling: PollingConfig,
    /// Request configuration
    pub request: RequestConfig,
    /// Debug configuration
    pub debug: DebugConfig,
}

/// Polling intervals configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PollingConfig {
    /// Interval for fetching projects
    pub projects_interval: Duration,
    /// Interval for fetching active jobs
    pub jobs_interval: Duration,
}

/// HTTP request configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RequestConfig {
    /// Number of items per page for paginated requests
    pub per_page: u32,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum number of retries for failed requests
    pub max_retries: u32,
}

/// Debug and logging configuration
#[derive(Debug, Clone)]
pub struct DebugConfig {
    /// Enable debug logging of HTTP responses
    pub log_responses: bool,
    /// Directory for storing debug logs
    pub log_directory: Option<PathBuf>,
}

/// Query parameters for fetching projects
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ProjectQuery {
    /// Search filter for project names
    pub search_filter: Option<CompactString>,
    /// Only fetch projects updated after this time
    pub updated_after: Option<DateTime<Utc>>,
    /// Number of results per page
    pub per_page: u32,
    /// Include project statistics
    pub include_statistics: bool,
    /// Include archived projects
    pub archived: bool,
    /// Only include projects where user is a member
    pub membership: bool,
    /// Search in namespaces
    pub search_namespaces: bool,
}

/// Query parameters for fetching pipelines
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct PipelineQuery {
    /// Only fetch pipelines updated after this time
    pub updated_after: Option<DateTime<Utc>>,
    /// Number of results per page
    pub per_page: u32,
    /// Pipeline scope (running, pending, finished, etc.)
    pub scope: Option<PipelineScope>,
    /// Pipeline status filter
    pub status: Option<PipelineStatus>,
}

/// Pipeline scope for filtering
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PipelineScope {
    Running,
    Pending,
    Finished,
    Branches,
    Tags,
}

/// Pipeline status for filtering
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PipelineStatus {
    Created,
    WaitingForResource,
    Preparing,
    Pending,
    Running,
    Success,
    Failed,
    Canceled,
    Skipped,
    Manual,
    Scheduled,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            projects_interval: Duration::from_secs(60),
            jobs_interval: Duration::from_secs(30),
        }
    }
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            per_page: 100,
            timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            log_responses: true, // Enable by default to help debug issues
            log_directory: Some(PathBuf::from("glom-logs")),
        }
    }
}

impl ClientConfig {
    /// Create a new client configuration
    pub fn new(
        base_url: impl Into<CompactString>,
        private_token: impl Into<CompactString>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            private_token: private_token.into(),
            search_filter: None,
            polling: PollingConfig::default(),
            request: RequestConfig::default(),
            debug: DebugConfig::default(),
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.base_url.is_empty() {
            return Err(ClientError::config_validation(
                "github_url",
                "Base URL cannot be empty",
            ));
        }

        if self.private_token.is_empty() {
            return Err(ClientError::config_validation(
                "github_token",
                "Private token cannot be empty",
            ));
        }

        if !self.base_url.starts_with("http://") && !self.base_url.starts_with("https://") {
            return Err(ClientError::config_validation(
                "github_url",
                "Base URL must start with http:// or https://",
            ));
        }

        if !self.base_url.contains("api.github.com")
            && !self.base_url.contains("github.com/api")
            && !self.base_url.contains("/api/v3")
        {
            return Err(ClientError::config_validation(
                "github_url",
                "Base URL should be a GitHub API URL (e.g., https://api.github.com or https://your-enterprise.com/api/v3)",
            ));
        }

        if url::Url::parse(&self.base_url).is_err() {
            return Err(ClientError::config_validation(
                "github_url",
                "Base URL is not a valid URL format",
            ));
        }

        if self.private_token.len() < 20 {
            return Err(ClientError::config_validation(
                "github_token",
                "GitHub token must be at least 20 characters long",
            ));
        }

        if !self.private_token.starts_with("ghp_")
            && !self.private_token.starts_with("gho_")
            && !self.private_token.starts_with("ghu_")
            && !self.private_token.starts_with("ghs_")
            && !self.private_token.starts_with("ghr_")
        {
            return Err(ClientError::config_validation(
                "github_token",
                "GitHub token should start with ghp_, gho_, ghu_, ghs_, or ghr_",
            ));
        }

        if self.request.per_page == 0 || self.request.per_page > 100 {
            return Err(ClientError::config_validation(
                "per_page",
                "per_page must be between 1 and 100",
            ));
        }

        if self.request.timeout.is_zero() {
            return Err(ClientError::config_validation(
                "timeout",
                "Timeout must be greater than zero",
            ));
        }

        Ok(())
    }

    /// Create default project query with config values
    pub fn default_project_query(&self) -> ProjectQuery {
        ProjectQuery {
            search_filter: self.search_filter.clone(),
            per_page: self.request.per_page,
            include_statistics: true,
            archived: false,
            membership: true,
            search_namespaces: true,
            ..Default::default()
        }
    }

    /// Create default pipeline query with config values
    pub fn default_pipeline_query(&self) -> PipelineQuery {
        PipelineQuery {
            per_page: self.request.per_page.min(60), // GitHub API limit for pipelines
            ..Default::default()
        }
    }
}

impl From<GlomConfig> for ClientConfig {
    fn from(config: GlomConfig) -> Self {
        Self::new(config.github_url, config.github_token).with_search_filter(config.search_filter)
    }
}

#[allow(dead_code)]
impl ClientConfig {
    /// Set search filter
    pub fn with_search_filter(mut self, filter: Option<CompactString>) -> Self {
        self.search_filter = filter;
        self
    }

    /// Set polling configuration
    pub fn with_polling(mut self, polling: PollingConfig) -> Self {
        self.polling = polling;
        self
    }

    /// Set request configuration
    pub fn with_request(mut self, request: RequestConfig) -> Self {
        self.request = request;
        self
    }

    /// Set debug configuration
    pub fn with_debug(mut self, debug: DebugConfig) -> Self {
        self.debug = debug;
        self
    }

    /// Enable debug logging
    pub fn with_debug_logging(mut self, enabled: bool) -> Self {
        self.debug.log_responses = enabled;
        self
    }
}

impl ProjectQuery {
    /// Set search filter
    #[allow(dead_code)] // Used in tests and may be used by API
    pub fn with_search_filter(mut self, filter: Option<CompactString>) -> Self {
        self.search_filter = filter;
        self
    }

    /// Set updated after filter
    pub fn with_updated_after(mut self, updated_after: Option<DateTime<Utc>>) -> Self {
        self.updated_after = updated_after;
        self
    }

    /// Set per page limit
    #[allow(dead_code)]
    pub fn with_per_page(mut self, per_page: u32) -> Self {
        self.per_page = per_page;
        self
    }
}

impl PipelineQuery {
    /// Create a new pipeline query
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set updated after filter
    pub fn with_updated_after(mut self, updated_after: Option<DateTime<Utc>>) -> Self {
        self.updated_after = updated_after;
        self
    }

    /// Set per page limit
    #[allow(dead_code)]
    pub fn with_per_page(mut self, per_page: u32) -> Self {
        self.per_page = per_page;
        self
    }
}
