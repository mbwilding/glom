//! High-level GitHub service operations

use std::sync::{Arc, mpsc::Sender};

use chrono::{DateTime, Utc};
use tokio::runtime::Handle;
use tracing::{debug, error, info, instrument, warn};

use super::{
    api::GithubApi,
    config::ClientConfig,
    error::{ClientError, Result},
};
use crate::{
    dispatcher::Dispatcher,
    event::{GlomEvent, IntoGlomEvent},
    id::{JobId, PipelineId, ProjectId},
    result::GlomError::{self, GeneralError},
};

/// High-level service for GitHub operations
///
/// Orchestrates API calls and handles event dispatching to the application
#[derive(Debug)]
pub struct GithubService {
    api: Arc<GithubApi>,
    sender: Sender<GlomEvent>,
    handle: Handle,
}

impl GithubService {
    /// Create service from existing API client
    pub fn from_api(api: Arc<GithubApi>, sender: Sender<GlomEvent>) -> Result<Self> {
        let handle = Handle::try_current().map_err(|_| {
            ClientError::config("GithubService must be created within a Tokio runtime context")
        })?;
        Ok(Self { api, sender, handle })
    }

    /// Fetch projects and dispatch results as events
    #[instrument(skip(self), fields(updated_after = ?updated_after))]
    pub async fn fetch_projects(&self, updated_after: Option<DateTime<Utc>>) -> Result<()> {
        if !self.api.is_configured() {
            return Ok(());
        }

        info!("Fetching projects from GitHub");

        let query = self
            .api
            .config()
            .default_project_query()
            .with_updated_after(updated_after);

        match self.api.get_projects(&query).await {
            Ok(projects) => {
                debug!(
                    project_count = projects.len(),
                    "Successfully fetched projects"
                );
                self.sender.dispatch(projects.into_glom_event());
                Ok(())
            },
            Err(e) => {
                error!(error = %e, "Failed to fetch projects");
                let glom_error = crate::result::GlomError::from(&e);
                self.sender
                    .dispatch(GlomEvent::AppError(glom_error));
                Err(e)
            },
        }
    }

    /// Fetch pipelines for a project and dispatch results as events
    #[instrument(skip(self), fields(project_id = %project_id, updated_after = ?updated_after))]
    pub async fn fetch_pipelines(
        &self,
        project_id: ProjectId,
        updated_after: Option<DateTime<Utc>>,
    ) -> Result<()> {
        if !self.api.is_configured() {
            return Ok(());
        }

        let query = self
            .api
            .config()
            .default_pipeline_query()
            .with_updated_after(updated_after);

        match self
            .api
            .get_pipelines(project_id.clone(), &query)
            .await
        {
            Ok(pipelines) => {
                debug!(
                    pipeline_count = pipelines.len(),
                    project_id = %project_id,
                    "Successfully fetched pipelines"
                );
                self.sender.dispatch(pipelines.into_glom_event());
                Ok(())
            },
            Err(e) => {
                error!(
                    error = %e,
                    project_id = %project_id,
                    "Failed to fetch pipelines"
                );
                let glom_error = crate::result::GlomError::from(&e);
                self.sender
                    .dispatch(GlomEvent::AppError(glom_error));
                Err(e)
            },
        }
    }

    /// Fetch all jobs (regular + trigger jobs) for a pipeline and dispatch results
    #[instrument(skip(self), fields(project_id = %project_id, pipeline_id = %pipeline_id))]
    pub async fn fetch_all_jobs(
        &self,
        project_id: ProjectId,
        pipeline_id: PipelineId,
    ) -> Result<()> {
        if !self.api.is_configured() {
            return Ok(());
        }

        match self
            .api
            .get_jobs(project_id.clone(), pipeline_id)
            .await
        {
            Ok(jobs) => {
                debug!(
                    job_count = jobs.len(),
                    project_id = %project_id,
                    pipeline_id = %pipeline_id,
                    "Successfully fetched jobs"
                );
                self.sender
                    .dispatch((project_id, pipeline_id, jobs).into_glom_event());
                Ok(())
            },
            Err(e) => {
                error!(
                    error = %e,
                    project_id = %project_id,
                    pipeline_id = %pipeline_id,
                    "Failed to fetch jobs"
                );
                let glom_error = crate::result::GlomError::from(&e);
                self.sender
                    .dispatch(GlomEvent::AppError(glom_error));
                Err(e)
            },
        }
    }

