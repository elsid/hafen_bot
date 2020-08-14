use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bot::vec2::{Vec2f, Vec2i};

pub const GRID_SIZE: i32 = 100;
pub const TILE_SIZE: f64 = 11.0;
pub const RESOLUTION: f64 = hexf64!("0x1.0p-10") * TILE_SIZE;

pub struct Map {
    tiles: BTreeMap<i32, Tile>,
    tiles_by_name: BTreeMap<String, i32>,
    grids: BTreeMap<i64, Grid>,
    grids_by_coord: BTreeMap<i64, BTreeMap<Vec2i, i64>>,
}

impl Map {
    pub fn new() -> Self {
        Self {
            tiles_by_name: BTreeMap::new(),
            tiles: BTreeMap::new(),
            grids_by_coord: BTreeMap::new(),
            grids: BTreeMap::new(),
        }
    }

    pub fn from_map_data(map_data: MapData) -> Self {
        let MapData { tiles, grids } = map_data;
        Self {
            tiles_by_name: tiles.iter().map(|v| (v.name.clone(), v.id)).collect(),
            tiles: tiles.into_iter().map(|v| (v.id, v)).collect(),
            grids_by_coord: make_grids_by_coord(&grids),
            grids: grids.into_iter().map(|v| (v.id, v)).collect(),
        }
    }

    pub fn as_map_data(&self) -> MapData {
        MapData {
            tiles: self.tiles.values().cloned().collect(),
            grids: self.grids.values().cloned().collect(),
        }
    }

    pub fn set_tile(&mut self, tile: Tile) {
        self.tiles_by_name.insert(tile.name.clone(), tile.id);
        self.tiles.insert(tile.id, tile);
    }

    pub fn add_grid(&mut self, mut grid: Grid, neighbours: Vec<GridNeighbour>) {
        let mut segments = neighbours.into_iter()
            .filter_map(|v| self.grids.get(&v.id).map(|g| (g.segment_id, v.offset, g.position)))
            .collect::<Vec<_>>();
        if !segments.is_empty() {
            segments.sort_by_key(|(segment_id, _, _)| *segment_id);
            segments.dedup_by_key(|(segment_id, _, _)| *segment_id);
            segments.sort_by_key(|(segment_id, _, _)| {
                (std::usize::MAX - self.grids_by_coord[segment_id].len(), *segment_id)
            });
            let (target_segment_id, target_offset, target_position) = segments[0];
            grid.segment_id = target_segment_id;
            grid.position = target_position - target_offset;
            for i in 1..segments.len() {
                let (segment_id, offset, position) = segments[i];
                let shift = target_position - target_offset + offset - position;
                let segment_grids = self.grids_by_coord.remove(&segment_id).unwrap();
                for &grid_id in segment_grids.values() {
                    let grid = self.grids.get_mut(&grid_id).unwrap();
                    grid.segment_id = target_segment_id;
                    grid.position += shift;
                    grid.revision += 1;
                    self.grids_by_coord.get_mut(&target_segment_id).unwrap().insert(grid.position, grid_id);
                }
            }
        }
        self.grids_by_coord.entry(grid.segment_id)
            .or_insert_with(|| BTreeMap::new())
            .insert(grid.position, grid.id);
        self.grids.insert(grid.id, grid);
    }

    pub fn update_grid(&mut self, grid: Grid) {
        if let Some(position) = self.grids.get(&grid.id).map(|v| v.position) {
            let shift = grid.position - position;
            if shift != Vec2i::zero() {
                for existing in self.grids.values_mut().filter(|v| v.segment_id == grid.segment_id) {
                    existing.position += shift;
                }
                if let Some(segment) = self.grids_by_coord.get_mut(&grid.segment_id) {
                    *segment = segment.into_iter().map(|(position, grid_id)| (*position + shift, *grid_id)).collect();
                }
            }
        }
        self.grids.insert(grid.id, grid);
    }

