use std::{fmt::Debug, sync::mpsc, thread};

use compact_str::CompactString;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use tracing::Level;

use crate::{
    dispatcher::Dispatcher,
    domain::{JobDto, PipelineDto, Project, ProjectDto, StatisticsDto},
    glom_app::GlomConfig,
    id::{JobId, PipelineId, ProjectId},
    result,
};

#[derive(Debug, Clone)]
pub enum GlomEvent {
    AppError(result::GlomError),
    AppExit,
    AppTick,
    ApplyTemporaryFilter(Option<CompactString>),
    ConfigApply,
    ConfigClose,
    ConfigOpen,
    ConfigUpdate(GlomConfig),
    FilterClear,
    FilterInputBackspace,
    FilterInputChar(CompactString),
    FilterMenuClose,
    FilterMenuShow,
    #[allow(dead_code)]
    GlitchOverride(GlitchState),
    InputKey(KeyEvent),
    JobLogDownloaded(ProjectId, JobId, CompactString),
    JobLogFetch(ProjectId, PipelineId),
    JobOpenUrl(ProjectId, PipelineId, JobId),
    JobsActiveFetch,
    JobsFetch(ProjectId, PipelineId),
    JobsLoaded(ProjectId, PipelineId, Vec<JobDto>),
    LogEntry(CompactString),
    LogLevelChanged(Level),
    NotificationDismiss,
    NotificationLast,
    PipelineActionsClose,
    PipelineActionsOpen(ProjectId, PipelineId),
    PipelineOpenUrl(ProjectId, PipelineId),
    PipelineSelected(PipelineId),
    PipelinesFetch(ProjectId),
    PipelinesLoaded(Vec<PipelineDto>),
    ProjectDetailsClose,
    ProjectDetailsOpen(ProjectId),
    #[allow(dead_code)]
    ProjectFetch(ProjectId),
    ProjectNext,
    ProjectOpenUrl(ProjectId),
    ProjectPrevious,
    ProjectSelected(ProjectId),
    ProjectUpdated(Box<Project>),
    ProjectsFetch,
    ProjectsLoaded(Vec<ProjectDto>),
    ProjectStatisticsFetch(ProjectId),
    ProjectStatisticsLoaded(ProjectId, StatisticsDto),
    ScreenCapture,
    ScreenCaptureToClipboard(String),
}