    /// Download job log and dispatch results
    #[instrument(skip(self), fields(project_id = %project_id, job_id = %job_id))]
    pub async fn download_job_log(&self, project_id: ProjectId, job_id: JobId) -> Result<()> {
        if !self.api.is_configured() {
            return Ok(());
        }

        info!("Downloading job log from GitHub");

        match self
            .api
            .get_job_trace(project_id.clone(), job_id)
            .await
        {
            Ok(trace) => {
                info!(
                    project_id = %project_id,
                    job_id = %job_id,
                    trace_length = trace.len(),
                    "Successfully downloaded job log"
                );
                self.sender
                    .dispatch(GlomEvent::JobLogDownloaded(project_id, job_id, trace));
                Ok(())
            },
            Err(e) => {
                error!(
                    error = %e,
                    project_id = %project_id,
                    job_id = %job_id,
                    "Failed to download job log"
                );
                let glom_error = crate::result::GlomError::from(&e);
                self.sender
                    .dispatch(GlomEvent::AppError(glom_error));
                Err(e)
            },
        }
    }

    /// Fetch repository statistics and dispatch results as events
    #[instrument(skip(self), fields(project_id = %project_id))]
    pub async fn fetch_repository_statistics(&self, project_id: ProjectId) -> Result<()> {
        if !self.api.is_configured() {
            return Ok(());
        }

        info!(project_id = %project_id, "Fetching repository statistics from GitHub");

        match self
            .api
            .get_repository_statistics(project_id.clone())
            .await
        {
            Ok(statistics) => {
                debug!(
                    project_id = %project_id,
                    commit_count = statistics.commit_count,
                    repo_size = statistics.repository_size,
                    artifacts_size = statistics.job_artifacts_size,
                    "Successfully fetched repository statistics"
                );
                self.sender
                    .dispatch(GlomEvent::ProjectStatisticsLoaded(project_id, statistics));
                Ok(())
            },
            Err(e) => {
                error!(
                    error = %e,
                    project_id = %project_id,
                    "Failed to fetch repository statistics"
                );
                let glom_error = crate::result::GlomError::from(&e);
                self.sender
                    .dispatch(GlomEvent::AppError(glom_error));
                Err(e)
            },
        }
    }

    /// Update service configuration
    pub fn update_config(&self, config: ClientConfig) -> Result<()> {
        self.api.update_config(config)
    }

    /// Get current configuration
    pub fn config(&self) -> ClientConfig {
        self.api.config()
    }

    /// Get reference to the underlying API client
    #[allow(dead_code)]
    pub fn api(&self) -> &GithubApi {
        &self.api
    }

    /// Spawn an async task to fetch projects
    ///
    /// This is a convenience method for fire-and-forget project fetching
    pub fn spawn_fetch_projects(&self, updated_after: Option<DateTime<Utc>>) {
        let api = self.api.clone();
        let sender = self.sender.clone();
        self.handle.spawn(async move {
            let temp_service = Self::from_api(api, sender).unwrap();
            if let Err(e) = temp_service.fetch_projects(updated_after).await {
                warn!("Background project fetch failed: {}", e);
            }
        });
    }

    /// Spawn an async task to fetch pipelines
    pub fn spawn_fetch_pipelines(
        &self,
        project_id: ProjectId,
        updated_after: Option<DateTime<Utc>>,
    ) {
        let api = self.api.clone();
        let sender = self.sender.clone();
        self.handle.spawn(async move {
            let temp_service = Self::from_api(api, sender).unwrap();
            if let Err(e) = temp_service
                .fetch_pipelines(project_id, updated_after)
                .await
            {
                warn!("Background pipeline fetch failed: {}", e);
            }
        });
    }

    /// Spawn an async task to fetch jobs
    pub fn spawn_fetch_jobs(&self, project_id: ProjectId, pipeline_id: PipelineId) {
        let api = self.api.clone();
        let sender = self.sender.clone();
        self.handle.spawn(async move {
            let temp_service = Self::from_api(api, sender).unwrap();
            if let Err(e) = temp_service
                .fetch_all_jobs(project_id, pipeline_id)
                .await
            {
                warn!("Background job fetch failed: {}", e);
            }
        });
    }