    pub fn get_tile(&self, segment_id: i64, tile_pos: Vec2i) -> Option<i32> {
        let grid_pos = tile_pos_to_grid_pos(tile_pos);
        if let Some(grid) = self.get_grid(segment_id, grid_pos) {
            let relative_tile_pos = tile_pos_to_relative_tile_pos(tile_pos, grid_pos);
            return Some(grid.tiles[get_grid_tile_index(relative_tile_pos)]);
        }
        None
    }

    pub fn get_grid(&self, segment_id: i64, grid_pos: Vec2i) -> Option<&Grid> {
        self.grids_by_coord.get(&segment_id)
            .and_then(|v| v.get(&grid_pos))
            .and_then(|id| self.grids.get(&id))
    }

    pub fn get_grid_by_id(&self, id: i64) -> Option<&Grid> {
        self.grids.get(&id)
    }

    pub fn get_tile_id_by_name(&self, name: &String) -> Option<i32> {
        self.tiles_by_name.get(name).map(|v| *v)
    }

    pub fn find_border_tiles(&self, segment_id: i64, allowed_tiles: &impl TileSet) -> Vec<Vec2i> {
        let mut result = Vec::new();
        if let Some(segment_grids) = self.grids_by_coord.get(&segment_id) {
            for (&grid_pos, grid_id) in segment_grids.iter() {
                let grid = &self.grids[grid_id];
                if !segment_grids.contains_key(&(grid_pos - Vec2i::only_x(1))) {
                    for y in 0..GRID_SIZE {
                        let relative_tile_pos = Vec2i::only_y(y);
                        let tile = grid.tiles[get_grid_tile_index(relative_tile_pos)];
                        if allowed_tiles.contains(tile) {
                            result.push(make_tile_pos(grid_pos, relative_tile_pos));
                        }
                    }
                }
                if !segment_grids.contains_key(&(grid_pos + Vec2i::only_x(1))) {
                    for y in 0..GRID_SIZE {
                        let relative_tile_pos = Vec2i::new(GRID_SIZE - 1, y);
                        let tile = grid.tiles[get_grid_tile_index(relative_tile_pos)];
                        if allowed_tiles.contains(tile) {
                            result.push(make_tile_pos(grid_pos, relative_tile_pos));
                        }
                    }
                }
                if !segment_grids.contains_key(&(grid_pos - Vec2i::only_y(1))) {
                    for x in 0..GRID_SIZE {
                        let relative_tile_pos = Vec2i::new(x, 0);
                        let tile = grid.tiles[get_grid_tile_index(relative_tile_pos)];
                        if allowed_tiles.contains(tile) {
                            result.push(make_tile_pos(grid_pos, relative_tile_pos));
                        }
                    }
                }
                if !segment_grids.contains_key(&(grid_pos + Vec2i::only_y(1))) {
                    for x in 0..GRID_SIZE {
                        let relative_tile_pos = Vec2i::new(x, GRID_SIZE - 1);
                        let tile = grid.tiles[get_grid_tile_index(relative_tile_pos)];
                        if allowed_tiles.contains(tile) {
                            result.push(make_tile_pos(grid_pos, relative_tile_pos));
                        }
                    }
                }
            }
        }
        result
    }
}

pub fn rel_tile_pos_to_pos(tile_pos: Vec2f) -> Vec2f {
    tile_pos * TILE_SIZE
}

pub fn pos_to_rel_tile_pos(pos: Vec2f) -> Vec2f {
    pos / TILE_SIZE
}

pub fn pos_to_tile_pos(pos: Vec2f) -> Vec2i {
    Vec2i::from(pos_to_rel_tile_pos(pos).floor())
}

pub fn tile_pos_to_pos(tile_pos: Vec2i) -> Vec2f {
    rel_tile_pos_to_pos(Vec2f::from(tile_pos))
}

pub fn map_pos_to_pos(map_pos: Vec2i) -> Vec2f {
    map_pos.center() * RESOLUTION
}

pub fn pos_to_map_pos(pos: Vec2f) -> Vec2i {
    Vec2i::from(pos.floor_by(RESOLUTION))
}

pub fn pos_to_grid_pos(pos: Vec2f) -> Vec2i {
    tile_pos_to_grid_pos(pos_to_tile_pos(pos))
}

