use std::collections::{BinaryHeap, BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use graphics::{Line, Rectangle, Transformed};
use graphics::math::identity;
use graphics::rectangle::square;
use serde::{Deserialize, Serialize};

use crate::bot::map::{Grid, grid_pos_to_tile_pos, GridNeighbour, Map, MapData, pos_to_grid_pos, rel_tile_pos_to_pos, Tile, tile_pos_to_pos, TILE_SIZE, TileSet};
use crate::bot::map_db::MapDb;
use crate::bot::math::as_score;
use crate::bot::objects::{Object, Objects, ObjectsData};
use crate::bot::player::{Item, Player, PlayerEquipment, Resource, Widget};
use crate::bot::protocol::{Event, MapGrid, Update};
use crate::bot::scene::{ArrowNode, CompositeBTreeMapNode, insert_to_composite_node_btree_map, Node, RectangleNode, remove_from_composite_node_btree_map};
use crate::bot::vec2::{Vec2f, Vec2i};
use crate::bot::walk_grid::walk_grid;

#[derive(Clone, Deserialize)]
pub struct WorldConfig {
    pub water_tiles: HashMap<String, f64>,
    pub ice_tiles: HashMap<String, f64>,
    pub report_iterations: usize,
    pub found_transition_color: [f32; 4],
    pub path_transition_color: [f32; 4],
    pub shorten_path_transition_color: [f32; 4],
    pub direct_path_transition_color: [f32; 4],
}

pub struct World {
    revision: u64,
    objects: Objects,
    map: Map,
    config: WorldConfig,
}

impl World {
    pub fn new(config: WorldConfig, map_db: Arc<Mutex<dyn MapDb + Send>>) -> Self {
        Self {
            revision: 0,
            objects: Objects::new(),
            map: Map::new(map_db),
            config,
        }
    }

    pub fn from_world_data(data: WorldData, config: WorldConfig, map_db: Arc<Mutex<dyn MapDb + Send>>) -> Self {
        Self {
            revision: data.revision,
            objects: Objects::from_objects_data(data.objects),
            map: Map::from_map_data(data.map, map_db),
            config,
        }
    }

    pub fn as_world_data(&self) -> WorldData {
        WorldData {
            revision: self.revision,
            objects: self.objects.as_objects_data(),
            map: self.map.as_map_data(),
        }
    }

    pub fn objects(&self) -> &Objects {
        &self.objects
    }

    pub fn for_player<'a>(&'a self, player: &'a Player) -> Option<PlayerWorld<'a>> {
        if let (
            Some(map_view_id),
            Some(game_ui_id),
            Some(player_name),
            Some(player_object_id),
            Some(player_grid_id),
            Some(player_stamina),
            Some(player_equipment),
        ) = (
            player.map_view_id(),
            player.game_ui_id(),
            player.name(),
            player.object_id(),
            player.grid_id(),
            player.stamina(),
            player.equipment(),
        ) {
            self.objects.get_by_id(player_object_id).map(|v| v.position)
                .and_then(|player_position| {
                    self.map.get_grid_by_id(player_grid_id)
                        .map(|grid| {
                            let grid_pos = pos_to_grid_pos(player_position);
                            (grid.segment_id, grid.position - grid_pos)
                        })
                        .map(|(player_segment_id, player_grid_offset)| {
                            PlayerWorld {
                                revision: self.revision,
                                map_view_id,
                                game_ui_id,
                                player,
                                player_name,
                                player_object_id,
                                player_position,
                                player_grid_id,
                                player_segment_id,
                                player_grid_offset,
                                player_stamina,
                                player_equipment,
                                objects: &self.objects,
                                map: &self.map,
                                config: &self.config,
                            }
                        })
                })
        } else {
            None
        }
    }

    pub fn update(&mut self, update: Update) -> bool {
        if self.apply_update(update) {
            self.revision += 1;
            true
        } else {
            false
        }
    }

    fn apply_update(&mut self, update: Update) -> bool {
        match update.event {
            Event::MapTile { id, version, name, color } => {
                self.map.set_tile(Tile { id, version, name, color });
                true
            }
            Event::MapGridAdd { grid, neighbours } => {
                self.update_map(grid, neighbours);
                true
            }
            Event::MapGridUpdate { grid } => {
                self.update_map(grid, Vec::new());
                true
            }
            Event::GobAdd { id, position, angle, name } => {
                self.objects.add(Object { id, position, angle, name });
                true
            }
            Event::GobRemove { id } => {
                self.objects.remove(id)
            }
            Event::GobMove { id, position, angle } => {
                self.objects.update(id, position, angle)
            }
            _ => false,
        }
    }

    fn update_map(&mut self, grid: MapGrid, neighbours: Vec<GridNeighbour>) {
        if let Some(existing) = self.map.get_grid_by_id(grid.id) {
            let map_grid = Grid {
                id: existing.id,
                segment_id: existing.segment_id,
                revision: existing.revision + 1,
                position: grid.position,
                heights: grid.heights,
                tiles: grid.tiles,
            };
            self.map.update_grid(map_grid);
        } else {
            let map_grid = Grid {
                id: grid.id,
                segment_id: grid.id,
                revision: 1,
                position: grid.position,
                heights: grid.heights,
                tiles: grid.tiles,
            };
            self.map.add_grid(map_grid, neighbours);
        }
    }
}

#[allow(dead_code)]
pub struct PlayerWorld<'a> {
    revision: u64,
    map_view_id: i32,
    game_ui_id: i32,
    player: &'a Player,
    player_name: &'a String,
    player_object_id: i64,
    player_position: Vec2f,
    player_grid_id: i64,
    player_segment_id: i64,
    player_grid_offset: Vec2i,
    player_stamina: i32,
    player_equipment: PlayerEquipment<'a>,
    objects: &'a Objects,
    map: &'a Map,
    config: &'a WorldConfig,
}

