use std::sync::mpsc::Sender;

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    dispatcher::Dispatcher,
    event::GlomEvent,
    id::{PipelineId, ProjectId},
    input::InputProcessor,
    ui::StatefulWidgets,
};

pub struct ProjectDetailsProcessor {
    sender: Sender<GlomEvent>,
    project_id: ProjectId,
    selected: Option<PipelineId>,
}

impl ProjectDetailsProcessor {
    pub fn new(sender: Sender<GlomEvent>, project_id: ProjectId) -> Self {
        Self { sender, project_id, selected: None }
    }

    fn process(&self, event: &KeyEvent, ui: &mut StatefulWidgets) {
        match event.code {
            KeyCode::Esc => self
                .sender
                .dispatch(GlomEvent::ProjectDetailsClose),
            KeyCode::Char('q') => self
                .sender
                .dispatch(GlomEvent::ProjectDetailsClose),
            KeyCode::Up => ui.handle_pipeline_selection(-1),
            KeyCode::Down => ui.handle_pipeline_selection(1),
            KeyCode::Char('k') => ui.handle_pipeline_selection(-1),
            KeyCode::Char('j') => ui.handle_pipeline_selection(1),
            KeyCode::Enter if self.selected.is_some() => {
                self.sender
                    .dispatch(GlomEvent::PipelineActionsOpen(
                        self.project_id.clone(),
                        self.selected.unwrap(),
                    ))
            },
            KeyCode::Char('o') if self.selected.is_some() => {
                self.sender
                    .dispatch(GlomEvent::PipelineActionsOpen(
                        self.project_id.clone(),
                        self.selected.unwrap(),
                    ))
            },
            KeyCode::F(12) => self.sender.dispatch(GlomEvent::ScreenCapture),
            _ => (),
        }
    }
}

impl InputProcessor for ProjectDetailsProcessor {
    fn apply(&mut self, event: &GlomEvent, ui: &mut StatefulWidgets) {
        match event {
            GlomEvent::PipelineSelected(pipeline) => self.selected = Some(*pipeline),
            GlomEvent::InputKey(e) => self.process(e, ui),
            _ => (),
        }
    }

    fn on_pop(&self) {}
    fn on_push(&self) {}
}
