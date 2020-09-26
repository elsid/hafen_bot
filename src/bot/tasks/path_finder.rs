use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;

use serde::Deserialize;

use crate::bot::map::{map_pos_to_tile_pos, pos_to_map_pos, pos_to_rel_tile_pos, pos_to_tile_pos, rel_tile_pos_to_pos, TILE_SIZE};
use crate::bot::protocol::{Button, Event, Message, Modifier, Update, Value};
use crate::bot::scene::{Layer, MapTransformArcNode, Node, Scene};
use crate::bot::tasks::task::Task;
use crate::bot::vec2::Vec2i;
use crate::bot::world::{BTreeMapTileWeights, make_find_path_node, PlayerWorld};

#[derive(Clone, Deserialize)]
pub struct PathFinderConfig {
    pub find_path_max_shortcut_length: f64,
    pub find_path_max_iterations: usize,
    pub max_next_point_shortcut_length: f64,
}

pub struct PathFinder {
    destination: Option<Vec2i>,
    tile_pos_path: VecDeque<Vec2i>,
    find_path_layer: Option<Layer>,
    config: PathFinderConfig,
    cancel: Arc<AtomicBool>,
}

impl PathFinder {
    pub fn new(config: PathFinderConfig, cancel: Arc<AtomicBool>) -> Self {
        Self {
            destination: None,
            tile_pos_path: VecDeque::new(),
            find_path_layer: None,
            config,
            cancel,
        }
    }
}

impl Task for PathFinder {
    fn name(&self) -> &'static str {
        "PathFinder"
    }

    fn get_next_message(&mut self, world: &PlayerWorld, scene: &Scene) -> Option<Message> {
        let player_pos = world.player_position();
        let player_tile_pos = pos_to_tile_pos(player_pos);
        if self.destination == Some(player_tile_pos) {
            self.destination = None;
            self.find_path_layer = None;
            debug!("PathFinder: reached destination");
            return Some(Message::Done { task: String::from("PathFinder") });
        }
        let player_tile = world.get_tile(player_tile_pos);
        let water_tiles_cost = world.config().water_tiles.iter()
            .filter_map(|(name, weight)| {
                world.get_tile_id_by_name(name).map(|id| (id, *weight))
            })
            .collect::<BTreeMap<i32, f64>>();
        if player_tile.is_none() || !water_tiles_cost.contains_key(&player_tile.unwrap()) {
            debug!("PathFinder: player tile {:?} at {:?} ({:?}) is not allowed tile {:?}",
                   player_tile, player_pos, pos_to_rel_tile_pos(player_pos), water_tiles_cost);
            self.destination = None;
            return None;
        }
        if self.tile_pos_path.is_empty() {
            if let Some(dst_tile_pos) = self.destination {
                let src_tile_pos = pos_to_tile_pos(player_pos);
                let find_path_node = make_find_path_node();
                self.find_path_layer = Some(Layer::new(
                    scene.clone(),
                    Arc::new(Mutex::new(
                        Node::from(MapTransformArcNode {
                            node: find_path_node.clone(),
                        })
                    )),
                ));
                self.tile_pos_path = VecDeque::from(world.find_path(
                    src_tile_pos,
                    dst_tile_pos,
                    &BTreeMapTileWeights(&water_tiles_cost),
                    self.config.find_path_max_shortcut_length,
                    self.config.find_path_max_iterations,
                    &find_path_node,
                    &self.cancel,
                ));
                if self.tile_pos_path.is_empty() {
                    debug!("PathFinder: path from {:?} to {:?} is not found by tiles {:?}",
                           src_tile_pos, dst_tile_pos, water_tiles_cost);
                    self.destination = None;
                } else {
                    debug!("PathFinder: found path from {:?} to {:?} by tiles {:?}: {:?}",
                           src_tile_pos, dst_tile_pos, water_tiles_cost, self.tile_pos_path);
                }
            }
        }
        while self.tile_pos_path.len() >= 2 {
            let src_rel_tile_pos = pos_to_rel_tile_pos(player_pos);
            let dst_rel_tile_pos = self.tile_pos_path[1].center();
            if !world.is_valid_shortcut_by_rel_pos(
                src_rel_tile_pos,
                dst_rel_tile_pos,
                &BTreeMapTileWeights(&water_tiles_cost),
                self.config.max_next_point_shortcut_length,
            ) {
                break;
            }
            self.tile_pos_path.pop_front();
        }
        while let Some(&tile_pos) = self.tile_pos_path.front() {
            let distance = rel_tile_pos_to_pos(tile_pos.center()).distance(player_pos);
            if distance > (2.0 * TILE_SIZE).sqrt() && tile_pos != pos_to_tile_pos(player_pos) {
                debug!("PathFinder: distance to the next path point {:?}: {}", tile_pos, distance);
                break;
            }
            self.tile_pos_path.pop_front();
        }
        if let Some(tile_pos) = self.tile_pos_path.front() {
            return Some(Message::WidgetMessage {
                sender: world.map_view_id(),
                kind: String::from("click"),
                arguments: vec![
                    Value::from(Vec2i::zero()),
                    Value::from(pos_to_map_pos(rel_tile_pos_to_pos(tile_pos.center()))),
                    Value::from(Button::LeftClick),
                    Value::from(Modifier::None),
                ],
            });
        }
        None
    }

    fn update(&mut self, world: &PlayerWorld, update: &Update) {
        match &update.event {
            Event::WidgetMessage { id, msg, args } => {
                if *id == world.map_view_id() && msg.as_str() == "click" && args.len() >= 4
                    && args[2] == Value::from(Button::LeftClick)
                    && args[3] == Value::from(Modifier::Alt) {
                    match &args[1] {
                        Value::Coord { value } => {
                            self.destination = Some(map_pos_to_tile_pos(*value));
                            self.tile_pos_path.clear();
                            debug!("PathFinder: set destination: {:?}", self.destination);
                        }
                        v => warn!("PathFinder: invalid click args[1]: {:?}", v),
                    }
                }
            }
            _ => (),
        }
    }
}
