use crate::{event::GlomEvent, ui::StatefulWidgets};

pub trait InputProcessor {
    fn apply(&mut self, event: &GlomEvent, ui: &mut StatefulWidgets);

    fn on_pop(&self);
    fn on_push(&self);
}
