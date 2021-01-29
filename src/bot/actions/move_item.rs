use std::time::Duration;

use crate::bot::actions::put_item::PutItem;
use crate::bot::actions::take_item::TakeItem;
use crate::bot::protocol::{Event, Message};
use crate::bot::vec2::Vec2i;
use crate::bot::world::PlayerWorld;

pub struct MoveItem {
    take_item: TakeItem,
    put_item: Option<PutItem>,
}

impl MoveItem {
    pub fn new(item_id: i32, widget_id: i32, position: Vec2i, timeout: Duration) -> Self {
        debug!("MoveItem item_id={} widget_id={} position={:?}", item_id, widget_id, position);
        Self {
            take_item: TakeItem::new(item_id, timeout),
            put_item: None,
        }
    }

    pub fn new_item_id(&self) -> Option<i32> {
        self.put_item.as_ref().and_then(|v| v.new_item_id())
    }

    pub fn get_next_message(&mut self, world: &PlayerWorld) -> Option<Message> {
        if let Some(put_item) = self.put_item.as_mut() {
            put_item.get_next_message(world)
        } else {
            self.take_item.get_next_message(world)
        }
    }

    pub fn update(&mut self, game_ui_id: i32, event: &Event) {
        if let Some(put_item) = self.put_item.as_mut() {
            put_item.update(event);
        } else {
            self.take_item.update(game_ui_id, event);
        }
    }
}
