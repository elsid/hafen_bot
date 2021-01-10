use crate::bot::protocol::{Event, Message, Update};
use crate::bot::scene::Scene;
use crate::bot::tasks::task::Task;
use crate::bot::world::PlayerWorld;

pub struct ExpWndCloser {
    closed: Vec<i32>,
    exp_wnd_ids: Vec<i32>,
}

impl ExpWndCloser {
    pub fn new() -> Self {
        Self {
            closed: Vec::new(),
            exp_wnd_ids: Vec::new(),
        }
    }
}

impl Task for ExpWndCloser {
    fn name(&self) -> &'static str {
        "ExpWndCloser"
    }

    fn get_next_message(&mut self, _: &PlayerWorld, _: &Scene) -> Option<Message> {
        if let Some(id) = self.exp_wnd_ids.iter().find(|v| self.closed.iter().find(|w| w == v).is_none()) {
            debug!("ExpWndCloser: close {}", id);
            self.closed.push(*id);
            return Some(Message::WidgetMessage {
                sender: *id,
                kind: String::from("close"),
                arguments: Vec::new(),
            });
        }
        None
    }

    fn update(&mut self, _: &PlayerWorld, update: &Update) {
        match &update.event {
            Event::NewWidget { id, kind, parent: _, pargs: _, cargs: _ } => {
                if kind.as_str().starts_with("ui/expwnd:") {
                    debug!("ExpWndCloser: got a new window {}", id);
                    self.exp_wnd_ids.push(*id);
                }
            }
            Event::WidgetMessage { id, msg, args: _ } => {
                if msg.as_str() == "close" {
                    debug!("ExpWndCloser: window {} is closed", id);
                    self.exp_wnd_ids.retain(|v| v != id);
                    self.closed.retain(|v| v != id);
                }
            }
            Event::Destroy { id } => {
                debug!("ExpWndCloser: window {} is destroyed", id);
                self.exp_wnd_ids.retain(|v| v != id);
                self.closed.retain(|v| v != id);
            }
            _ => (),
        }
    }

    fn restore(&mut self, world: &PlayerWorld) {
        for widget in world.widgets().values() {
            if widget.kind.as_str().starts_with("ui/expwnd:") {
                debug!("ExpWndCloser: restored window {}", widget.id);
                self.exp_wnd_ids.push(widget.id);
            }
        }
    }
}
