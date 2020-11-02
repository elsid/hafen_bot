use std::time::{Duration, Instant};

use crate::bot::bot::Bot;
use crate::bot::protocol::{Event, Message, Update, Value};
use crate::bot::scene::Scene;
use crate::bot::vec2::Vec2i;
use crate::bot::world::PlayerWorld;

const TIMEOUT: Duration = Duration::from_secs(1);

pub struct OpenBelt {
    last_message: Option<Instant>,
    item_id: Option<i32>,
    widget_id: Option<i32>,
}

impl OpenBelt {
    pub fn new() -> Self {
        Self {
            last_message: None,
            item_id: None,
            widget_id: None,
        }
    }
}

impl Bot for OpenBelt {
    fn name(&self) -> &'static str {
        "OpenBelt"
    }

    fn get_next_message(&mut self, world: &PlayerWorld, _: &Scene) -> Option<Message> {
        let item_id = world.player_equipment().belt();
        if self.item_id != item_id {
            debug!("OpenBelt: new item={:?}", item_id);
            self.item_id = item_id;
            self.widget_id = None;
        }
        if self.widget_id.is_some() {
            debug!("OpenBelt: done");
            return Some(Message::Done { bot: String::from("OpenBelt") });
        }
        if let Some(item_id) = self.item_id {
            let now = Instant::now();
            if self.last_message.map(|v| now - v < TIMEOUT).unwrap_or(false) {
                debug!("OpenBelt: wait");
                return None;
            }
            self.last_message = Some(now);
            debug!("OpenBelt: click={}", item_id);
            return Some(Message::WidgetMessage {
                sender: item_id,
                kind: String::from("iact"),
                arguments: vec![Value::from(Vec2i::zero()), Value::from(0i32)],
            });
        }
        Some(Message::Done { bot: String::from("OpenBelt") })
    }

    fn update(&mut self, world: &PlayerWorld, update: &Update) {
        debug!("OpenBelt: update");
        match &update.event {
            Event::AddWidget { id, parent, pargs } => {
                if *parent == world.game_ui_id() && pargs.len() >= 3 && pargs[2] == &["id", "toolbelt"][..] {
                    debug!("OpenBelt: new widget={}", id);
                    self.widget_id = Some(*id);
                }
            }
            Event::Destroy { id } => {
                if Some(*id) == self.widget_id {
                    debug!("OpenBelt: reset widget");
                    self.widget_id = None;
                }
            }
            _ => (),
        }
    }
}

#[derive(Debug)]
struct Action {
    id: i32,
    index: i32,
}
