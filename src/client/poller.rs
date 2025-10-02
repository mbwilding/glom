//! Background polling for GitHub resources

use std::{sync::Arc, time::Duration};

use tokio::{sync::broadcast, time::sleep};
use tracing::{debug, error, info, instrument};

use super::{api::GithubApi, config::PollingConfig, service::GithubService};
use crate::{dispatcher::Dispatcher, event::GlomEvent};

/// Background poller for GitHub resources
///
/// Manages periodic fetching of projects and active jobs with configurable intervals
#[derive(Debug)]
#[allow(dead_code)]
pub struct GithubPoller {
    api: Arc<GithubApi>,
    sender: std::sync::mpsc::Sender<GlomEvent>,
    config: PollingConfig,
    shutdown_tx: broadcast::Sender<()>,
    shutdown_rx: broadcast::Receiver<()>,
}

#[allow(dead_code)]
impl GithubPoller {
    /// Create a new GitHub poller
    pub fn new(
        api: Arc<GithubApi>,
        sender: std::sync::mpsc::Sender<GlomEvent>,
        config: PollingConfig,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        Self { api, sender, config, shutdown_tx, shutdown_rx }
    }

    /// Start polling in the background
    ///
    /// This will spawn two separate async tasks:
    /// - One for polling projects at the configured interval
    /// - One for polling active jobs at the configured interval
    #[instrument(skip(self))]
    pub async fn start(mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            projects_interval = ?self.config.projects_interval,
            jobs_interval = ?self.config.jobs_interval,
            "Starting GitHub poller"
        );

        // Spawn projects polling task
        let projects_task = {
            let api = Arc::clone(&self.api);
            let sender = self.sender.clone();
            let interval = self.config.projects_interval;
            let mut shutdown_rx = self.shutdown_tx.subscribe();

            tokio::spawn(async move {
                Self::poll_projects(api, sender, interval, &mut shutdown_rx).await;
            })
        };

        // Spawn jobs polling task
        let jobs_task = {
            let api = Arc::clone(&self.api);
            let sender = self.sender.clone();
            let interval = self.config.jobs_interval;
            let mut shutdown_rx = self.shutdown_tx.subscribe();

            tokio::spawn(async move {
                Self::poll_active_jobs(api, sender, interval, &mut shutdown_rx).await;
            })
        };

        // Wait for shutdown signal
        let _ = self.shutdown_rx.recv().await;

        info!("Shutting down GitHub poller");

        // Cancel polling tasks
        projects_task.abort();
        jobs_task.abort();

        // Wait a bit for graceful shutdown
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Send shutdown signal to stop polling
    pub fn shutdown(&self) {
        debug!("Sending shutdown signal to GitHub poller");
        let _ = self.shutdown_tx.send(());
    }

    /// Get a shutdown sender for external shutdown control
    pub fn shutdown_sender(&self) -> broadcast::Sender<()> {
        self.shutdown_tx.clone()
    }

    /// Update polling configuration
    pub fn update_config(&mut self, config: PollingConfig) {
        self.config = config;
    }

    /// Get current polling configuration
    pub fn config(&self) -> &PollingConfig {
        &self.config
    }

    // Private polling implementations

    /// Poll projects at regular intervals
    #[instrument(skip(api, sender, shutdown_rx), fields(interval = ?interval))]
    async fn poll_projects(
        api: Arc<GithubApi>,
        sender: std::sync::mpsc::Sender<GlomEvent>,
        interval: Duration,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        debug!("Starting projects polling loop");

        loop {
            tokio::select! {
                _ = sleep(interval) => {
                    debug!("Polling projects");
                    let service = GithubService::from_api(api.clone(), sender.clone()).unwrap();
                    service.spawn_fetch_projects(None);
                }
                _ = shutdown_rx.recv() => {
                    debug!("Projects polling received shutdown signal");
                    break;
                }
            }
        }

        debug!("Projects polling loop ended");
    }

    /// Poll active jobs at regular intervals
    #[instrument(skip(_api, sender, shutdown_rx), fields(interval = ?interval))]
    async fn poll_active_jobs(
        _api: Arc<GithubApi>,
        sender: std::sync::mpsc::Sender<GlomEvent>,
        interval: Duration,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        debug!("Starting active jobs polling loop");

        loop {
            tokio::select! {
                _ = sleep(interval) => {
                    debug!("Requesting active jobs refresh");
                    // Dispatch event to request active jobs refresh
                    // The main application will handle which jobs to fetch
                    sender.dispatch(GlomEvent::JobsActiveFetch);
                }
                _ = shutdown_rx.recv() => {
                    debug!("Active jobs polling received shutdown signal");
                    break;
                }
            }
        }

        debug!("Active jobs polling loop ended");
    }
}

/// Builder for GithubPoller with fluent API
#[derive(Debug)]
#[allow(dead_code)]
pub struct GithubPollerBuilder {
    api: Option<Arc<GithubApi>>,
    sender: Option<std::sync::mpsc::Sender<GlomEvent>>,
    config: PollingConfig,
}

#[allow(dead_code)]
impl GithubPollerBuilder {
    /// Create a new poller builder
    pub fn new() -> Self {
        Self {
            api: None,
            sender: None,
            config: PollingConfig::default(),
        }
    }

    /// Set the GitHub API
    pub fn api(mut self, api: Arc<GithubApi>) -> Self {
        self.api = Some(api);
        self
    }

    /// Set the event sender
    pub fn sender(mut self, sender: std::sync::mpsc::Sender<GlomEvent>) -> Self {
        self.sender = Some(sender);
        self
    }

    /// Set polling configuration
    pub fn config(mut self, config: PollingConfig) -> Self {
        self.config = config;
        self
    }

    /// Set projects polling interval
    pub fn projects_interval(mut self, interval: Duration) -> Self {
        self.config.projects_interval = interval;
        self
    }

    /// Set jobs polling interval
    pub fn jobs_interval(mut self, interval: Duration) -> Self {
        self.config.jobs_interval = interval;
        self
    }

    /// Build the GitHub poller
    pub fn build(self) -> Result<GithubPoller, String> {
        let api = self.api.ok_or("GitHub API is required")?;
        let sender = self.sender.ok_or("Event sender is required")?;
        Ok(GithubPoller::new(api, sender, self.config))
    }
}

impl Default for GithubPollerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawn a GitHub poller as a background task
///
/// This is a convenience function for quickly starting background polling
#[allow(dead_code)]
pub async fn spawn_poller(
    api: Arc<GithubApi>,
    sender: std::sync::mpsc::Sender<GlomEvent>,
    config: PollingConfig,
) -> broadcast::Sender<()> {
    let poller = GithubPoller::new(api, sender, config);
    let shutdown_sender = poller.shutdown_sender();

    tokio::spawn(async move {
        if let Err(e) = poller.start().await {
            error!("GitHub poller failed: {}", e);
        }
    });

    shutdown_sender
}
