use std::{collections::HashMap, sync::mpsc::Sender};

use chrono::{DateTime, Utc};
use itertools::Itertools;
use tracing::{debug, info, instrument, warn};

use crate::{
    dispatcher::Dispatcher,
    domain::{Job, Pipeline, Project},
    event::GlomEvent,
    id::ProjectId,
};

pub struct ProjectStore {
    sender: Sender<GlomEvent>,
    projects: Vec<Project>,
    project_id_lookup: HashMap<ProjectId, usize>,
    sorted: Vec<Project>, // todo: ref projects
}

impl ProjectStore {
    pub fn new(sender: Sender<GlomEvent>) -> Self {
        Self {
            sender,
            projects: Vec::new(),
            // pipelines: Vec::new(),
            project_id_lookup: HashMap::new(),
            sorted: Vec::new(),
        }
    }

    #[instrument(skip(self, event), fields(event_type = %event.variant_name()))]
    pub fn apply(&mut self, event: &GlomEvent) {
        match event {
            // requests jobs for pipelines that have not been loaded yet and repository statistics
            GlomEvent::ProjectDetailsOpen(id) => {
                debug!(project_id = %id, "Opening project details and requesting missing jobs and statistics");
                let project = self.find(id.clone()).unwrap();
                let project_id = project.id.clone();

                // Fetch jobs for pipelines that haven't been loaded yet
                project
                    .recent_pipelines()
                    .into_iter()
                    .filter(|p| p.jobs.is_none())
                    .for_each(|p| self.dispatch(GlomEvent::JobsFetch(project_id.clone(), p.id)));

                // Fetch repository statistics if they haven't been loaded yet
                if project.commit_count == 0
                    && project.repo_size_kb == 0
                    && project.artifacts_size_kb == 0
                    && !project.statistics_loading
                {
                    // Set loading state
                    let sender = self.sender.clone();
                    if let Some(project_mut) = self.find_mut(project_id.clone()) {
                        project_mut.statistics_loading = true;
                        sender.dispatch(GlomEvent::ProjectUpdated(Box::new(project_mut.clone())));
                    }
                    self.dispatch(GlomEvent::ProjectStatisticsFetch(project_id));
                }
            },

            // updates the projects in the store
            GlomEvent::ProjectsLoaded(projects) => {
                debug!(
                    project_count = projects.len(),
                    "Processing received projects"
                );
                let first_projects = self.sorted.is_empty();
                projects
                    .iter()
                    .map(|p| Project::from(p.clone()))
                    .for_each(|p| {
                        let project = p.clone();
                        self.sync_project(p);
                        let sender = self.sender.clone();
                        sender.dispatch(GlomEvent::ProjectUpdated(Box::new(project)))
                    });

                self.sorted = self.projects_sorted_by_last_activity();
                if first_projects {
                    self.dispatch(GlomEvent::ProjectSelected(
                        self.sorted.first().unwrap().id.clone(),
                    ));
                }
            },

            // updates the pipelines for a project
            GlomEvent::PipelinesLoaded(pipelines) => {
                let project_id = pipelines[0].project_id.clone();
                debug!(project_id = %project_id, pipeline_count = pipelines.len(), "Processing received pipelines");
                let sender = self.sender.clone();

                if let Some(project) = self.find_mut(project_id.clone()) {
                    let pipelines: Vec<Pipeline> = pipelines
                        .iter()
                        .map(|p| Pipeline::from(p.clone()))
                        .collect();

                    pipelines
                        .iter()
                        .filter(|&p| p.status.is_active() || p.has_active_jobs())
                        .for_each(|p| {
                            sender.dispatch(GlomEvent::JobsFetch(project_id.clone(), p.id))
                        });

                    project.update_pipelines(pipelines);
                    sender.dispatch(GlomEvent::ProjectUpdated(Box::new(project.clone())))
                }

                self.sorted = self.projects_sorted_by_last_activity();
            },

            GlomEvent::JobsLoaded(project_id, pipeline_id, job_dtos) => {
                debug!(project_id = %project_id, pipeline_id = %pipeline_id, job_count = job_dtos.len(), "Processing received jobs");
                let jobs: Vec<Job> = job_dtos
                    .iter()
                    .map(|j| Job::from(j.clone()))
                    .collect();

                let sender = self.sender.clone();
                if let Some(project) = self.find_mut(project_id.clone()) {
                    project.update_jobs(*pipeline_id, jobs);
                    // todo: ugly, fix
                    project.update_commit(
                        *pipeline_id,
                        job_dtos
                            .first()
                            .map(|j| j.commit.clone().into())
                            .unwrap(),
                    );
                    sender.dispatch(GlomEvent::ProjectUpdated(Box::new(project.clone())))
                }

                self.sorted = self.projects_sorted_by_last_activity();
            },

            // updates project statistics when loaded
            GlomEvent::ProjectStatisticsLoaded(project_id, statistics) => {
                debug!(project_id = %project_id, "Processing received project statistics");
                let sender = self.sender.clone();
                if let Some(project) = self.find_mut(project_id.clone()) {
                    project.commit_count = statistics.commit_count;
                    project.repo_size_kb = statistics.repository_size / 1024; // Convert bytes to KB
                    project.artifacts_size_kb = statistics.job_artifacts_size / 1024; // Convert bytes to KB
                    project.statistics_loading = false; // Clear loading state

                    sender.dispatch(GlomEvent::ProjectUpdated(Box::new(project.clone())));
                }
            },

            // requests pipelines for a project if they are not already loaded
            GlomEvent::ProjectSelected(id) => {
                debug!(project_id = %id, "Project selected");
                let mut request_pipelines = false;
                if let Some(project) = self.find_mut(id.clone())
                    && project.pipelines.is_none()
                {
                    project.pipelines = Some(Vec::new());
                    request_pipelines = true;
                };

                if request_pipelines {
                    self.dispatch(GlomEvent::PipelinesFetch(id.clone()));
                };
            },
            _ => {},
        }
    }

