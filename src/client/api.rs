//! Core HTTP client for GitHub API

use std::sync::RwLock;

use chrono::Local;
use compact_str::{CompactString, format_compact};
use reqwest::{Client, RequestBuilder, Response};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::{
    config::{ClientConfig, PipelineQuery, ProjectQuery},
    error::{ClientError, Result},
};
use crate::{
    domain::{
        ContributorDto, GitHubArtifactsResponse, GitHubJobsResponse, GitHubSearchResponse,
        GitHubWorkflowRunsResponse, JobDto, PipelineDto, ProjectDto, RepositoryDetailsDto,
        StatisticsDto,
    },
    id::{JobId, PipelineId, ProjectId},
};

/// Pure HTTP client for GitHub API
#[derive(Debug)]
pub struct GithubApi {
    client: RwLock<Client>,
    config: RwLock<ClientConfig>,
}

/// GitHub API error response formats
#[derive(Debug, Deserialize)]
struct GithubApiError {
    error: CompactString,
    error_description: Option<CompactString>,
}

#[derive(Debug, Deserialize)]
struct GithubApiError2 {
    message: CompactString,
}

impl GithubApi {
    pub fn force_new(config: ClientConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.request.timeout)
            .build()
            .map_err(ClientError::Http)?;

        Ok(Self {
            client: RwLock::new(client),
            config: RwLock::new(config),
        })
    }

    /// Get projects from GitHub API
    #[instrument(skip(self), fields(per_page = %query.per_page))]
    pub async fn get_projects(&self, query: &ProjectQuery) -> Result<Vec<ProjectDto>> {
        let url = self.build_projects_url(query);

        // Determine if we're using search API based on the URL, not just the filter presence
        if url.contains("/search/repositories") {
            // Search API returns wrapped response
            let response: GitHubSearchResponse<ProjectDto> = self.get_json(&url).await?;
            Ok(response.items)
        } else {
            // Direct user repos API returns array of repositories
            let repos: Vec<ProjectDto> = self.get_json(&url).await?;
            Ok(repos)
        }
    }

    /// Get pipelines for a project
    #[instrument(skip(self), fields(project_id = %project_id, per_page = %query.per_page))]
    pub async fn get_pipelines(
        &self,
        project_id: ProjectId,
        query: &PipelineQuery,
    ) -> Result<Vec<PipelineDto>> {
        let url = self.build_pipelines_url(project_id.clone(), query);
        let mut response: GitHubWorkflowRunsResponse = self.get_json(&url).await?;

        // Set project_id for each workflow run since GitHub doesn't include it
        for pipeline in &mut response.workflow_runs {
            pipeline.project_id = project_id.clone();
        }

        Ok(response.workflow_runs)
    }

    /// Get jobs for a workflow run
    #[instrument(skip(self), fields(project_id = %project_id, pipeline_id = %pipeline_id))]
    pub async fn get_jobs(
        &self,
        project_id: ProjectId,
        pipeline_id: PipelineId,
    ) -> Result<Vec<JobDto>> {
        let url = {
            let config = self.config.read().unwrap();
            // For GitHub, project_id should represent repo path "owner/repo"
            format_compact!(
                "{}/repos/{}/actions/runs/{}/jobs",
                config.base_url,
                project_id,
                pipeline_id
            )
        };

        let response: GitHubJobsResponse = self.get_json(&url).await?;
        let mut jobs = response.jobs;
        jobs.sort_by_key(|job| job.id);
        debug!(job_count = jobs.len(), "Successfully fetched jobs");
        Ok(jobs)
    }

    /// Get job logs
    #[instrument(skip(self), fields(project_id = %project_id, job_id = %job_id))]
    pub async fn get_job_trace(
        &self,
        project_id: ProjectId,
        job_id: JobId,
    ) -> Result<CompactString> {
        let url = {
            let config = self.config.read().unwrap();
            format_compact!(
                "{}/repos/{}/actions/jobs/{}/logs",
                config.base_url,
                project_id,
                job_id
            )
        };

        let response = self.authenticated_request(&url).send().await?;
        let body = response.text().await?;
        Ok(body.into())
    }

    /// Get repository statistics (size, commit count, etc.)
    #[instrument(skip(self), fields(project_id = %project_id))]
    pub async fn get_repository_statistics(&self, project_id: ProjectId) -> Result<StatisticsDto> {
        let repo_url = {
            let config = self.config.read().unwrap();
            format_compact!("{}/repos/{}", config.base_url, project_id)
        };

        let repo_details: RepositoryDetailsDto = self.get_json(&repo_url).await?;

        let commit_count = self
            .get_commit_count(project_id.clone())
            .await
            .unwrap_or(0);

        let artifacts_size = self
            .get_total_artifacts_size(project_id.clone())
            .await
            .unwrap_or(0);

        Ok(StatisticsDto {
            commit_count,
            repository_size: repo_details.size * 1024, // GitHub returns size in KB
            job_artifacts_size: artifacts_size,
        })
    }

    /// Get total size of all artifacts across workflow runs
    async fn get_total_artifacts_size(&self, project_id: ProjectId) -> Result<u64> {
        let url = {
            let config = self.config.read().unwrap();
            format_compact!(
                "{}/repos/{}/actions/artifacts?per_page=100",
                config.base_url,
                project_id
            )
        };

        match self
            .get_json::<GitHubArtifactsResponse>(&url)
            .await
        {
            Ok(response) => {
                let total_size: u64 = response
                    .artifacts
                    .iter()
                    .map(|a| a.size_in_bytes)
                    .sum();
                debug!(
                    project_id = %project_id,
                    artifact_count = response.artifacts.len(),
                    total_size = total_size,
                    "Successfully fetched artifacts"
                );
                Ok(total_size)
            },
            Err(e) => {
                debug!(
                    project_id = %project_id,
                    error = %e,
                    "Failed to fetch artifacts, returning 0"
                );
                Ok(0)
            },
        }
    }

    /// Get commit count using GitHub's contributors API
    async fn get_commit_count(&self, project_id: ProjectId) -> Result<u32> {
        let url = {
            let config = self.config.read().unwrap();
            format_compact!(
                "{}/repos/{}/contributors?per_page=100",
                config.base_url,
                project_id
            )
        };

        match self.get_json::<Vec<ContributorDto>>(&url).await {
            Ok(contributors) => {
                let total_commits: u32 = contributors.iter().map(|c| c.contributions).sum();
                debug!(
                    project_id = %project_id,
                    contributor_count = contributors.len(),
                    total_commits = total_commits,
                    "Successfully fetched commit count from contributors API"
                );
                Ok(total_commits)
            },
            Err(e) => {
                debug!(
                    project_id = %project_id,
                    error = %e,
                    "Failed to fetch contributors, trying fallback method"
                );
                self.get_commit_count_fallback(project_id).await
            },
        }
    }

    /// Fallback method to estimate commit count from recent commits
    async fn get_commit_count_fallback(&self, project_id: ProjectId) -> Result<u32> {
        let url = {
            let config = self.config.read().unwrap();
            format_compact!(
                "{}/repos/{}/commits?per_page=100",
                config.base_url,
                project_id
            )
        };

        match self
            .get_json::<Vec<serde_json::Value>>(&url)
            .await
        {
            Ok(commits) => {
                // This gives us at least the count of recent commits
                // For repositories with more than 100 commits, this will be an underestimate
                // but it's better than 0
                let commit_count = commits.len() as u32;
                debug!(
                    project_id = %project_id,
                    commit_count = commit_count,
                    "Fallback: fetched recent commits count"
                );
                Ok(commit_count)
            },
            Err(e) => {
                debug!(
                    project_id = %project_id,
                    error = %e,
                    "Fallback method failed, returning 0"
                );
                Ok(0)
            },
        }
    }

    /// Update configuration
    pub fn update_config(&self, config: ClientConfig) -> Result<()> {
        config.validate()?;

        let client = Client::builder()
            .timeout(config.request.timeout)
            .build()
            .map_err(ClientError::Http)?;

        *self.config.write().unwrap() = config;
        *self.client.write().unwrap() = client;

        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> ClientConfig {
        self.config.read().unwrap().clone()
    }

    pub fn is_configured(&self) -> bool {
        self.config
            .read()
            .map(|c| c.validate().is_ok())
            .unwrap_or(false)
    }

    /// Perform authenticated GET request and deserialize JSON response
    async fn get_json<T>(&self, url: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = self.authenticated_request(url).send().await?;
        self.handle_response(response).await
    }

    /// Create authenticated request builder
    fn authenticated_request(&self, url: &str) -> RequestBuilder {
        let client = self.client.read().unwrap();
        let private_token = self.config.read().unwrap().private_token.clone();
        client
            .get(url)
            .header("Authorization", format!("token {}", private_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "glom-github-client")
    }

    /// Handle HTTP response and deserialize JSON
    async fn handle_response<T>(&self, response: Response) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url_path = response.url().path().to_string();
        let status = response.status();
        let body = response.text().await?;

        // Log response if debug is enabled
        {
            let config = self.config.read().unwrap();
            if config.debug.log_responses {
                self.log_response_to_file(&url_path, &body, &config);
            }
        }

        if status.is_success() {
            serde_json::from_str(&body).map_err(|e| {
                // Log the problematic JSON for debugging
                eprintln!("JSON Parse Error for {}: {}", url_path, e);
                eprintln!("Response body: {}", body);
                ClientError::json_parse(url_path, "Failed to parse response", e)
            })
        } else {
            self.handle_error_response(status.as_u16(), &body)
        }
    }

    /// Handle error responses from GitHub API
    fn handle_error_response<T>(&self, status: u16, body: &str) -> Result<T> {
        match status {
            401 => {
                // Try to parse GitHub API error to distinguish between invalid and expired tokens
                if let Ok(api_error) = serde_json::from_str::<GithubApiError>(body) {
                    match api_error.error.as_str() {
                        "invalid_token" => Err(ClientError::InvalidToken),
                        "expired_token" => Err(ClientError::ExpiredToken),
                        _ => {
                            // Check error description for expiration indicators
                            if let Some(description) = &api_error.error_description
                                && (description.contains("expired")
                                    || description.contains("expiry"))
                            {
                                return Err(ClientError::ExpiredToken);
                            }
                            Err(ClientError::Authentication)
                        },
                    }
                } else {
                    Err(ClientError::Authentication)
                }
            },
            404 => Err(ClientError::not_found("Resource")),
            422 => {
                if let Ok(api_error) = serde_json::from_str::<GithubApiError2>(body) {
                    Err(ClientError::github_api(format_compact!(
                        "Validation Failed: {}",
                        api_error.message
                    )))
                } else {
                    Err(ClientError::github_api(format_compact!(
                        "Validation Failed: {}",
                        body
                    )))
                }
            },
            429 => Err(ClientError::rate_limit(None)),
            _ => {
                if let Ok(api_error) = serde_json::from_str::<GithubApiError>(body) {
                    Err(ClientError::github_api(format_compact!(
                        "HTTP {}: {} {}",
                        status,
                        api_error.error,
                        api_error.error_description.unwrap_or_default()
                    )))
                } else if let Ok(api_error2) = serde_json::from_str::<GithubApiError2>(body) {
                    Err(ClientError::github_api(format_compact!(
                        "HTTP {}: {}",
                        status,
                        api_error2.message
                    )))
                } else {
                    Err(ClientError::github_api(format_compact!(
                        "HTTP {}: {}",
                        status,
                        body
                    )))
                }
            },
        }
    }

    /// Build URL for repositories search/list endpoint
    fn build_projects_url(&self, query: &ProjectQuery) -> CompactString {
        let config = self.config.read().unwrap();

        // For simple search terms or no filter, use the user repos API for reliability
        // GitHub's search API is complex and often fails with simple terms
        if query.search_filter.is_none()
            || query
                .search_filter
                .as_ref()
                .is_some_and(|f| !f.contains(':') && !f.contains(' '))
        {
            format_compact!(
                "{}/user/repos?type=all&sort=updated&direction=desc&per_page={}",
                config.base_url,
                query.per_page
            )
        } else {
            // Only use search API for complex queries with GitHub search syntax
            let mut url = format_compact!("{}/search/repositories?q=", config.base_url);

            if let Some(filter) = &query.search_filter {
                // Complex query, use as-is
                url.push_str(&format_compact!("{}", filter));
            }

            url.push_str(" user:@me");
            url.push_str("&sort=updated&order=desc");
            url.push_str(&format_compact!("&per_page={}", query.per_page));

            url
        }
    }

    /// Build URL for workflow runs endpoint
    fn build_pipelines_url(&self, project_id: ProjectId, query: &PipelineQuery) -> CompactString {
        let config = self.config.read().unwrap();
        let mut url = format_compact!(
            "{}/repos/{}/actions/runs?per_page={}",
            config.base_url,
            project_id,
            query.per_page
        );

        if let Some(updated_after) = query.updated_after {
            url.push_str(&format_compact!(
                "&created=>={}",
                updated_after.format("%Y-%m-%dT%H:%M:%SZ")
            ));
        }

        url
    }

    /// Log HTTP response to file for debugging
    fn log_response_to_file(&self, path: &str, body: &str, config: &ClientConfig) {
        if let Some(log_dir) = &config.debug.log_directory {
            if !log_dir.exists()
                && let Err(e) = std::fs::create_dir_all(log_dir)
            {
                warn!("Failed to create log directory: {}", e);
                return;
            }

            let filename = format!(
                "{}_{}.json",
                Local::now().format("%Y-%m-%d_%H-%M-%S"),
                path.replace('/', "_")
            );

            let log_path = log_dir.join(filename);

            if let Err(e) = std::fs::write(&log_path, body) {
                warn!("Failed to write response log to {:?}: {}", log_path, e);
            } else {
                debug!("Response logged to {:?}", log_path);
            }
        }
    }
}
