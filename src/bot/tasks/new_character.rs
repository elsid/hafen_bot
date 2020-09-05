use std::collections::VecDeque;

use serde::Deserialize;

use crate::bot::map::{map_pos_to_pos, pos_to_map_pos};
use crate::bot::protocol::{Button, Event, Message, Modifier, Update, Value};
use crate::bot::tasks::task::Task;
use crate::bot::scene::Scene;
use crate::bot::vec2::Vec2i;
use crate::bot::world::PlayerWorld;

const MAX_DISTANCE: f64 = 1.0;

#[derive(Deserialize)]
pub struct NewCharacterParams {
    character_name: String,
}

pub struct NewCharacter {
    map_pos_path: VecDeque<Vec2i>,
    name_changer: String,
    character_name: String,
    change_name_window_id: Option<i32>,
    change_name_text_id: Option<i32>,
    state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    FindNameChanger,
    WaitForChangeNameTextId,
    WaitForName,
    HasName,
}

impl NewCharacter {
    pub fn new(params: NewCharacterParams) -> Self {
        Self {
            map_pos_path: VecDeque::from(vec![
                Vec2i::new(-924781, -941823),
                Vec2i::new(-926335, -962729),
                Vec2i::new(-910798, -995822),
                Vec2i::new(-928122, -1018571),
                Vec2i::new(-927311, -1044199),
            ]),
            name_changer: String::from("gfx/borka/body"),
            character_name: params.character_name,
            change_name_window_id: None,
            change_name_text_id: None,
            state: State::FindNameChanger,
        }
    }
}

impl Task for NewCharacter {
    fn name(&self) -> &'static str {
        "NewCharacter"
    }

    fn get_next_message(&mut self, world: &PlayerWorld, _: &Scene) -> Option<Message> {
        if self.state == State::HasName {
            debug!("NewCharacter: has name");
            return None;
        }
        if world.is_player_stuck() && self.change_name_window_id.is_none() && self.change_name_text_id.is_none() {
            debug!("NewCharacter: find the name changer");
            self.state = State::FindNameChanger;
        }
        if self.state == State::WaitForName {
            debug!("NewCharacter: waiting for a name");
            return None;
        }
        if let Some(id) = self.change_name_text_id {
            debug!("NewCharacter: change name");
            self.state = State::WaitForName;
            return Some(Message::UIMessage {
                id,
                kind: String::from("settext"),
                arguments: vec![Value::from(self.character_name.clone())],
            });
        }
        if self.state == State::WaitForChangeNameTextId {
            debug!("NewCharacter: waiting for change name text widget");
            return None;
        }
        if let Some(object) = world.get_object_by_name(&self.name_changer) {
            debug!("NewCharacter: go to the name changer");
            self.state = State::WaitForChangeNameTextId;
            return Some(Message::WidgetMessage {
                sender: world.map_view_id(),
                kind: String::from("click"),
                arguments: vec![
                    Value::from(Vec2i::zero()),
                    Value::from(pos_to_map_pos(object.position)),
                    Value::from(Button::RightClick),
                    Value::from(Modifier::None),
                    Value::from(0i32),
                    Value::from(object.id as i32),
                    Value::from(pos_to_map_pos(object.position)),
                    Value::from(0i32),
                    Value::from(0i32),
                ],
            });
        }
        while !self.map_pos_path.is_empty() {
            if let Some(map_pos) = self.map_pos_path.front() {
                if map_pos_to_pos(*map_pos).distance(world.player_position()) > MAX_DISTANCE {
                    break;
                }
            }
            self.map_pos_path.pop_front();
        }
        if let Some(map_pos) = self.map_pos_path.front() {
            debug!("NewCharacter: go to the next path point: {:?}", map_pos);
            return Some(Message::WidgetMessage {
                sender: world.map_view_id(),
                kind: String::from("click"),
                arguments: vec![
                    Value::from(Vec2i::zero()),
                    Value::from(*map_pos),
                    Value::from(Button::LeftClick),
                    Value::from(Modifier::None),
                ],
            });
        }
        None
    }

    fn update(&mut self, _: &PlayerWorld, update: &Update) {
        match &update.event {
            Event::NewWidget { id, kind, parent, pargs: _, cargs } => {
                match kind.as_str() {
                    "wnd" => {
                        if let Value::Str { value } = &cargs[1] {
                            if value.as_str() != "Change Name" {
                                return;
                            }
                            self.change_name_window_id = Some(*id);
                            debug!("NewCharacter: got change name window id: {}", id);
                        }
                    }
                    "text" => {
                        if Some(parent) == self.change_name_window_id.as_ref() {
                            self.change_name_text_id = Some(*id);
                            debug!("NewCharacter: got change name text widget id: {}", id);
                        }
                    }
                    _ => (),
                }
            }
            Event::Destroy { id } => {
                if self.change_name_window_id == Some(*id) {
                    self.change_name_window_id = None;
                    debug!("NewCharacter: change name window {} is destroyed", id);
                } else if self.change_name_text_id == Some(*id) {
                    self.change_name_text_id = None;
                    debug!("NewCharacter: change name text widget {} is destroyed", id);
                }
            }
            Event::UIMessage { id, msg, args: _ } => {
                if Some(*id) == self.change_name_text_id && msg.as_str() == "settext" {
                    self.state = State::HasName;
                    debug!("NewCharacter: got name");
                }
            }
            _ => (),
        }
    }
}