fn tile_pos_to_relative_tile_pos(tile_pos: Vec2i, grid_pos: Vec2i) -> Vec2i {
    tile_pos - grid_pos_to_tile_pos(grid_pos)
}

pub fn grid_pos_to_tile_pos(grid_pos: Vec2i) -> Vec2i {
    grid_pos * GRID_SIZE
}

fn get_grid_tile_index(tile_pos: Vec2i) -> usize {
    tile_pos.x() as usize + tile_pos.y() as usize * GRID_SIZE as usize
}

pub fn make_tile_pos(grid_pos: Vec2i, relative_tile_pos: Vec2i) -> Vec2i {
    grid_pos_to_tile_pos(grid_pos) + relative_tile_pos
}

pub fn tile_pos_to_grid_pos(tile_pos: Vec2i) -> Vec2i {
    tile_pos.floor_div_i32(GRID_SIZE)
}

fn make_grids_by_coord(grids: &Vec<Grid>) -> BTreeMap<i64, BTreeMap<Vec2i, i64>> {
    let mut grids_by_coord = BTreeMap::new();
    for grid in grids.iter() {
        grids_by_coord.entry(grid.segment_id)
            .or_insert_with(|| BTreeMap::new())
            .insert(grid.position, grid.id);
    }
    grids_by_coord
}

