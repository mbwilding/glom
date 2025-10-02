use std::sync::mpsc::Sender;

use compact_str::ToCompactString;
use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    dispatcher::Dispatcher, event::GlomEvent, id::ProjectId, input::InputProcessor,
    ui::StatefulWidgets,
};

pub struct NormalModeProcessor {
    sender: Sender<GlomEvent>,
    selected: Option<ProjectId>,
}

impl NormalModeProcessor {
    pub fn new(sender: Sender<GlomEvent>) -> Self {
        Self { sender, selected: None }
    }

    fn process(&self, event: &KeyEvent) {
        if let Some(e) = match event.code {
            KeyCode::Enter if self.selected.is_some() => Some(GlomEvent::ProjectDetailsOpen(
                self.selected.clone().unwrap(),
            )),
            KeyCode::Char('o') if self.selected.is_some() => Some(GlomEvent::ProjectDetailsOpen(
                self.selected.clone().unwrap(),
            )),
            KeyCode::Char('a') => Some(GlomEvent::NotificationLast),
            KeyCode::Char('c') => Some(GlomEvent::ConfigOpen),
            KeyCode::Char('f') => Some(GlomEvent::FilterMenuShow),
            KeyCode::Char('/') => Some(GlomEvent::FilterMenuShow),
            KeyCode::Char('p') => self
                .selected
                .clone()
                .map(GlomEvent::PipelinesFetch),
            KeyCode::Char('q') => Some(GlomEvent::AppExit),
            KeyCode::Char('r') => Some(GlomEvent::ProjectsFetch),
            KeyCode::Char('w') => self
                .selected
                .clone()
                .map(GlomEvent::ProjectOpenUrl),
            KeyCode::F(12) => Some(GlomEvent::ScreenCapture),
            KeyCode::Up => Some(GlomEvent::ProjectPrevious),
            KeyCode::Down => Some(GlomEvent::ProjectNext),
            KeyCode::Char('k') => Some(GlomEvent::ProjectPrevious),
            KeyCode::Char('j') => Some(GlomEvent::ProjectNext),
            KeyCode::Esc => Some(GlomEvent::FilterClear),
            _ => None,
        } {
            self.dispatch(e)
        }
    }

    fn process_filter_input(&self, event: &KeyEvent, _widgets: &mut StatefulWidgets) {
        match event.code {
            KeyCode::Enter => {
                // Filter is already applied, just close the input
                self.dispatch(GlomEvent::FilterMenuClose);
            },
            KeyCode::Esc => {
                // Cancel filter and reset to no filter
                self.dispatch(GlomEvent::ApplyTemporaryFilter(None));
                self.dispatch(GlomEvent::FilterMenuClose);
            },
            KeyCode::Backspace => {
                self.dispatch(GlomEvent::FilterInputBackspace);
            },
            KeyCode::Char(c) => {
                self.dispatch(GlomEvent::FilterInputChar(c.to_compact_string()));
            },
            _ => {},
        }
    }
}

impl InputProcessor for NormalModeProcessor {
    fn apply(&mut self, event: &GlomEvent, ui: &mut StatefulWidgets) {
        match event {
            GlomEvent::ProjectSelected(id) => self.selected = Some(id.clone()),
            GlomEvent::InputKey(e) => {
                if ui.filter_input_active {
                    self.process_filter_input(e, ui);
                } else {
                    self.process(e);
                }
            },
            _ => (),
        }
    }

    fn on_pop(&self) {}
    fn on_push(&self) {}
}

impl Dispatcher for NormalModeProcessor {
    fn dispatch(&self, event: GlomEvent) {
        self.sender.dispatch(event)
    }
}
