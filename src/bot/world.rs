use std::collections::{BinaryHeap, BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::bot::common::as_score;
use crate::bot::map::{Grid, grid_pos_to_tile_pos, Map, MapData, pos_to_grid_pos, Tile, TileSet};
use crate::bot::objects::{Object, Objects, ObjectsData};
use crate::bot::player::{Player, Widget};
use crate::bot::protocol::{Event, Update};
use crate::bot::vec2::{Vec2f, Vec2i};
use crate::bot::walk_grid::WalkGrid;

const REPORT_ITERATIONS: usize = 1_000_000;

pub struct World {
    objects: Objects,
    map: Map,
}

impl World {
    pub fn new() -> Self {
        Self {
            objects: Objects::new(),
            map: Map::new(),
        }
    }

    pub fn from_world_data(data: WorldData) -> Self {
        Self {
            objects: Objects::from_objects_data(data.objects),
            map: Map::from_map_data(data.map),
        }
    }

    pub fn as_world_data(&self) -> WorldData {
        WorldData {
            objects: self.objects.as_objects_data(),
            map: self.map.as_map_data(),
        }
    }

    pub fn for_player<'a>(&'a self, player: &'a Player) -> Option<PlayerWorld<'a>> {
        if let (
            Some(map_view_id),
            Some(game_ui_id),
            Some(player_name),
            Some(player_object_id),
            Some(player_grid_id),
        ) = (
            player.map_view_id(),
            player.game_ui_id(),
            player.name(),
            player.object_id(),
            player.grid_id(),
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
                                map_view_id,
                                game_ui_id,
                                player_name,
                                player_object_id,
                                player_position,
                                player_grid_id,
                                player_segment_id,
                                player_grid_offset,
                                is_player_stuck: player.is_stuck(),
                                objects: &self.objects,
                                widgets: &player.widgets(),
                                map: &self.map,
                            }
                        })
                })
        } else {
            None
        }
    }

    pub fn update(&mut self, update: Update) -> bool {
        match update.event {
            Event::MapTile { id, version, name, color } => {
                self.map.set_tile(Tile { id, version, name, color });
                true
            }
            Event::MapGridAdd { grid, neighbours } => {
                let map_grid = Grid {
                    id: grid.id,
                    segment_id: grid.id,
                    revision: 1,
                    position: grid.position,
                    heights: grid.heights,
                    tiles: grid.tiles,
                };
                self.map.add_grid(map_grid, neighbours);
                true
            }
            Event::MapGridUpdate { grid } => {
                if let Some(existing) = self.map.get_grid_by_id(grid.id) {
                    let map_grid = Grid {
                        id: grid.id,
                        segment_id: existing.segment_id,
                        revision: existing.revision + 1,
                        position: existing.position,
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
                    self.map.add_grid(map_grid, Vec::new());
                }
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
}

pub struct PlayerWorld<'a> {
    map_view_id: i32,
    game_ui_id: i32,
    player_name: &'a String,
    player_object_id: i64,
    player_position: Vec2f,
    player_grid_id: i64,
    player_segment_id: i64,
    player_grid_offset: Vec2i,
    is_player_stuck: bool,
    widgets: &'a BTreeMap<i32, Widget>,
    objects: &'a Objects,
    map: &'a Map,
}

impl<'a> PlayerWorld<'a> {
    pub fn map_view_id(&self) -> i32 {
        self.map_view_id
    }

    pub fn player_object_id(&self) -> i64 {
        self.player_object_id
    }

    pub fn player_position(&self) -> Vec2f {
        self.player_position
    }

    pub fn is_player_stuck(&self) -> bool {
        self.is_player_stuck
    }

    pub fn get_object_by_name(&self, name: &String) -> Option<&Object> {
        self.objects.get_by_name(name)
    }

    pub fn get_tile_id_by_name(&self, name: &String) -> Option<i32> {
        self.map.get_tile_id_by_name(name)
    }

    pub fn get_tile(&self, tile_pos: Vec2i) -> Option<i32> {
        self.map.get_tile(
            self.player_segment_id,
            tile_pos + grid_pos_to_tile_pos(self.player_grid_offset),
        )
    }

    pub fn find_border_tiles(&self, weights: &impl TileWeights) -> Vec<Vec2i> {
        self.map.find_border_tiles(self.player_segment_id, weights)
    }

    pub fn find_path(&self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i, weights: &impl TileWeights,
                     max_shortcut_length: f64, max_iterations: usize) -> Vec<Vec2i> {
        let path = self.find_reversed_tiles_path(src_tile_pos, dst_tile_pos, weights, max_iterations);
        self.shorten_reversed_tiles_path(path, weights, max_shortcut_length)
    }

    fn find_reversed_tiles_path(&self, src_tile_pos: Vec2i, dst_tile_pos: Vec2i,
                                weights: &impl TileWeights, max_iterations: usize) -> Vec<Vec2i> {
        let mut ordered = BinaryHeap::new();
        let mut scores: BTreeMap<Vec2i, f64> = BTreeMap::new();
        let mut costs: BTreeMap<Vec2i, f64> = BTreeMap::new();
        let mut backtrack = BTreeMap::new();
        let mut open_set = BTreeSet::new();

        let initial_distance = src_tile_pos.center().distance(dst_tile_pos.center());
        costs.insert(src_tile_pos, 0.0);
        scores.insert(src_tile_pos, initial_distance);
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
        let mut visits: BTreeMap<Vec2i, usize> = BTreeMap::new();
        let mut max_visits: usize = 0;
        let mut max_visits_pos = Vec2i::zero();

        info!("find_reversed_tiles_path src_tile_pos={:?} dst_tile_pos={:?} distance={}",
              src_tile_pos, dst_tile_pos, min_distance);

        while let Some((_, tile_pos)) = ordered.pop() {
            {
                let count = visits.entry(tile_pos).or_insert(0);
                *count += 1;
                if max_visits < *count {
                    max_visits = *count;
                    max_visits_pos = tile_pos;
                }
                min_distance = min_distance.min(tile_pos.center().distance(dst_tile_pos.center()));
            }
            if tile_pos == dst_tile_pos {
                info!("find_reversed_tiles_path found iterations={} ordered={} costs={} push_count={} min_distance={} max_visits={} max_visits_pos={:?} max_visits_cost={:?}",
                      iterations, ordered.len(), costs.len(), push_count, min_distance, max_visits, max_visits_pos, costs.get(&max_visits_pos));
                return reconstruct_path(src_tile_pos, dst_tile_pos, backtrack);
            }
            if iterations >= max_iterations {
                info!("find_reversed_tiles_path not found iterations={} ordered={} costs={} push_count={} min_distance={} max_visits={} max_visits_pos={:?} max_visits_cost={:?}",
                      iterations, ordered.len(), costs.len(), push_count, min_distance, max_visits, max_visits_pos, costs.get(&max_visits_pos));
                break;
            }
            open_set.remove(&tile_pos);
            if let Some(tile) = self.get_tile(tile_pos) {
                if let Some(weight) = weights.get(tile) {
                    for &(shift, distance) in EDGES.iter() {
                        let next_tile_pos = tile_pos + shift;
                        if let Some(next_tile) = self.get_tile(next_tile_pos) {
                            if let Some(next_weight) = weights.get(next_tile) {
                                let next_cost = costs[&tile_pos] + distance * (weight + next_weight) / 2.0;
                                let other_cost = *costs.get(&next_tile_pos).unwrap_or(&std::f64::MAX);
                                if next_cost < other_cost {
                                    backtrack.insert(next_tile_pos, tile_pos);
                                    costs.insert(next_tile_pos, next_cost);
                                    let next_score = next_cost + next_tile_pos.center().distance(dst_tile_pos.center());
                                    scores.insert(next_tile_pos, next_score);
                                    if open_set.insert(next_tile_pos) {
                                        ordered.push((as_score(next_score), next_tile_pos));
                                        push_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            iterations += 1;
            if iterations % REPORT_ITERATIONS == 0 {
                info!("find_reversed_tiles_path iterations={} ordered={} costs={} push_count={} min_distance={} max_visits={} max_visits_pos={:?} max_visits_cost={:?}",
                      iterations, ordered.len(), costs.len(), push_count, min_distance, max_visits, max_visits_pos, costs.get(&max_visits_pos));
            }
        }

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
        for position in WalkGrid::new(src_rel_tile_pos, dst_rel_tile_pos) {
            if src_rel_tile_pos.distance(position) > max_length {
                return false;
            }
            if let Some(tile) = self.get_tile(position.floor()) {
                if allowed_tiles.contains(tile) {
                    continue;
                }
            }
            return false;
        }
        true
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WorldData {
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