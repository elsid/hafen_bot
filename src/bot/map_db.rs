use std::sync::{Arc, Mutex};

use crate::bot::map::{Grid, GridNeighbour, Tile};
use crate::bot::vec2::Vec2i;

pub trait MapDb {
    fn get_tiles(&self) -> Vec<Tile>;

    fn get_tile_id_by_name(&self, name: &String) -> Option<i32>;

    fn set_tile(&self, tile: &Tile);

    fn get_grids(&self) -> Vec<Grid>;

    fn get_grid_ids_by_segment_id(&self, segment_id: i64) -> Vec<i64>;

    fn get_grid_by_id(&self, grid_id: i64) -> Option<Arc<Mutex<Grid>>>;

    fn get_grid(&self, segment_id: i64, position: Vec2i) -> Option<Arc<Mutex<Grid>>>;

    fn add_grid(&self, grid_id: i64, heights: &Vec<f32>, tiles: &Vec<i32>, neighbours: &Vec<GridNeighbour>);

    fn update_grid(&self, grid_id: i64, heights: &Vec<f32>, tiles: &Vec<i32>);
}