impl GlomEvent {
    /// Get the variant name as a string slice (without "GlomEvent::" prefix)
    pub fn variant_name(&self) -> &'static str {
        match self {
            GlomEvent::AppError(_) => "AppError",
            GlomEvent::AppExit => "AppExit",
            GlomEvent::AppTick => "AppTick",
            GlomEvent::ApplyTemporaryFilter(_) => "ApplyTemporaryFilter",
            GlomEvent::ConfigApply => "ConfigApply",
            GlomEvent::ConfigClose => "ConfigClose",
            GlomEvent::ConfigOpen => "ConfigOpen",
            GlomEvent::ConfigUpdate(_) => "ConfigUpdate",
            GlomEvent::FilterClear => "FilterClear",
            GlomEvent::FilterInputBackspace => "FilterInputBackspace",
            GlomEvent::FilterInputChar(_) => "FilterInputChar",
            GlomEvent::FilterMenuClose => "FilterMenuClose",
            GlomEvent::FilterMenuShow => "FilterMenuShow",
            GlomEvent::GlitchOverride(_) => "GlitchOverride",
            GlomEvent::InputKey(_) => "InputKey",
            GlomEvent::JobLogDownloaded(_, _, _) => "JobLogDownloaded",
            GlomEvent::JobLogFetch(_, _) => "JobLogFetch",
            GlomEvent::JobOpenUrl(_, _, _) => "JobOpenUrl",
            GlomEvent::JobsActiveFetch => "JobsActiveFetch",
            GlomEvent::JobsFetch(_, _) => "JobsFetch",
            GlomEvent::JobsLoaded(_, _, _) => "JobsLoaded",
            GlomEvent::LogEntry(_) => "LogEntry",
            GlomEvent::LogLevelChanged(_) => "LogLevelChanged",
            GlomEvent::NotificationDismiss => "NotificationDismiss",
            GlomEvent::NotificationLast => "NotificationLast",
            GlomEvent::PipelineActionsClose => "PipelineActionsClose",
            GlomEvent::PipelineActionsOpen(_, _) => "PipelineActionsOpen",
            GlomEvent::PipelineOpenUrl(_, _) => "PipelineOpenUrl",
            GlomEvent::PipelineSelected(_) => "PipelineSelected",
            GlomEvent::PipelinesFetch(_) => "PipelinesFetch",
            GlomEvent::PipelinesLoaded(_) => "PipelinesLoaded",
            GlomEvent::ProjectDetailsClose => "ProjectDetailsClose",
            GlomEvent::ProjectDetailsOpen(_) => "ProjectDetailsOpen",
            GlomEvent::ProjectFetch(_) => "ProjectFetch",
            GlomEvent::ProjectNext => "ProjectNext",
            GlomEvent::ProjectOpenUrl(_) => "ProjectOpenUrl",
            GlomEvent::ProjectPrevious => "ProjectPrevious",
            GlomEvent::ProjectSelected(_) => "ProjectSelected",
            GlomEvent::ProjectUpdated(_) => "ProjectUpdated",
            GlomEvent::ProjectsFetch => "ProjectsFetch",
            GlomEvent::ProjectsLoaded(_) => "ProjectsLoaded",
            GlomEvent::ProjectStatisticsFetch(_) => "ProjectStatisticsFetch",
            GlomEvent::ProjectStatisticsLoaded(_, _) => "ProjectStatisticsLoaded",
            GlomEvent::ScreenCapture => "ScreenCapture",
            GlomEvent::ScreenCaptureToClipboard(_) => "ScreenCaptureToClipboard",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GlitchState {
    #[allow(dead_code)]
    RampedUp,
    #[allow(dead_code)]
    Normal,
}

#[derive(Debug)]
pub struct EventHandler {
    sender: mpsc::Sender<GlomEvent>,
    receiver: mpsc::Receiver<GlomEvent>,
    _handler: thread::JoinHandle<()>,
}

pub trait IntoGlomEvent {
    fn into_glom_event(self) -> GlomEvent;
}

impl EventHandler {
    pub fn new(tick_rate: std::time::Duration) -> Self {
        let (sender, receiver) = mpsc::channel();

        let handler = {
            let sender = sender.clone();
            thread::spawn(move || {
                let mut last_tick = std::time::Instant::now();
                loop {
                    let timeout = tick_rate
                        .checked_sub(last_tick.elapsed())
                        .unwrap_or(tick_rate);

                    if event::poll(timeout).expect("unable to poll for events") {
                        Self::apply_event(&sender);
                    }

                    if last_tick.elapsed() >= tick_rate {
                        sender.dispatch(GlomEvent::AppTick);
                        last_tick = std::time::Instant::now();
                    }
                }
            })
        };

        Self { sender, receiver, _handler: handler }
    }

    pub fn sender(&self) -> mpsc::Sender<GlomEvent> {
        self.sender.clone()
    }

    pub fn next(&self) -> Result<GlomEvent, mpsc::RecvError> {
        self.receiver.recv()
    }

    pub fn try_next(&self) -> Option<GlomEvent> {
        self.receiver.try_recv().ok()
    }

    fn apply_event(sender: &mpsc::Sender<GlomEvent>) {
        match event::read().expect("unable to read event") {
            CrosstermEvent::Key(e) if e.kind == KeyEventKind::Press => {
                sender.send(GlomEvent::InputKey(e))
            },

            _ => Ok(()),
        }
        .expect("failed to send event")
    }
}

impl From<Vec<ProjectDto>> for GlomEvent {
    fn from(projects: Vec<ProjectDto>) -> Self {
        GlomEvent::ProjectsLoaded(projects)
    }
}

impl From<Vec<PipelineDto>> for GlomEvent {
    fn from(pipelines: Vec<PipelineDto>) -> Self {
        GlomEvent::PipelinesLoaded(pipelines)
    }
}

impl From<(ProjectId, PipelineId, Vec<JobDto>)> for GlomEvent {
    fn from(value: (ProjectId, PipelineId, Vec<JobDto>)) -> Self {
        let (project_id, pipeline_id, jobs) = value;
        GlomEvent::JobsLoaded(project_id, pipeline_id, jobs)
    }
}

impl IntoGlomEvent for Vec<ProjectDto> {
    fn into_glom_event(self) -> GlomEvent {
        GlomEvent::ProjectsLoaded(self)
    }
}

impl IntoGlomEvent for Vec<PipelineDto> {
    fn into_glom_event(self) -> GlomEvent {
        GlomEvent::PipelinesLoaded(self)
    }
}

impl IntoGlomEvent for (ProjectId, PipelineId, Vec<JobDto>) {
    fn into_glom_event(self) -> GlomEvent {
        let (project_id, pipeline_id, jobs) = self;
        GlomEvent::JobsLoaded(project_id, pipeline_id, jobs)
    }
}
