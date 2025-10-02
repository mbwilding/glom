use std::sync::mpsc::Sender;

use crate::{
    event::GlomEvent,
    input::{
        InputProcessor,
        processor::{ConfigProcessor, PipelineActionsProcessor, ProjectDetailsProcessor},
    },
    ui::StatefulWidgets,
};

pub struct InputMultiplexer {
    sender: Sender<GlomEvent>,
    processors: Vec<Box<dyn InputProcessor>>,
}

impl InputMultiplexer {
    pub fn new(sender: Sender<GlomEvent>) -> Self {
        Self { sender, processors: Vec::new() }
    }

    pub fn push(&mut self, processor: Box<dyn InputProcessor>) {
        self.processors.push(processor);
        if let Some(processor) = self.processors.last() {
            processor.on_push()
        }
    }

    pub fn pop_processor(&mut self) {
        if let Some(processor) = self.processors.last() {
            processor.on_pop();
        }
        self.processors.pop();
    }

    pub fn apply(&mut self, event: &GlomEvent, ui: &mut StatefulWidgets) {
        match event {
            // project details popup
            GlomEvent::ProjectDetailsOpen(id) => {
                self.push(Box::new(ProjectDetailsProcessor::new(
                    self.sender.clone(),
                    id.clone(),
                )));
            },
            GlomEvent::ProjectDetailsClose => self.pop_processor(),

            // pipeline actions popup
            GlomEvent::PipelineActionsOpen(_, _) => {
                self.push(Box::new(PipelineActionsProcessor::new(self.sender.clone())));
            },
            GlomEvent::PipelineActionsClose => self.pop_processor(),

            // config
            GlomEvent::ConfigOpen => {
                self.push(Box::new(ConfigProcessor::new(self.sender.clone())));
            },
            GlomEvent::ConfigClose => self.pop_processor(),

            _ => (),
        }

        if let Some(processor) = self.processors.last_mut() {
            processor.apply(event, ui)
        }
    }
}