impl<'a> PlayerWorld<'a> {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn map_view_id(&self) -> i32 {
        self.map_view_id
    }

    pub fn game_ui_id(&self) -> i32 {
        self.game_ui_id
    }

    pub fn player_object_id(&self) -> i64 {
        self.player_object_id
    }

    pub fn player_position(&self) -> Vec2f {
        self.player_position
    }

    pub fn player_segment_id(&self) -> i64 {
        self.player_segment_id
    }

    pub fn player_grid_id(&self) -> i64 {
        self.player_grid_id
    }

    pub fn is_player_stuck(&self) -> bool {
        self.player.is_stuck()
    }

    pub fn player_stamina(&self) -> i32 {
        self.player_stamina
    }

    pub fn player_inventory_items(&self) -> &BTreeMap<i32, Item> {
        self.player.inventory_items()
    }

    pub fn player_belt_items(&self) -> &BTreeMap<i32, Item> {
        self.player.belt_items()
    }

    pub fn player_equipment(&self) -> &PlayerEquipment {
        &self.player_equipment
    }

    pub fn widgets(&self) -> &BTreeMap<i32, Widget> {
        self.player.widgets()
    }

    pub fn resources(&self) -> &BTreeMap<i32, Resource> {
        self.player.resources()
    }

    pub fn config(&self) -> &WorldConfig {
        self.config
    }

    pub fn get_object_by_name(&self, name: &String) -> Option<&Object> {
        self.objects.get_by_name(name)
    }

    pub fn get_tile_id_by_name(&self, name: &String) -> Option<i32> {
        self.map.get_tile_id_by_name(name)
    }

    pub fn get_tile_by_id(&self, id: i32) -> Option<&Tile> {
        self.map.get_tile_by_id(id)
    }

    pub fn get_tile(&self, tile_pos: Vec2i) -> Option<i32> {
        self.map.get_tile(
            self.player_segment_id,
            tile_pos + grid_pos_to_tile_pos(self.player_grid_offset),
        )
    }

    pub fn iter_grids(&self) -> impl Iterator<Item=&Grid> {
        self.map.iter_grids()
    }

    pub fn iter_objects(&self) -> impl Iterator<Item=&Object> {
        self.objects.iter()
    }

