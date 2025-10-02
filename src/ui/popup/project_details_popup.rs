use compact_str::ToCompactString;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    prelude::{Line, StatefulWidget, Text},
    text::Span,
    widgets::{TableState, Widget},
};
use tachyonfx::RefRect;

use crate::{
    domain::{Pipeline, Project},
    theme::theme,
    ui::{fx::popup_window, popup::utility::CenteredShrink, widget::PipelineTable},
};

/// Project details popup
pub struct ProjectDetailsPopup {}

/// State of the project details popup
pub struct ProjectDetailsPopupState {
    pub project: Project,
    project_namespace: Text<'static>,
    project_stat_summary: Text<'static>,
    pub pipelines: PipelineTable,
    pub pipelines_table_state: TableState,
    pub popup_area: RefRect,
}

impl ProjectDetailsPopup {
    pub fn new() -> ProjectDetailsPopup {
        Self {}
    }
}

impl ProjectDetailsPopupState {
    pub fn with_project(&self, project: Project) -> Self {
        Self::new(project, self.popup_area.clone())
    }

    pub fn new(project: Project, popup_area: RefRect) -> ProjectDetailsPopupState {
        let (namespace, name) = project.path_and_name();

        let description = match &project.description {
            Some(d) => d.to_string(),
            None => String::new(),
        };

        let project_namespace = Text::from(vec![
            Line::from(name.to_string()).style(theme().project_name),
            Line::from(namespace.trim_end_matches('/').to_string()).style(theme().project_parents),
            Line::from(description).style(theme().project_description),
        ]);

        let project_stat_summary = Self::create_stats_text(
            project.commit_count,
            project.repo_size_kb,
            project.artifacts_size_kb,
            project.statistics_loading,
        );

        let pipelines: Vec<&Pipeline> = project.recent_pipelines();
        let pipelines = PipelineTable::new(&pipelines);

        ProjectDetailsPopupState {
            project,
            project_namespace,
            project_stat_summary,
            pipelines,
            pipelines_table_state: TableState::default().with_selected(0),
            popup_area,
        }
    }

    fn create_stats_text(
        commit_count: u32,
        repo_size_kb: u64,
        artifacts_size_kb: u64,
        loading: bool,
    ) -> Text<'static> {
        let commits_value = if loading && commit_count == 0 {
            "···"
        } else {
            &commit_count.to_compact_string()
        };

        let repo_size_value = if loading && repo_size_kb == 0 {
            "···"
        } else {
            &Self::format_size(repo_size_kb)
        };

        let artifacts_size_value = if loading && artifacts_size_kb == 0 {
            "···"
        } else {
            &Self::format_size(artifacts_size_kb)
        };

        let width = 22;

        Text::from(vec![
            Self::create_aligned_line("Commits:", commits_value, width, theme().project_commits),
            Self::create_aligned_line("Repository:", repo_size_value, width, theme().project_size),
            Self::create_aligned_line(
                "Artifacts:",
                artifacts_size_value,
                width,
                theme().project_size,
            ),
        ])
    }

    fn create_aligned_line(
        label: &str,
        value: &str,
        width: usize,
        styles: [ratatui::style::Style; 2],
    ) -> Line<'static> {
        let label_len = label.len();
        let value_len = value.len();
        let total_content = label_len + value_len;

        if total_content >= width {
            Line::from(vec![
                Span::from(label.to_string()).style(styles[1]),
                Span::from(" ").style(styles[1]),
                Span::from(value.to_string()).style(styles[0]),
            ])
        } else {
            let spacing = width - total_content;
            let spaces = " ".repeat(spacing);

            Line::from(vec![
                Span::from(label.to_string()).style(styles[1]),
                Span::from(spaces).style(styles[1]),
                Span::from(value.to_string()).style(styles[0]),
            ])
        }
    }

    fn format_size(size_kb: u64) -> String {
        let (size, unit) = match size_kb {
            s if s < 1024 => (s as f32, "KB"),
            s if s < 1024 * 1024 => (s as f32 / 1024.0, "MB"),
            s => (s as f32 / (1024.0 * 1024.0), "GB"),
        };
        format!("{size:.2} {unit}")
    }

    pub fn update_popup_area(&self, screen: Rect) -> Rect {
        let pipeline_table_h = 2 * self.pipelines.rows.len() as u16;
        let project_details_h = 4;
        let total_height = 2 + project_details_h + pipeline_table_h;

        let a = screen.inner_centered(screen.width, total_height);
        self.popup_area.set(a);
        a
    }
}

impl StatefulWidget for ProjectDetailsPopup {
    type State = ProjectDetailsPopupState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let pipeline_table_h = 2 * state.pipelines.rows.len() as u16;
        let project_details_h = 4;

        let area = state.update_popup_area(area);

        popup_window(
            "Project Details",
            Some(vec![
                ("ESC", "close"),
                ("↑ ↓", "selection"),
                ("↵", "actions..."),
            ]),
        )
        .render(area, buf);

        let content_area = area.inner(Margin::new(2, 1));
        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(project_details_h),
                Constraint::Length(pipeline_table_h),
            ])
            .split(content_area);

        let project_details_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100), Constraint::Length(22)])
            .split(outer_layout[0]);

        state
            .project_namespace
            .clone()
            .render(project_details_layout[0], buf);

        state
            .project_stat_summary
            .clone()
            .render(project_details_layout[1], buf);

        PipelineTable::new(&state.project.recent_pipelines()).render(
            outer_layout[1],
            buf,
            &mut state.pipelines_table_state,
        );
    }
}
