use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use graphics::{Rectangle, Transformed};
use graphics::math::identity;
use graphics::rectangle::square;

use crate::bot::clusterization::{get_cluster_median, make_adjacent_tiles_clusters};
use crate::bot::map::{pos_to_map_pos, pos_to_rel_tile_pos, pos_to_tile_pos, rel_tile_pos_to_pos, tile_pos_to_pos, TILE_SIZE};
use crate::bot::math::as_score;
use crate::bot::protocol::{Button, Message, Modifier, Update, Value};
use crate::bot::scene::{CompositeVecNode, Layer, MapTransformArcNode, MapTransformBoxNode, Node, RectangleNode, Scene};
use crate::bot::tasks::task::Task;
use crate::bot::vec2::Vec2i;
use crate::bot::world::{BTreeMapTileWeights, make_find_path_node, PlayerWorld};

const WATER_TILES_COST: &'static [(&'static str, f64)] = &[
    ("gfx/tiles/water", 3.0),
    ("gfx/tiles/deep", 1.0),
];
const MAX_FIND_PATH_SHORTCUT_LENGTH: f64 = 25.0;
const MAX_NEXT_POINT_SHORTCUT_LENGTH: f64 = 50.0;
const MAX_ITERATIONS: usize = 10 * 1_000_000;

pub struct Explorer {
    border_tiles: Vec<Vec2i>,
    tile_pos_path: VecDeque<Vec2i>,
    find_path_layer: Option<Layer>,
    border_tiles_layer: Option<Layer>,
}

impl Explorer {
    pub fn new() -> Self {
        Self {
            border_tiles: Vec::new(),
            tile_pos_path: VecDeque::new(),
            find_path_layer: None,
            border_tiles_layer: None,
        }
    }
}

impl Task for Explorer {
    fn name(&self) -> &'static str {
        "Explorer"
    }

    fn get_next_message(&mut self, world: &PlayerWorld, scene: &Scene) -> Option<Message> {
        let player_pos = world.player_position();
        let water_tiles_cost = WATER_TILES_COST.iter()
            .filter_map(|&(name, weight)| {
                world.get_tile_id_by_name(&String::from(name)).map(|id| (id, weight))
            })
            .collect::<BTreeMap<i32, f64>>();
        if self.border_tiles.is_empty() {
            let border_tiles = world.find_border_tiles(&BTreeMapTileWeights(&water_tiles_cost));
            let clusters = make_adjacent_tiles_clusters(&border_tiles);
            self.border_tiles = clusters.iter().filter_map(get_cluster_median).collect();
            self.border_tiles.sort_by_key(|&tile_pos| {
                -as_score(rel_tile_pos_to_pos(tile_pos.center()).distance(player_pos))
            });
            debug!("Explorer: found border tiles: {:?}", self.border_tiles);
            self.border_tiles_layer = Some(make_border_tiles_layer(scene.clone(), &self.border_tiles));
        }
        while let (true, Some(dst_tile_pos)) = (self.tile_pos_path.is_empty(), self.border_tiles.last()) {
            let find_path_node = make_find_path_node();
            self.find_path_layer = Some(Layer::new(
                scene.clone(),
                Arc::new(Mutex::new(
                    Node::from(MapTransformArcNode {
                        node: find_path_node.clone(),
                    })
                )),
            ));
            let src_tile_pos = pos_to_tile_pos(player_pos);
            self.tile_pos_path = VecDeque::from(world.find_path(
                src_tile_pos,
                *dst_tile_pos,
                &BTreeMapTileWeights(&water_tiles_cost),
                MAX_FIND_PATH_SHORTCUT_LENGTH,
                MAX_ITERATIONS,
                &find_path_node,
            ));
            if !self.tile_pos_path.is_empty() {
                debug!("Explorer: found path from {:?} to {:?} by tiles {:?}: {:?}",
                       src_tile_pos, dst_tile_pos, water_tiles_cost, self.tile_pos_path);
                break;
            }
            debug!("Explorer: path from {:?} to {:?} is not found by tiles {:?}",
                   src_tile_pos, dst_tile_pos, water_tiles_cost);
            self.border_tiles.pop();
            self.border_tiles_layer = Some(make_border_tiles_layer(scene.clone(), &self.border_tiles));
        }
        while self.tile_pos_path.len() >= 2 {
            let src_rel_tile_pos = pos_to_rel_tile_pos(player_pos);
            let dst_rel_tile_pos = self.tile_pos_path[1].center();
            if !world.is_valid_shortcut_by_rel_pos(
                src_rel_tile_pos,
                dst_rel_tile_pos,
                &BTreeMapTileWeights(&water_tiles_cost),
                MAX_NEXT_POINT_SHORTCUT_LENGTH,
            ) {
                break;
            }
            self.tile_pos_path.pop_front();
        }
        while let Some(&tile_pos) = self.tile_pos_path.front() {
            let distance = tile_pos_to_pos(tile_pos).distance(player_pos);
            if distance > (2.0 * TILE_SIZE).sqrt() && tile_pos != pos_to_tile_pos(player_pos) {
                debug!("Explorer: distance to the next path point {:?}: {}", tile_pos, distance);
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
        self.border_tiles.clear();
        None
    }

    fn update(&mut self, _: &PlayerWorld, _: &Update) {}
}

fn make_border_tiles_layer(scene: Scene, border_tiles: &Vec<Vec2i>) -> Layer {
    Layer::new(
        scene,
        Arc::new(Mutex::new(
            Node::from(MapTransformBoxNode {
                node: Box::new(make_border_tiles_node(border_tiles)),
            })
        )),
    )
}

fn make_border_tiles_node(border_tiles: &Vec<Vec2i>) -> Node {
    Node::from(CompositeVecNode {
        nodes: border_tiles.iter()
            .map(|tile_pos| {
                let position = tile_pos_to_pos(*tile_pos);
                Node::from(RectangleNode {
                    value: Rectangle::new([0.8, 0.4, 0.2, 0.9]),
                    rectangle: square(0.0, 0.0, TILE_SIZE),
                    transform: identity().trans(position.x(), position.y()),
                })
            })
            .collect()
    })
}