    pub fn objects_len(&self) -> usize {
        self.objects.len()
    }

    pub fn get_grid_by_id(&self, grid_id: i64) -> Option<&Grid> {
        self.map.get_grid_by_id(grid_id)
    }

    pub fn find_border_tiles(&self, weights: &impl TileWeights) -> Vec<Vec2i> {
        self.map.find_border_tiles(self.player_segment_id, weights)
    }

    pub fn find_path(&self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i, weights: &impl TileWeights,
                     max_shortcut_length: f64, max_iterations: usize,
                     node: &Arc<Mutex<Node>>, cancel: &Arc<AtomicBool>) -> Vec<Vec2i> {
        if src_tile_pos == dst_tile_pos {
            return vec![dst_tile_pos];
        }
        let mut transitions = Transitions::new(
            node,
            &self.config.direct_path_transition_color,
            &self.config.found_transition_color,
            &self.config.shorten_path_transition_color,
        );
        transitions.add_direct_path(src_tile_pos, dst_tile_pos);
        let path = self.find_reversed_tiles_path(src_tile_pos, dst_tile_pos, weights, max_iterations, &mut transitions, cancel);
        transitions.add_path(src_tile_pos, &path, true, self.config.path_transition_color);
        let shorten_path = self.shorten_reversed_tiles_path(path, weights, max_shortcut_length);
        transitions.add_shorten_path(src_tile_pos, &shorten_path);
        shorten_path
    }

    fn find_reversed_tiles_path(&self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i,
                                weights: &impl TileWeights, max_iterations: usize,
                                transitions: &mut Transitions, cancel: &Arc<AtomicBool>) -> Vec<Vec2i> {
        let mut ordered = BinaryHeap::new();
        let mut costs: BTreeMap<Vec2i, f64> = BTreeMap::new();
        let mut backtrack = BTreeMap::new();
        let mut open_set = BTreeSet::new();

        let initial_distance = src_tile_pos.center().distance(dst_tile_pos.center());
        costs.insert(src_tile_pos, 0.0);
        ordered.push((as_score(initial_distance), src_tile_pos));

        const EDGES: &[(Vec2i, f64)] = &[
            (Vec2i::new(-1, -1), std::f64::consts::SQRT_2),
            (Vec2i::new(-1, 0), 1.0),
            (Vec2i::new(-1, 1), std::f64::consts::SQRT_2),
            (Vec2i::new(0, -1), 1.0),
            (Vec2i::new(0, 1), 1.0),
            (Vec2i::new(1, -1), std::f64::consts::SQRT_2),
            (Vec2i::new(1, 0), 1.0),
            (Vec2i::new(1, 1), std::f64::consts::SQRT_2),
        ];

        let mut iterations: usize = 0;
        let mut push_count: usize = 0;
        let mut min_distance = src_tile_pos.center().distance(dst_tile_pos.center());

        debug!("find_reversed_tiles_path src_tile_pos={:?} dst_tile_pos={:?} distance={}",
               src_tile_pos, dst_tile_pos, min_distance);

        let get_weight = |tile_pos| self.get_tile(tile_pos).and_then(|tile| weights.get(tile));
        let is_reachable = |tile_pos| {
            if let Some(tile) = self.get_tile(tile_pos) {
                weights.get(tile).is_some()
            } else {
                true
            }
        };

        if !is_reachable(dst_tile_pos) {
            return Vec::new();
        }

        while let Some((_, tile_pos)) = ordered.pop() {
            min_distance = min_distance.min(tile_pos.center().distance(dst_tile_pos.center()));
            if tile_pos == dst_tile_pos {
                debug!("find_reversed_tiles_path found iterations={} ordered={} costs={} push_count={} min_distance={}",
                       iterations, ordered.len(), costs.len(), push_count, min_distance);
                return reconstruct_path(src_tile_pos, dst_tile_pos, backtrack);
            }
            if cancel.load(Ordering::Relaxed) {
                debug!("find_reversed_tiles_path cancelled");
                break;
            }
            if iterations >= max_iterations {
                debug!("find_reversed_tiles_path reached max iterations");
                break;
            }
            open_set.remove(&tile_pos);
            if let Some(tile) = self.get_tile(tile_pos) {
                if let Some(weight) = weights.get(tile) {
                    for &(shift, distance) in EDGES.iter() {
                        let next_tile_pos = tile_pos + shift;
                        if let Some(next_weight) = get_weight(next_tile_pos) {
                            if distance != 1.0 {
                                if !is_reachable(tile_pos + shift.with_x(0))
                                    || !is_reachable(tile_pos + shift.with_y(0)) {
                                    continue;
                                }
                            }
                            let right = next_tile_pos + Vec2i::only_x(1);
                            let left = next_tile_pos - Vec2i::only_x(1);
                            let top = next_tile_pos + Vec2i::only_y(1);
                            let bottom = next_tile_pos - Vec2i::only_y(1);
                            if right != tile_pos && !is_reachable(right)
                                || left != tile_pos && !is_reachable(left)
                                || top != tile_pos && !is_reachable(top)
                                || bottom != tile_pos && !is_reachable(bottom) {
                                continue;
                            }
                            let next_cost = costs[&tile_pos] + distance * (weight + next_weight) / 2.0;
                            let other_cost = *costs.get(&next_tile_pos).unwrap_or(&std::f64::MAX);
                            if next_cost < other_cost {
                                backtrack.insert(next_tile_pos, tile_pos);
                                costs.insert(next_tile_pos, next_cost);
                                if open_set.insert(next_tile_pos) {
                                    let next_score = next_cost + next_tile_pos.center().distance(dst_tile_pos.center());
                                    ordered.push((-as_score(next_score), next_tile_pos));
                                    push_count += 1;
                                }
                            }
                            transitions.update_found(tile_pos, next_tile_pos);
                        }
                    }
                }
            }
            iterations += 1;
            if iterations % self.config.report_iterations == 0 {
                debug!("find_reversed_tiles_path iterations={} ordered={} costs={} push_count={} min_distance={}",
                       iterations, ordered.len(), costs.len(), push_count, min_distance);
            }
        }

        debug!("find_reversed_tiles_path not found iterations={} ordered={} costs={} push_count={} min_distance={}",
               iterations, ordered.len(), costs.len(), push_count, min_distance);

        Vec::new()
    }

