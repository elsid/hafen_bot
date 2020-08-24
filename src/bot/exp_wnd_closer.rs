use crate::bot::bot::Bot;
use crate::bot::protocol::{Event, Message, Update};
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

impl Bot for ExpWndCloser {
    fn name(&self) -> &'static str {
        "ExpWndCloser"
    }

    fn get_next_message(&mut self, _: &PlayerWorld) -> Option<Message> {
        if let Some(id) = self.exp_wnd_ids.iter().find(|v| self.closed.iter().find(|w| w == v).is_none()) {
            info!("ExpWndCloser: close {}", id);
            self.closed.push(*id);
            return Some(Message::WidgetMessage {
                sender: *id,
                kind: String::from("close"),
                arguments: Vec::new(),
            });
        }
        None
    }

    fn update(&mut self, update: &Update) {
        match &update.event {
            Event::NewWidget { id, kind, parent: _, pargs: _, cargs: _ } => {
                if kind.as_str() == "ui/expwnd:17" {
                    info!("ExpWndCloser: got a new window {}", id);
                    self.exp_wnd_ids.push(*id);
                }
            }
            Event::WidgetMessage { id, msg, args: _ } => {
                if msg.as_str() == "close" {
                    info!("ExpWndCloser: window {} is closed", id);
                    self.exp_wnd_ids.retain(|v| v != id);
                    self.closed.retain(|v| v != id);
                }
            }
            _ => (),
        }
    }
}