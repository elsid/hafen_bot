use std::time::{Duration, Instant};

use crate::bot::bot::Bot;
use crate::bot::protocol::{Event, Message, Update, Value};
use crate::bot::scene::Scene;
use crate::bot::vec2::Vec2i;
use crate::bot::world::PlayerWorld;

const TIMEOUT: Duration = Duration::from_secs(1);

pub struct UseItem {
    item_id: i32,
    action_name: String,
    last_message: Option<Instant>,
    action: Option<Action>,
    ready: bool,
    done: bool,
}

impl UseItem {
    pub fn new(item_id: i32, action_name: String) -> Self {
        debug!("UseItem item_id={} action_name={}", item_id, action_name);
        Self {
            item_id,
            action_name,
            last_message: None,
            action: None,
            ready: false,
            done: false,
        }
    }

    pub fn item_id(&self) -> i32 {
        self.item_id
    }
}

impl Bot for UseItem {
    fn name(&self) -> &'static str {
        "UseItem"
    }

    fn get_next_message(&mut self, _: &PlayerWorld, _: &Scene) -> Option<Message> {
        if self.done {
            debug!("UseItem item_id={} action_name={}: done", self.item_id, self.action_name);
            return Some(Message::Done { bot: String::from("UseItem") });
        }
        let now = Instant::now();
        if let Some(action) = self.action.as_ref() {
            if !self.ready {
                debug!("UseItem item_id={} action_name={}: not ready", self.item_id, self.action_name);
                return None;
            }
            if self.last_message.map(|v| now - v < TIMEOUT).unwrap_or(false) {
                debug!("UseItem item_id={} action_name={}: wait apply action", self.item_id, self.action_name);
                return None;
            }
            self.last_message = Some(now);
            debug!("UseItem item_id={} action_name={}: apply action", self.item_id, self.action_name);
            Some(Message::WidgetMessage {
                sender: action.id,
                kind: String::from("cl"),
                arguments: vec![Value::from(action.index), Value::from(0i32)],
            })
        } else {
            if self.last_message.map(|v| now - v < TIMEOUT).unwrap_or(false) {
                debug!("UseItem item_id={} action_name={}: wait get action", self.item_id, self.action_name);
                return None;
            }
            self.last_message = Some(now);
            debug!("UseItem item_id={} action_name={}: get action", self.item_id, self.action_name);
            Some(Message::WidgetMessage {
                sender: self.item_id,
                kind: String::from("iact"),
                arguments: vec![Value::from(Vec2i::zero()), Value::from(0i32)],
            })
        }
    }

    fn update(&mut self, _: &PlayerWorld, update: &Update) {
        if self.done {
            return;
        }
        match &update.event {
            Event::NewWidget { id, kind, parent: _, pargs: _, cargs } => {
                if kind == "sm" && cargs.len() >= 1 {
                    self.action = cargs.iter()
                        .enumerate()
                        .find(|(_, v)| **v == self.action_name)
                        .map(|(i, _)| Action { id: *id, index: i as i32 });
                    self.ready = false;
                    self.last_message = None;
                    debug!("UseItem item_id={} action_name={}: choose action={:?}", self.item_id, self.action_name, self.action);
                }
            }
            Event::AddWidget { id, parent: _, pargs: _ } => {
                if self.action.as_ref().map(|v| v.id == *id).unwrap_or(false) {
                    self.ready = true;
                    self.last_message = None;
                    debug!("UseItem item_id={} action_name={}: ready", self.item_id, self.action_name);
                }
            }
            Event::UIMessage { id, msg, args: _ } => {
                if self.action.as_ref().map(|v| v.id == *id).unwrap_or(false) {
                    match msg.as_str() {
                        "act" => {
                            debug!("UseItem item_id={} action_name={}: set done", self.item_id, self.action_name);
                            self.done = true;
                        }
                        "cancel" => {
                            debug!("UseItem item_id={} action_name={}: cancel", self.item_id, self.action_name);
                            self.action = None;
                            self.ready = false;
                            self.last_message = None;
                        }
                        _ => (),
                    }
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