    fn shorten_reversed_tiles_path(&self, reversed_tiles_path: Vec<Vec2i>,
                                   allowed_tiles: &impl TileSet, max_shortcut_length: f64) -> Vec<Vec2i> {
        if reversed_tiles_path.len() < 2 {
            return reversed_tiles_path;
        }

        let mut result = Vec::new();
        let mut last = reversed_tiles_path.len() - 1;
        let mut current = reversed_tiles_path[last];

        while last > 0 {
            let mut index = 0;
            while index < last && !self.is_valid_shortcut(
                current,
                reversed_tiles_path[index],
                allowed_tiles,
                max_shortcut_length,
            ) {
                index += 1;
            }
            if index == last {
                result.push(reversed_tiles_path[index]);
                last -= 1;
            } else {
                current = reversed_tiles_path[index];
                last = index;
                result.push(reversed_tiles_path[last]);
            }
        }

        result
    }

    pub fn is_valid_shortcut(&self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i,
                             allowed_tiles: &impl TileSet, max_length: f64) -> bool {
        if src_tile_pos.x() == dst_tile_pos.x() {
            self.is_valid_shortcut_by_x(src_tile_pos, dst_tile_pos, allowed_tiles, max_length)
        } else if src_tile_pos.y() == dst_tile_pos.y() {
            self.is_valid_shortcut_by_y(src_tile_pos, dst_tile_pos, allowed_tiles, max_length)
        } else {
            self.is_valid_shortcut_by_rel_pos(
                src_tile_pos.center(),
                dst_tile_pos.center(),
                allowed_tiles,
                max_length,
            )
        }
    }