    fn projects_sorted_by_last_activity(&mut self) -> Vec<Project> {
        self.projects
            .iter()
            .sorted_by(|a, b| b.last_activity().cmp(&a.last_activity()))
            .cloned()
            .collect()
    }

    pub fn find(&self, id: ProjectId) -> Option<&Project> {
        self.project_idx(id)
            .map(|idx| &self.projects[idx])
    }

    pub fn sorted_projects(&self) -> &[Project] {
        &self.sorted
    }

    fn find_mut(&mut self, id: ProjectId) -> Option<&mut Project> {
        self.project_idx(id)
            .map(|idx| &mut self.projects[idx])
    }

    fn project_idx(&self, id: ProjectId) -> Option<usize> {
        self.project_id_lookup.get(&id).copied()
    }

    #[instrument(skip(self, project), fields(project_id = %project.id, project_path = %project.path))]
    fn sync_project(&mut self, mut project: Project) {
        let sender = self.sender.clone();
        let project_id = project.id.clone();
        match self.find_mut(project_id.clone()) {
            Some(existing_entry) => {
                sender.dispatch(GlomEvent::PipelinesFetch(project_id.clone()));
                existing_entry.update_project(project.clone())
            },
            None => {
                self.project_id_lookup
                    .insert(project_id.clone(), self.projects.len());
                if !is_older_than_7d(project.last_activity()) {
                    sender.dispatch(GlomEvent::PipelinesFetch(project_id));
                    project.pipelines = Some(Vec::new());
                }
                self.projects.push(project);
            },
        }
    }
}

fn is_older_than_7d(date: DateTime<Utc>) -> bool {
    Utc::now().signed_duration_since(date).num_days() > 7
}

