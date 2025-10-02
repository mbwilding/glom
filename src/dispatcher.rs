use std::sync::mpsc;

use crate::event::GlomEvent;

pub trait Dispatcher {
    fn dispatch(&self, event: GlomEvent);
}

impl Dispatcher for mpsc::Sender<GlomEvent> {
    fn dispatch(&self, event: GlomEvent) {
        let _ = self.send(event);
    }
}