    fn is_valid_shortcut_by_x(&self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i,
                              allowed_tiles: &impl TileSet, max_length: f64) -> bool {
        let mut y = src_tile_pos.y();
        let shift = if y < dst_tile_pos.y() { 1 } else { -1 };
        while y != dst_tile_pos.y() {
            if (src_tile_pos.y() - y).abs() as f64 > max_length {
                return false;
            }
            if let Some(tile) = self.get_tile(src_tile_pos.with_y(y)) {
                if allowed_tiles.contains(tile) {
                    y += shift;
                    continue;
                }
            }
            return false;
        }
        true
    }

    fn is_valid_shortcut_by_y(&self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i,
                              allowed_tiles: &impl TileSet, max_length: f64) -> bool {
        let mut x = src_tile_pos.x();
        let shift = if x < dst_tile_pos.x() { 1 } else { -1 };
        while x != dst_tile_pos.x() {
            if (src_tile_pos.x() - x).abs() as f64 > max_length {
                return false;
            }
            if let Some(tile) = self.get_tile(src_tile_pos.with_x(x)) {
                if allowed_tiles.contains(tile) {
                    x += shift;
                    continue;
                }
            }
            return false;
        }
        true
    }

    pub fn is_valid_shortcut_by_rel_pos(&self, src_rel_tile_pos: Vec2f, dst_rel_tile_pos: Vec2f,
                                        allowed_tiles: &impl TileSet, max_length: f64) -> bool {
        let is_allowed = |tile_pos| {
            if let Some(tile) = self.get_tile(tile_pos) {
                allowed_tiles.contains(tile)
            } else {
                true
            }
        };
        let mut prev_tile_pos = None;
        walk_grid(src_rel_tile_pos, dst_rel_tile_pos, |position| {
            if src_rel_tile_pos.distance(position) > max_length {
                return false;
            }
            let tile_pos = Vec2i::from(position.floor());
            if !is_allowed(tile_pos) {
                return false;
            }
            if let Some(prev) = prev_tile_pos {
                let shift = tile_pos - prev;
                if (shift.x() != 0 && !is_allowed(prev + shift.with_x(0)))
                    || (shift.y() != 0 && !is_allowed(prev + shift.with_y(0))) {
                    return false;
                }
            }
            prev_tile_pos = Some(tile_pos);
            true
        })
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct WorldData {
    revision: u64,
    objects: ObjectsData,
    map: MapData,
}

fn reconstruct_path(src_tile_pos: Vec2i, dst_tile_pos: Vec2i,
                    backtrack: BTreeMap<Vec2i, Vec2i>) -> Vec<Vec2i> {
    let mut result = vec![dst_tile_pos];
    let mut current = dst_tile_pos;
    loop {
        let prev = backtrack[&current];
        if prev == src_tile_pos {
            break;
        }
        result.push(prev);
        current = prev;
    }
    result
}

pub trait TileWeights: TileSet {
    fn get(&self, tile: i32) -> Option<f64>;
}

impl<T: TileWeights> TileSet for T {
    fn contains(&self, tile: i32) -> bool {
        self.get(tile).is_some()
    }
}

pub struct BTreeMapTileWeights<'a>(pub &'a BTreeMap<i32, f64>);

impl<'a> TileWeights for BTreeMapTileWeights<'a> {
    fn get(&self, tile: i32) -> Option<f64> {
        self.0.get(&tile).map(|v| *v)
    }
}

pub fn make_find_path_node() -> Arc<Mutex<Node>> {
    Arc::new(Mutex::new(Node::CompositeBTreeMap(CompositeBTreeMapNode::default())))
}

struct Transitions<'a> {
    node: &'a Arc<Mutex<Node>>,
    id_counter: usize,
    values: BTreeMap<Vec2i, (Vec2i, usize)>,
    direct_path_transition_color: &'a [f32; 4],
    found_transition_color: &'a [f32; 4],
    shorten_path_transition_color: &'a [f32; 4],
}

