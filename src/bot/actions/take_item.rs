use std::time::{Duration, Instant};

use crate::bot::protocol::{Event, Message, Value};
use crate::bot::world::PlayerWorld;

pub struct TakeItem {
    item_id: i32,
    timeout: Duration,
    take: Option<Instant>,
    new_item_id: Option<i32>,
}

impl TakeItem {
    pub fn new(item_id: i32, timeout: Duration) -> Self {
        debug!("TakeItem item_id={}", item_id);
        Self { item_id, timeout, take: None, new_item_id: None }
    }

    pub fn new_item_id(&self) -> Option<i32> {
        self.new_item_id
    }

    pub fn get_next_message(&mut self, world: &PlayerWorld) -> Option<Message> {
        if self.new_item_id.is_some() {
            return Some(Message::Done { task: String::from("TakeItem") });
        }
        let now = Instant::now();
        if self.take.map(|v| now - v < self.timeout).unwrap_or(false) {
            return None;
        }
        self.take = Some(now);
        world.player_inventories().values()
            .find_map(|items| items.get(&self.item_id))
            .and_then(|item| item.position)
            .map(|position| Message::WidgetMessage {
                sender: self.item_id,
                kind: String::from("take"),
                arguments: vec![Value::from(position)],
            })
            .or_else(|| Some(Message::Error { message: String::from("item is not found") }))
    }

    pub fn update(&mut self, game_ui_id: i32, event: &Event) {
        if let Event::NewWidget { id, kind, parent, pargs, .. } = event {
            if kind == "item" && *parent == game_ui_id && pargs.len() > 1 && pargs[0] == "hand" {
                self.new_item_id = Some(*id);
            }
        }
    }
}