#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Tile {
    pub id: i32,
    pub version: i32,
    pub name: String,
    pub color: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct Grid {
    pub id: i64,
    pub revision: i64,
    pub segment_id: i64,
    pub position: Vec2i,
    pub heights: Vec<f32>,
    pub tiles: Vec<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct GridNeighbour {
    pub id: i64,
    pub offset: Vec2i,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct MapData {
    tiles: Vec<Tile>,
    grids: Vec<Grid>,
}

pub trait TileSet {
    fn contains(&self, tile: i32) -> bool;
}

#[cfg(test)]
mod tests {
    use std::iter::repeat;

    use super::*;

    #[test]
    fn added_tile_should_be_accessible() {
        let mut map = Map::new();
        let grid = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::new(42, 13),
            heights: repeat(1.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        map.add_grid(grid.clone(), Vec::new());
        assert_eq!(map.get_grid_by_id(1), Some(&grid));
        assert_eq!(map.get_grid(1, Vec2i::new(42, 13)), Some(&grid));
        assert_eq!(map.get_tile(1, grid_pos_to_tile_pos(Vec2i::new(42, 13))), Some(1));
    }

    #[test]
    fn adjacent_grids_should_be_stored_in_a_single_segment() {
        let mut map = Map::new();
        let grid1 = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::zero(),
            heights: Vec::new(),
            tiles: Vec::new(),
        };
        let grid2 = Grid {
            id: 2,
            revision: 1,
            segment_id: 2,
            position: Vec2i::new(1, 0),
            heights: Vec::new(),
            tiles: Vec::new(),
        };
        map.add_grid(grid1, Vec::new());
        map.add_grid(grid2, vec![GridNeighbour { id: 1, offset: Vec2i::new(-1, 0) }]);
        assert_eq!(map.get_grid(1, Vec2i::zero()).map(|v| (v.id, v.segment_id)), Some((1, 1)));
        assert_eq!(map.get_grid(1, Vec2i::new(1, 0)).map(|v| (v.id, v.segment_id)), Some((2, 1)));
    }

    #[test]
    fn separate_grids_should_be_stored_in_different_segments() {
        let mut map = Map::new();
        let grid1 = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::zero(),
            heights: Vec::new(),
            tiles: Vec::new(),
        };
        let grid2 = Grid {
            id: 2,
            revision: 1,
            segment_id: 2,
            position: Vec2i::zero(),
            heights: Vec::new(),
            tiles: Vec::new(),
        };
        map.add_grid(grid1, Vec::new());
        map.add_grid(grid2, Vec::new());
        assert_eq!(map.get_grid(1, Vec2i::zero()).map(|v| (v.id, v.segment_id)), Some((1, 1)));
        assert_eq!(map.get_grid(2, Vec2i::zero()).map(|v| (v.id, v.segment_id)), Some((2, 2)));
    }

    #[test]
    fn adjacent_grid_to_separated_segments_should_merge_them() {
        let mut map = Map::new();
        let grid1 = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::zero(),
            heights: Vec::new(),
            tiles: Vec::new(),
        };
        let grid2 = Grid {
            id: 2,
            revision: 1,
            segment_id: 2,
            position: Vec2i::zero(),
            heights: Vec::new(),
            tiles: Vec::new(),
        };
        let grid3 = Grid {
            id: 3,
            revision: 1,
            segment_id: 2,
            position: Vec2i::new(1, 0),
            heights: Vec::new(),
            tiles: Vec::new(),
        };
        let adjacent_grid = Grid {
            id: 4,
            revision: 1,
            segment_id: 3,
            position: Vec2i::zero(),
            heights: Vec::new(),
            tiles: Vec::new(),
        };
        map.add_grid(grid1, Vec::new());
        map.add_grid(grid2, Vec::new());
        map.add_grid(grid3, Vec::new());
        map.add_grid(adjacent_grid, vec![
            GridNeighbour { id: 1, offset: Vec2i::new(-1, -1) },
            GridNeighbour { id: 2, offset: Vec2i::new(1, 0) },
        ]);
        assert_eq!(map.get_grid_by_id(1).map(|v| (v.id, v.segment_id, v.position)), Some((1, 2, Vec2i::new(-2, -1))));
        assert_eq!(map.get_grid_by_id(2).map(|v| (v.id, v.segment_id, v.position)), Some((2, 2, Vec2i::new(0, 0))));
        assert_eq!(map.get_grid_by_id(3).map(|v| (v.id, v.segment_id, v.position)), Some((3, 2, Vec2i::new(1, 0))));
        assert_eq!(map.get_grid_by_id(4).map(|v| (v.id, v.segment_id, v.position)), Some((4, 2, Vec2i::new(-1, 0))));
    }

    #[test]
    fn get_grid_should_return_none_for_absent_grid() {
        let mut map = Map::new();
        let grid = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::new(42, 13),
            heights: repeat(1.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        map.add_grid(grid.clone(), Vec::new());
        assert_eq!(map.get_grid(1, Vec2i::zero()), None);
    }

    #[test]
    fn get_grid_should_return_none_for_absent_segment() {
        let mut map = Map::new();
        let grid = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::new(42, 13),
            heights: repeat(1.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        map.add_grid(grid.clone(), Vec::new());
        assert_eq!(map.get_grid(2, Vec2i::new(42, 13)), None);
    }

    #[test]
    fn get_tile_should_return_none_for_absent_grid() {
        let mut map = Map::new();
        let grid = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::new(42, 13),
            heights: repeat(1.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        map.add_grid(grid.clone(), Vec::new());
        assert_eq!(map.get_tile(1, Vec2i::zero()), None);
    }

    #[test]
    fn get_tile_should_return_none_for_absent_segment() {
        let mut map = Map::new();
        let grid = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::new(42, 13),
            heights: repeat(1.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        map.add_grid(grid.clone(), Vec::new());
        assert_eq!(map.get_tile(2, grid_pos_to_tile_pos(Vec2i::new(42, 13))), None);
    }

    #[test]
    fn update_grid_should_change_grid_position() {
        let mut map = Map::new();
        let mut grid = Grid {
            id: 1,
            revision: 1,
            segment_id: 1,
            position: Vec2i::new(42, 13),
            heights: repeat(1.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        map.add_grid(grid.clone(), Vec::new());
        grid.position = Vec2i::new(13, 42);
        map.update_grid(grid.clone());
        assert_eq!(map.get_grid_by_id(1), Some(&grid));
        assert_eq!(map.get_grid(1, Vec2i::new(13, 42)), Some(&grid));
        assert_eq!(map.get_tile(1, grid_pos_to_tile_pos(Vec2i::new(13, 42))), Some(1));
    }
}