impl<'a> Transitions<'a> {
    fn new(node: &'a Arc<Mutex<Node>>, direct_path_transition_color: &'a [f32; 4],
           found_transition_color: &'a [f32; 4], shorten_path_transition_color: &'a [f32; 4]) -> Self {
        Self {
            node,
            id_counter: 0,
            values: BTreeMap::new(),
            direct_path_transition_color,
            found_transition_color,
            shorten_path_transition_color,
        }
    }

    fn add_direct_path(&mut self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i) {
        self.id_counter += 1;
        let src = rel_tile_pos_to_pos(src_tile_pos.center());
        let dst = rel_tile_pos_to_pos(dst_tile_pos.center());
        self.id_counter += 1;
        insert_to_composite_node_btree_map(self.node, self.id_counter, Node::from(ArrowNode {
            value: Line::new(self.direct_path_transition_color.clone(), 0.2),
            line: [src.x(), src.y(), dst.x(), dst.y()],
            head_size: 2.0,
            transform: identity(),
        }));
    }

    fn set(&mut self, src: Vec2i, dst: Vec2i) -> (usize, Option<usize>) {
        self.id_counter += 1;
        (self.id_counter, self.values.insert(dst, (src, self.id_counter)).map(|(_, id)| id))
    }

    fn update_found(&mut self, tile_pos: Vec2i, next_tile_pos: Vec2i) {
        let (new_id, old_id) = self.set(tile_pos, next_tile_pos);
        if let Some(id) = old_id {
            remove_from_composite_node_btree_map(self.node, id);
        }
        let src = rel_tile_pos_to_pos(tile_pos.center());
        let dst = rel_tile_pos_to_pos(next_tile_pos.center());
        insert_to_composite_node_btree_map(self.node, new_id, Node::from(ArrowNode {
            value: Line::new(self.found_transition_color.clone(), 0.2),
            line: [src.x(), src.y(), dst.x(), dst.y()],
            head_size: 2.0,
            transform: identity(),
        }));
    }

    fn add_path(&mut self, src_tile_pos: Vec2i, path: &Vec<Vec2i>, reversed: bool, color: [f32; 4]) {
        if path.is_empty() {
            return;
        }
        if reversed {
            self.add_path_arrow_transition(src_tile_pos, path[path.len() - 1], color);
        } else {
            self.add_path_arrow_transition(src_tile_pos, path[0], color);
        }
        for i in 0..path.len() - 1 {
            if reversed {
                self.add_path_arrow_transition(path[i + 1], path[i], color);
            } else {
                self.add_path_arrow_transition(path[i], path[i + 1], color);
            }
        }
    }

    fn add_shorten_path(&mut self, src_tile_pos: Vec2i, path: &Vec<Vec2i>) {
        if path.is_empty() {
            return;
        }
        self.add_path_tiles_transition(src_tile_pos, path[0]);
        self.add_path(src_tile_pos, path, false, self.shorten_path_transition_color.clone());
        for i in 0..path.len() - 1 {
            self.add_path_tiles_transition(path[i], path[i + 1]);
        }
    }

    fn add_path_arrow_transition(&mut self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i, color: [f32; 4]) {
        let src = rel_tile_pos_to_pos(src_tile_pos.center());
        let dst = rel_tile_pos_to_pos(dst_tile_pos.center());
        self.id_counter += 1;
        insert_to_composite_node_btree_map(self.node, self.id_counter, Node::from(ArrowNode {
            value: Line::new(color, 0.2),
            line: [src.x(), src.y(), dst.x(), dst.y()],
            head_size: 2.0,
            transform: identity(),
        }));
    }

    fn add_path_tiles_transition(&mut self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i) {
        walk_grid(src_tile_pos.center(), dst_tile_pos.center(), |position| {
            self.id_counter += 1;
            let rect_pos = tile_pos_to_pos(Vec2i::from(position.floor()));
            insert_to_composite_node_btree_map(self.node, self.id_counter, Node::from(RectangleNode {
                value: Rectangle::new([0.4, 0.8, 0.4, 0.6]),
                rectangle: square(0.0, 0.0, TILE_SIZE),
                transform: identity().trans(rect_pos.x(), rect_pos.y()),
            }));
            true
        });
    }
}