#[instrument(skip(event))]
pub fn log_event(event: &GlomEvent) {
    match event {
        GlomEvent::ProjectsFetch => info!("Requesting all projects from GitHub"),
        GlomEvent::ProjectsLoaded(projects) => {
            info!(count = projects.len(), "Received projects from GitHub API")
        },
        GlomEvent::ProjectFetch(id) => debug!(project_id = %id, "Refreshing project"),
        GlomEvent::JobsActiveFetch => debug!("Requesting active pipelines for all projects"),
        GlomEvent::PipelinesFetch(id) => {
            debug!(project_id = %id, "Requesting pipelines for project")
        },
        GlomEvent::JobsFetch(project_id, pipeline_id) => {
            debug!(project_id = %project_id, pipeline_id = %pipeline_id, "Requesting jobs")
        },
        GlomEvent::PipelinesLoaded(pipelines) => {
            let project_id = pipelines.first().map(|p| p.project_id.clone());
            debug!(count = pipelines.len(), project_id = ?project_id, "Received pipelines from GitHub API")
        },
        GlomEvent::JobsLoaded(project_id, pipeline_id, jobs) => {
            debug!(project_id = %project_id, pipeline_id = %pipeline_id, count = jobs.len(), "Received jobs")
        },
        GlomEvent::ProjectStatisticsFetch(project_id) => {
            debug!(project_id = %project_id, "Requesting repository statistics")
        },
        GlomEvent::ProjectStatisticsLoaded(project_id, statistics) => {
            debug!(project_id = %project_id, commit_count = statistics.commit_count,
                   repo_size = statistics.repository_size, artifacts_size = statistics.job_artifacts_size,
                   "Received repository statistics")
        },
        GlomEvent::ProjectDetailsOpen(id) => debug!(project_id = %id, "Opening project details"),
        GlomEvent::ProjectDetailsClose => debug!("Closing project details popup"),
        GlomEvent::PipelineActionsOpen(project_id, pipeline_id) => {
            debug!(project_id = %project_id, pipeline_id = %pipeline_id, "Opening pipeline actions")
        },
        GlomEvent::ProjectSelected(id) => debug!(project_id = %id, "Selected project"),
        GlomEvent::PipelineSelected(id) => debug!(pipeline_id = %id, "Selected pipeline"),
        GlomEvent::ProjectOpenUrl(id) => info!(project_id = %id, "Opening project in browser"),
        GlomEvent::PipelineOpenUrl(project_id, pipeline_id) => {
            info!(project_id = %project_id, pipeline_id = %pipeline_id, "Opening pipeline in browser")
        },
        GlomEvent::JobOpenUrl(project_id, pipeline_id, job_id) => {
            info!(project_id = %project_id, pipeline_id = %pipeline_id, job_id = %job_id, "Opening job in browser")
        },
        GlomEvent::JobLogFetch(project_id, pipeline_id) => {
            info!(project_id = %project_id, pipeline_id = %pipeline_id, "Downloading job error log")
        },
        GlomEvent::JobLogDownloaded(project_id, job_id, log_content) => {
            info!(
                project_id = %project_id,
                job_id = %job_id,
                content_length = log_content.len(),
                "Job log downloaded successfully"
            )
        },
        GlomEvent::ConfigOpen => debug!("Displaying configuration"),
        GlomEvent::ConfigApply => info!("Applying new configuration"),
        GlomEvent::ConfigUpdate(_) => debug!("Updating configuration"),
        GlomEvent::ApplyTemporaryFilter(filter) => {
            debug!(filter = ?filter, "Applying temporary filter")
        },
        GlomEvent::FilterClear => info!("Clearing project filter"),
        GlomEvent::FilterMenuClose => debug!("Closing filter input"),
        GlomEvent::AppExit => info!("Application shutting down"),
        GlomEvent::AppError(err) => {
            warn!(error = %err, error_type = ?std::mem::discriminant(err), "Application error occurred")
        },
        GlomEvent::LogEntry(_msg) => {}, // Don't log LogEntry events to prevent infinite loop
        _ => {},                         // Don't log every event
    }
}

impl Dispatcher for ProjectStore {
    fn dispatch(&self, event: GlomEvent) {
        self.sender.send(event).unwrap();
    }
}
