use std::time::{Duration, Instant};

use crate::bot::protocol::{Message, Value};
use crate::bot::vec2::Vec2i;
use crate::bot::world::PlayerWorld;

pub struct OpenBelt {
    timeout: Duration,
    last_message: Option<Instant>,
    item_id: Option<i32>,
    widget_id: Option<i32>,
}

impl OpenBelt {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            last_message: None,
            item_id: None,
            widget_id: None,
        }
    }

    pub fn get_next_message(&mut self, world: &PlayerWorld) -> Option<Message> {
        let item_id = world.player_equipment().belt();
        if self.item_id != item_id {
            self.item_id = item_id;
            self.widget_id = None;
            debug!("OpenBelt: found belt item={:?}", self.item_id);
        }
        if let Some(widget_id) = self.widget_id {
            if world.widgets().contains_key(&widget_id) {
                debug!("OpenBelt: opened widget={:?}", self.widget_id);
                return Some(Message::Done { task: String::from("OpenBelt") });
            } else {
                self.widget_id = None;
            }
        }
        if self.widget_id.is_none() {
            self.widget_id = world.widgets().values()
                .find(|widget| {
                    widget.kind == "wnd"
                        && widget.parent == world.game_ui_id()
                        && widget.pargs_add.len() >= 3
                        && widget.pargs_add[2] == &["id", "toolbelt"][..]
                })
                .map(|widget| widget.id);
            if self.widget_id.is_some() {
                debug!("OpenBelt: opened widget={:?}", self.widget_id);
                return Some(Message::Done { task: String::from("OpenBelt") });
            }
        }
        if let Some(item_id) = self.item_id {
            let now = Instant::now();
            if self.last_message.map(|v| now - v < self.timeout).unwrap_or(false) {
                debug!("OpenBelt: wait");
                return None;
            }
            self.last_message = Some(now);
            debug!("OpenBelt: click item={}", item_id);
            return Some(Message::WidgetMessage {
                sender: item_id,
                kind: String::from("iact"),
                arguments: vec![Value::from(Vec2i::zero()), Value::from(0i32)],
            });
        }
        Some(Message::Error { message: String::from("belt item is not found") })
    }
}