    /// Spawn an async task to download job log
    pub fn spawn_download_job_log(&self, project_id: ProjectId, job_id: JobId) {
        let api = self.api.clone();
        let sender = self.sender.clone();
        self.handle.spawn(async move {
            let temp_service = Self::from_api(api, sender).unwrap();
            if let Err(e) = temp_service
                .download_job_log(project_id, job_id)
                .await
            {
                warn!("Background job log download failed: {}", e);
            }
        });
    }

    /// Spawn an async task to fetch repository statistics
    pub fn spawn_fetch_repository_statistics(&self, project_id: ProjectId) {
        let api = self.api.clone();
        let sender = self.sender.clone();
        self.handle.spawn(async move {
            let temp_service = Self::from_api(api, sender).unwrap();
            if let Err(e) = temp_service
                .fetch_repository_statistics(project_id)
                .await
            {
                warn!("Background repository statistics fetch failed: {}", e);
            }
        });
    }
}

// Convert ClientError to the application's GlomError type
impl From<&ClientError> for crate::result::GlomError {
    fn from(err: &ClientError) -> Self {
        match err {
            ClientError::Http(e) => GeneralError(format!("HTTP error: {e}").into()),
            ClientError::JsonParse { endpoint, message, .. } => {
                GeneralError(format!("JSON parse error from {endpoint}: {message}").into())
            },
            ClientError::GithubApi { message } => GeneralError(message.clone()),
            ClientError::Config(msg) => GeneralError(msg.into()),
            ClientError::ConfigValidation { field, message } => {
                GlomError::config_validation_error(field, message)
            },
            ClientError::Authentication => GeneralError("Authentication failed".into()),
            ClientError::InvalidToken => GlomError::InvalidGithubToken,
            ClientError::ExpiredToken => GlomError::ExpiredGithubToken,
            ClientError::Timeout => GeneralError("Request timeout".into()),
            ClientError::InvalidUrl { url } => GeneralError(format!("Invalid URL: {url}").into()),
            ClientError::NotFound { resource } => {
                GeneralError(format!("Not found: {resource}").into())
            },
            ClientError::RateLimit { .. } => GeneralError("Rate limit exceeded".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::client::config::ClientConfig;

    impl GithubService {
        /// Create a new GitHub service
        pub fn new(config: ClientConfig, sender: Sender<GlomEvent>) -> Result<Self> {
            let api = Arc::new(GithubApi::new(config)?);
            let handle = Handle::try_current().map_err(|_| {
                ClientError::config("GithubService must be created within a Tokio runtime context")
            })?;

            Ok(Self { api, sender, handle })
        }
    }

    fn test_config() -> ClientConfig {
        ClientConfig::new(
            "https://api.github.com",
            "ghp_1234567890123456789012345678901234567890",
        )
    }

    #[tokio::test]
    async fn test_service_creation() {
        let config = test_config();
        let (sender, _receiver) = mpsc::channel();
        let service = GithubService::new(config, sender);
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_service_creation_invalid_config() {
        let config = ClientConfig::new("", "test-token");
        let (sender, _receiver) = mpsc::channel();
        let service = GithubService::new(config, sender);
        assert!(service.is_err());
    }

    #[tokio::test]
    async fn test_config_access() {
        let config = test_config();
        let (sender, _receiver) = mpsc::channel();
        let service = GithubService::new(config.clone(), sender).unwrap();

        assert_eq!(service.config().base_url, config.base_url);
        assert_eq!(service.config().private_token, config.private_token);
    }

    #[tokio::test]
    async fn test_error_conversion() {
        let client_error = ClientError::config("Test error");
        let glom_error: crate::result::GlomError = (&client_error).into();

        match glom_error {
            GeneralError(msg) => {
                assert!(msg.contains("Test error"));
            },
            _ => panic!("Expected GeneralError"),
        }
    }

    // Note: Integration tests with actual API calls would require
    // a test GitHub instance or mocked HTTP responses
}
