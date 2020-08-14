use std::collections::BTreeMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::bot::map::pos_to_grid_pos;
use crate::bot::protocol::{Event, Update, Value};
use crate::bot::stuck_detector::StuckDetector;
use crate::bot::vec2::{Vec2f, Vec2i};
use crate::bot::world::World;

#[derive(Default)]
pub struct Player {
    map_view_id: Option<i32>,
    game_ui_id: Option<i32>,
    name: Option<String>,
    object_id: Option<i64>,
    grid_id: Option<i64>,
    position: Option<Vec2f>,
    widgets: BTreeMap<i32, Widget>,
    map_grids: Vec<MapGrid>,
    stuck_detector: StuckDetector,
    is_stuck: bool,
}

impl Player {
    pub fn map_view_id(&self) -> Option<i32> {
        self.map_view_id
    }

    pub fn game_ui_id(&self) -> Option<i32> {
        self.game_ui_id
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    pub fn object_id(&self) -> Option<i64> {
        self.object_id
    }

    pub fn grid_id(&self) -> Option<i64> {
        self.grid_id
    }

    pub fn widgets(&self) -> &BTreeMap<i32, Widget> {
        &self.widgets
    }

    pub fn is_stuck(&self) -> bool {
        self.is_stuck
    }

    pub fn from_player_data(data: PlayerData) -> Self {
        Self {
            map_view_id: data.map_view_id,
            game_ui_id: data.game_ui_id,
            name: data.name,
            object_id: data.object_id,
            grid_id: data.grid_id,
            position: data.position,
            widgets: data.widgets.into_iter().map(|v| (v.id, v)).collect(),
            map_grids: data.map_grids,
            stuck_detector: StuckDetector::new(),
            is_stuck: false,
        }
    }

    pub fn as_player_data(&self) -> PlayerData {
        PlayerData {
            map_view_id: self.map_view_id,
            game_ui_id: self.game_ui_id,
            name: self.name.clone(),
            object_id: self.object_id,
            grid_id: self.grid_id,
            position: self.position,
            widgets: self.widgets.values().cloned().collect(),
            map_grids: self.map_grids.clone(),
        }
    }

    pub fn update(&mut self, world: &World, update: &Update) -> bool {
        match &update.event {
            Event::NewWidget { id, kind, parent, pargs: _, cargs } => {
                match kind.as_str() {
                    "gameui" => {
                        self.game_ui_id = Some(*id);
                        if cargs.len() >= 2 {
                            if let Value::Str { value } = &cargs[0] {
                                self.name = Some(value.clone());
                            }
                            if let Value::Int { value } = &cargs[1] {
                                self.object_id = Some(*value as i64);
                                if let Some(object) = world.objects().get_by_id(*value as i64) {
                                    self.update_player(object.id, object.position);
                                }
                            }
                        }
                    }
                    "mapview" => {
                        self.map_view_id = Some(*id);
                    }
                    _ => (),
                }
                self.widgets.insert(*id, Widget { id: *id, parent: *parent, kind: kind.clone() });
                true
            }
            Event::UIMessage { id, msg, args } => {
                if Some(*id) == self.map_view_id && msg.as_str() == "plob" && args.len() > 0 {
                    match &args[0] {
                        Value::Nil => {
                            self.object_id = None;
                            true
                        }
                        Value::Int { value } => {
                            self.object_id = Some((*value).into());
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            Event::Destroy { id } => {
                if Some(*id) == self.game_ui_id {
                    self.game_ui_id = None;
                } else if Some(*id) == self.map_view_id {
                    self.map_view_id = None;
                }
                self.widgets.remove(id).is_some()
            }
            Event::MapGridAdd { grid, neighbours: _ } => {
                self.map_grids.push(MapGrid { id: grid.id, position: grid.position });
                if Some(grid.position) == self.position.map(|v| pos_to_grid_pos(v)) {
                    self.grid_id = Some(grid.id);
                    debug!("Player: set grid: {}", grid.id);
                }
                true
            }
            Event::MapGridRemove { id } => {
                if self.grid_id == Some(*id) {
                    self.grid_id = None;
                    debug!("Player: reset grid");
                }
                self.map_grids.retain(|grid| grid.id != *id);
                true
            }
            Event::GobAdd { id, position, angle: _, name: _ } => {
                self.update_player(*id, *position)
            }
            Event::GobRemove { id } => {
                if Some(*id) == self.object_id {
                    self.position = None;
                    self.grid_id = None;
                    self.stuck_detector = StuckDetector::new();
                    self.is_stuck = false;
                    debug!("Player: reset");
                    true
                } else {
                    false
                }
            }
            Event::GobMove { id, position, angle: _ } => {
                self.update_player(*id, *position)
            }
            _ => false,
        }
    }

    fn update_player(&mut self, object_id: i64, object_position: Vec2f) -> bool {
        if self.object_id == Some(object_id) {
            self.position = Some(object_position);
            let grid_position = pos_to_grid_pos(object_position);
            if let Some(grid) = self.map_grids.iter().find(|v| v.position == grid_position) {
                self.grid_id = Some(grid.id);
            }
            let now = Instant::now();
            self.is_stuck = self.stuck_detector.check(object_position, now);
            self.stuck_detector.update(object_position, now);
            if self.is_stuck {
                debug!("Player is stuck at {:?}", object_position);
            }
            true
        } else {
            false
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PlayerData {
    map_view_id: Option<i32>,
    game_ui_id: Option<i32>,
    name: Option<String>,
    object_id: Option<i64>,
    grid_id: Option<i64>,
    position: Option<Vec2f>,
    widgets: Vec<Widget>,
    map_grids: Vec<MapGrid>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Widget {
    pub id: i32,
    pub parent: i32,
    pub kind: String,
}

#[derive(Default, Serialize, Deserialize, Clone)]
struct MapGrid {
    id: i64,
    position: Vec2i,
}
