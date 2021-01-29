use std::time::{Duration, Instant};

use crate::bot::protocol::{Event, Message, Value};
use crate::bot::vec2::Vec2i;
use crate::bot::world::PlayerWorld;

pub struct PutItem {
    widget_id: i32,
    position: Vec2i,
    timeout: Duration,
    drop: Option<Instant>,
    new_item_id: Option<i32>,
}

impl PutItem {
    pub fn new(widget_id: i32, position: Vec2i, timeout: Duration) -> Self {
        debug!("PutItem widget_id={} position={:?}", widget_id, position);
        Self { widget_id, position, timeout, drop: None, new_item_id: None }
    }

    pub fn new_item_id(&self) -> Option<i32> {
        self.new_item_id
    }

    pub fn get_next_message(&mut self, world: &PlayerWorld) -> Option<Message> {
        if self.new_item_id.is_some() {
            return Some(Message::Done { task: String::from("PutItem") });
        }
        let now = Instant::now();
        if self.drop.map(|v| now - v < self.timeout).unwrap_or(false) {
            return None;
        }
        if world.player_hand().is_none() {
            return Some(Message::Error { message: String::from("player hand is empty") });
        }
        self.drop = Some(now);
        Some(Message::WidgetMessage {
            sender: self.widget_id,
            kind: String::from("drop"),
            arguments: vec![Value::from(self.position)],
        })
    }

    pub fn update(&mut self, event: &Event) {
        if let Event::NewWidget { id, kind, parent, pargs, .. } = event {
            if kind == "item" && *parent == self.widget_id && pargs.len() > 1 && pargs[0] == self.position {
                self.new_item_id = Some(*id);
            }
        }
    }
}
