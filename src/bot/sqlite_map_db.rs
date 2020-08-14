use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rand::distributions::{Distribution, Uniform};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use rusqlite::{Connection, named_params, NO_PARAMS, OptionalExtension, Row, Transaction};

use crate::bot::map::{Grid, GridNeighbour, Tile};
use crate::bot::map_db::MapDb;
use crate::bot::vec2::Vec2i;

const CREATE_DB_QUERY: &'static str = r"
    BEGIN TRANSACTION;

    CREATE TABLE IF NOT EXISTS tiles (
        tile_id INTEGER PRIMARY KEY,
        version INTEGER NOT NULL,
        name TEXT NOT NULL UNIQUE,
        color INTEGER NOT NULL
    );

    CREATE UNIQUE INDEX IF NOT EXISTS uq_tiles_name
        ON tiles (name);

    CREATE TABLE IF NOT EXISTS grids (
        grid_id INTEGER PRIMARY KEY,
        revision INTEGER NOT NULL,
        segment_id INTEGER NOT NULL,
        position_x INTEGER NOT NULL,
        position_y INTEGER NOT NULL,
        heights BLOB NOT NULL,
        tiles BLOB NOT NULL
    );

    CREATE INDEX IF NOT EXISTS i_grids_coord
        ON grids (segment_id, position_x, position_y);

    CREATE INDEX IF NOT EXISTS i_grids_segment
        ON grids (segment_id);

    COMMIT;
";

const GET_TILES: &'static str = r"
    SELECT tile_id, version, name, color
      FROM tiles
     ORDER BY tile_id
";

const GET_GRIDS: &'static str = r"
    SELECT grid_id, revision, segment_id, position_x, position_y, heights, tiles
      FROM grids
     ORDER BY grid_id
";

const GET_GRID_IDS_BY_SEGMENT_ID: &'static str = r"
    SELECT grid_id
      FROM grids
     WHERE segment_id = :segment_id
";

const INSERT_TILE_QUERY: &'static str = r"
    INSERT OR IGNORE INTO tiles (tile_id, version, name, color)
    VALUES (:tile_id, :version, :name, :color)
    ON CONFLICT (tile_id) DO UPDATE SET
        version = excluded.version,
        name = excluded.name
    WHERE version < excluded.version
";

const GET_TILE_BY_NAME_QUERY: &'static str = r"
    SELECT tile_id, version, name, color
      FROM tiles
     WHERE name = :name
";

const INSERT_NEW_SEGMENT_GRID_QUERY: &'static str = r"
    INSERT INTO grids (grid_id, revision, segment_id, position_x, position_y, heights, tiles)
    VALUES (:grid_id, 1, :grid_id, 0, 0, :heights, :tiles)
";

const INSERT_EXISTING_SEGMENT_GRID_QUERY: &'static str = r"
    INSERT INTO grids (grid_id, revision, segment_id, position_x, position_y, heights, tiles)
    VALUES (:grid_id, 1, :segment_id, :position_x, :position_y, :heights, :tiles)
";

const UPDATE_GRID_QUERY: &'static str = r"
    UPDATE grids
       SET revision = revision + 1,
           heights = :heights,
           tiles = :tiles
     WHERE grid_id = :grid_id
";

const GET_GRID_BY_ID: &'static str = r"
    SELECT grid_id, revision, segment_id, position_x, position_y, heights, tiles
      FROM grids
     WHERE grid_id = :grid_id
";

const GET_GRID_REVISION_BY_ID: &'static str = r"
    SELECT revision
      FROM grids
     WHERE grid_id = :grid_id
";

const GET_GRID_BY_COORD: &'static str = r"
    SELECT grid_id, revision, segment_id, position_x, position_y, heights, tiles
      FROM grids
     WHERE segment_id = :segment_id AND position_x = :position_x AND position_y = :position_y
";

const GET_GRID_REVISION_BY_COORD: &'static str = r"
    SELECT revision
      FROM grids
     WHERE segment_id = :segment_id AND position_x = :position_x AND position_y = :position_y
";

const GET_GRID_COORD: &'static str = r"
    SELECT segment_id, position_x, position_y
      FROM grids
     WHERE grid_id = :grid_id
";

const GET_SEGMENT_SIZES: &'static str = r"
    SELECT segment_id, COUNT(1)
      FROM grids
     GROUP BY segment_id
";

const MOVE_SEGMENT_GRIDS: &'static str = r"
   UPDATE grids
      SET revision = revision + 1,
          segment_id = :dst_segment_id,
          position_x = position_x + :shift_x,
          position_y = position_y + :shift_y
    WHERE segment_id = :src_segment_id
";

pub struct SqliteMapDb {
    conn: RefCell<Connection>,
    tiles: RefCell<BTreeMap<String, CachedTile>>,
    grids_by_id: RefCell<BTreeMap<i64, CachedGrid>>,
    grids_by_coord: RefCell<BTreeMap<Coordi, CachedGrid>>,
    rng: RefCell<SmallRng>,
    cache_ttl: Option<Uniform<Duration>>,
}

impl SqliteMapDb {
    pub fn new(conn: Connection, cache_ttl: Duration) -> Self {
        conn.execute_batch(CREATE_DB_QUERY).unwrap();
        let tiles = {
            let mut stmt = conn.prepare(GET_TILES).unwrap();
            stmt.query_map(NO_PARAMS, Tile::from_sqlite_row).unwrap()
                .map(|v| v.unwrap())
                .map(|v| {
                    (
                        v.name.clone(),
                        CachedTile { cached_at: Instant::now(), value: Some(Arc::new(Mutex::new(v))) },
                    )
                })
                .collect()
        };
        let grids_by_coord: BTreeMap<Coordi, CachedGrid> = {
            let mut stmt = conn.prepare(GET_GRIDS).unwrap();
            stmt.query_map(NO_PARAMS, Grid::from_sqlite_row).unwrap()
                .map(|v| v.unwrap())
                .map(|v| {
                    (
                        Coordi { segment_id: v.segment_id, position: v.position },
                        CachedGrid { cached_at: Instant::now(), value: Some(Arc::new(Mutex::new(v))) },
                    )
                })
                .collect()
        };
        let grids_by_id = grids_by_coord.values().cloned()
            .map(|v| {
                let id = v.value.as_ref().unwrap().lock().unwrap().id;
                (id, v)
            }).collect();
        Self {
            conn: RefCell::new(conn),
            tiles: RefCell::new(tiles),
            grids_by_id: RefCell::new(grids_by_id),
            grids_by_coord: RefCell::new(grids_by_coord),
            rng: RefCell::new(SeedableRng::from_entropy()),
            cache_ttl: if cache_ttl.is_zero() {
                None
            } else {
                Some(Uniform::new(cache_ttl / 2, cache_ttl.saturating_add(cache_ttl / 2)))
            },
        }
    }

    fn get_cached_grid_by_id(&self, grid_id: i64) -> Option<Option<Arc<Mutex<Grid>>>> {
        if let Some(grid) = self.grids_by_id.borrow_mut().get_mut(&grid_id) {
            let mut rng = self.rng.borrow_mut();
            if Instant::now() - grid.cached_at < self.cache_ttl.map(|v| v.sample(rng.deref_mut())).unwrap_or(Duration::ZERO) {
                return Some(grid.value.as_ref().map(Arc::clone));
            }
            if let Some(value) = grid.value.as_ref().map(Arc::clone) {
                if let Some(revision) = get_grid_revision_by_id(self.conn.borrow().deref(), grid_id).unwrap() {
                    if value.lock().unwrap().revision == revision {
                        grid.cached_at = Instant::now();
                        return Some(Some(value));
                    }
                } else {
                    grid.cached_at = Instant::now();
                    return Some(None);
                }
            }
        }
        None
    }

    fn get_cached_grid(&self, coord: &Coordi) -> Option<Option<Arc<Mutex<Grid>>>> {
        if let Some(grid) = self.grids_by_coord.borrow_mut().get_mut(&coord) {
            let mut rng = self.rng.borrow_mut();
            if Instant::now() - grid.cached_at < self.cache_ttl.map(|v| v.sample(rng.deref_mut())).unwrap_or(Duration::ZERO) {
                return Some(grid.value.as_ref().map(Arc::clone));
            }
            if let Some(value) = grid.value.as_ref().map(Arc::clone) {
                if let Some(revision) = get_grid_revision_by_coord(self.conn.borrow().deref(), coord.segment_id, coord.position).unwrap() {
                    if value.lock().unwrap().revision == revision {
                        grid.cached_at = Instant::now();
                        return Some(Some(value));
                    }
                } else {
                    grid.cached_at = Instant::now();
                    return Some(None);
                }
            }
        }
        None
    }

    fn cache_grid(&self, grid: Arc<Mutex<Grid>>) {
        let value = grid.clone();
        let locked_grid = grid.lock().unwrap();
        let coord = Coordi { segment_id: locked_grid.segment_id, position: locked_grid.position };
        let cached_grid = CachedGrid {
            cached_at: Instant::now(),
            value: Some(value),
        };
        self.grids_by_coord.borrow_mut().insert(coord, cached_grid.clone());
        self.grids_by_id.borrow_mut().insert(locked_grid.id, cached_grid);
    }
}

impl MapDb for SqliteMapDb {
    fn get_tiles(&self) -> Vec<Tile> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(GET_TILES).unwrap();
        stmt.query_map(NO_PARAMS, |row| { Tile::from_sqlite_row(row) }).unwrap()
            .map(|v| v.unwrap())
            .collect()
    }

    fn get_tile_id_by_name(&self, name: &String) -> Option<i32> {
        if let Some(tile) = self.tiles.borrow().get(name) {
            let mut rng = self.rng.borrow_mut();
            if Instant::now() - tile.cached_at < self.cache_ttl.map(|v| v.sample(rng.deref_mut())).unwrap_or(Duration::ZERO) {
                return tile.value.as_ref().map(|v| v.lock().unwrap().id);
            }
        }
        if let Some(tile) = get_tile_by_name(self.conn.borrow().deref(), name).unwrap() {
            self.tiles.borrow_mut().insert(name.clone(), CachedTile {
                cached_at: Instant::now(),
                value: Some(Arc::new(Mutex::new(tile))),
            });
            return self.tiles.borrow().get(name)
                .and_then(|v| v.value.as_ref().map(|v| v.lock().unwrap().id));
        }
        self.tiles.borrow_mut().insert(name.clone(), CachedTile {
            cached_at: Instant::now(),
            value: None,
        });
        None
    }

    fn set_tile(&self, tile: &Tile) {
        let updated = set_tile(self.conn.borrow().deref(), tile).unwrap();
        if updated > 0 {
            self.tiles.borrow_mut().clear();
        }
    }

    fn get_grids(&self) -> Vec<Grid> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(GET_GRIDS).unwrap();
        stmt.query_map(NO_PARAMS, |row| { Grid::from_sqlite_row(row) }).unwrap()
            .map(|v| v.unwrap())
            .collect()
    }

    fn get_grid_ids_by_segment_id(&self, segment_id: i64) -> Vec<i64> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(GET_GRID_IDS_BY_SEGMENT_ID).unwrap();
        stmt.query_map_named(
            named_params! { ":segment_id": segment_id },
            |row| { row.get::<usize, i64>(0) },
        ).unwrap()
            .map(|v| v.unwrap())
            .collect()
    }

    fn get_grid_by_id(&self, grid_id: i64) -> Option<Arc<Mutex<Grid>>> {
        if let Some(grid) = self.get_cached_grid_by_id(grid_id) {
            return grid;
        }
        if let Some(grid) = get_grid_by_id(self.conn.borrow().deref(), grid_id).unwrap() {
            let grid_rc = Arc::new(Mutex::new(grid));
            self.cache_grid(Arc::clone(&grid_rc));
            return Some(grid_rc);
        }
        self.grids_by_id.borrow_mut().insert(grid_id, CachedGrid {
            cached_at: Instant::now(),
            value: None,
        });
        None
    }

    fn get_grid(&self, segment_id: i64, position: Vec2i) -> Option<Arc<Mutex<Grid>>> {
        let coord = Coordi { segment_id, position };
        if let Some(grid) = self.get_cached_grid(&coord) {
            return grid;
        }
        if let Some(grid) = get_grid_by_coord(self.conn.borrow().deref(), segment_id, position).unwrap() {
            let grid_rc = Arc::new(Mutex::new(grid));
            self.cache_grid(Arc::clone(&grid_rc));
            return Some(grid_rc);
        }
        self.grids_by_coord.borrow_mut().insert(coord, CachedGrid {
            cached_at: Instant::now(),
            value: None,
        });
        None
    }

    fn add_grid(&self, grid_id: i64, heights: &Vec<f32>, tiles: &Vec<i32>,
                neighbours: &Vec<GridNeighbour>) {
        add_grid(self.conn.borrow_mut().deref_mut(), grid_id, heights, tiles, neighbours).unwrap();
        self.grids_by_coord.borrow_mut().clear();
    }

    fn update_grid(&self, grid_id: i64, heights: &Vec<f32>, tiles: &Vec<i32>) {
        update_grid(self.conn.borrow().deref(), grid_id, heights, tiles).unwrap();
        self.grids_by_coord.borrow_mut().clear();
    }
}

fn set_tile(conn: &Connection, tile: &Tile) -> rusqlite::Result<usize> {
    conn.execute_named(
        INSERT_TILE_QUERY,
        named_params! {
            ":tile_id": tile.id,
            ":version": tile.version,
            ":name": tile.name,
            ":color": tile.color,
        },
    )
}

fn get_tile_by_name(conn: &Connection, name: &String) -> rusqlite::Result<Option<Tile>> {
    conn.query_row_named(
        GET_TILE_BY_NAME_QUERY,
        named_params! { ":name": name },
        Tile::from_sqlite_row,
    ).optional()
}

fn get_grid_by_id(conn: &Connection, grid_id: i64) -> rusqlite::Result<Option<Grid>> {
    conn.query_row_named(
        GET_GRID_BY_ID,
        named_params! { ":grid_id": grid_id },
        Grid::from_sqlite_row,
    ).optional()
}

fn get_grid_revision_by_id(conn: &Connection, grid_id: i64) -> rusqlite::Result<Option<i64>> {
    conn.query_row_named(
        GET_GRID_REVISION_BY_ID,
        named_params! { ":grid_id": grid_id },
        |v| v.get::<usize, i64>(0),
    ).optional()
}

fn get_grid_by_coord(conn: &Connection, segment_id: i64,
                     position: Vec2i) -> rusqlite::Result<Option<Grid>> {
    conn.query_row_named(
        GET_GRID_BY_COORD,
        named_params! {
                ":segment_id": segment_id,
                ":position_x": position.x(),
                ":position_y": position.y(),
            },
        Grid::from_sqlite_row,
    ).optional()
}

fn get_grid_revision_by_coord(conn: &Connection, segment_id: i64,
                              position: Vec2i) -> rusqlite::Result<Option<i64>> {
    conn.query_row_named(
        GET_GRID_REVISION_BY_COORD,
        named_params! {
                ":segment_id": segment_id,
                ":position_x": position.x(),
                ":position_y": position.y(),
            },
        |v| v.get::<usize, i64>(0),
    ).optional()
}

fn add_grid(conn: &mut Connection, grid_id: i64, heights: &Vec<f32>, tiles: &Vec<i32>,
            neighbours: &Vec<GridNeighbour>) -> rusqlite::Result<()> {
    let tx: Transaction = conn.transaction()?;
    if let Some(_) = get_grid_coord(tx.deref(), grid_id).unwrap() {
        update_grid(tx.deref(), grid_id, heights, tiles)?;
        return tx.commit();
    }
    let mut segments = get_segments(tx.deref(), neighbours)?;
    if !segments.is_empty() {
        segments.sort_by_key(|v| v.segment_id);
        segments.dedup_by_key(|v| v.segment_id);
        let GridSegment {
            segment_id: target_segment,
            offset: target_offset,
            position: target_position,
        } = segments[0];
        if segments.len() > 1 {
            let segments_sizes = get_segment_sizes(tx.deref())?;
            segments.sort_by_key(|v| {
                (-segments_sizes[&v.segment_id], v.segment_id)
            });
            for i in 1..segments.len() {
                let GridSegment { segment_id, offset, position } = segments[i];
                let shift = target_position - target_offset + offset - position;
                move_segment_grids(tx.deref(), segment_id, target_segment, shift)?;
            }
        }
        let position = target_position - target_offset;
        tx.execute_named(
            INSERT_EXISTING_SEGMENT_GRID_QUERY,
            named_params! {
                ":grid_id": grid_id,
                ":segment_id": target_segment,
                ":position_x": position.x(),
                ":position_y": position.y(),
                ":heights": serde_json::to_vec(heights).unwrap(),
                ":tiles": serde_json::to_vec(tiles).unwrap(),
            },
        )?;
    } else {
        tx.execute_named(
            INSERT_NEW_SEGMENT_GRID_QUERY,
            named_params! {
                ":grid_id": grid_id,
                ":heights": serde_json::to_vec(heights).unwrap(),
                ":tiles": serde_json::to_vec(tiles).unwrap(),
            },
        )?;
    }
    tx.commit()
}

fn update_grid(conn: &Connection, grid_id: i64, heights: &Vec<f32>,
               tiles: &Vec<i32>) -> rusqlite::Result<usize> {
    conn.execute_named(
        UPDATE_GRID_QUERY,
        named_params! {
                ":grid_id": grid_id,
                ":heights": serde_json::to_vec(heights).unwrap(),
                ":tiles": serde_json::to_vec(tiles).unwrap(),
            },
    )
}

fn get_segments(conn: &Connection, neighbours: &Vec<GridNeighbour>) -> rusqlite::Result<Vec<GridSegment>> {
    let mut result = Vec::new();
    for neighbour in neighbours.iter() {
        if let Some(Coordi { segment_id, position }) = get_grid_coord(conn, neighbour.id)? {
            result.push(GridSegment { segment_id, offset: neighbour.offset, position });
        }
    }
    Ok(result)
}

struct GridSegment {
    segment_id: i64,
    offset: Vec2i,
    position: Vec2i,
}

fn get_grid_coord(conn: &Connection, grid_id: i64) -> rusqlite::Result<Option<Coordi>> {
    conn.query_row_named(
        GET_GRID_COORD,
        named_params! {":grid_id": grid_id},
        |row| {
            Ok(Coordi {
                segment_id: row.get::<usize, i64>(0)?,
                position: Vec2i::new(row.get(1)?, row.get(2)?),
            })
        },
    ).optional()
}

fn get_segment_sizes(conn: &Connection) -> rusqlite::Result<HashMap<i64, i64>> {
    let mut stmt = conn.prepare(GET_SEGMENT_SIZES)?;
    let iter = stmt.query_map(
        NO_PARAMS,
        |row| Ok((row.get::<usize, i64>(0)?, row.get::<usize, i64>(1)?)),
    )?;
    let mut result = HashMap::new();
    for value in iter {
        let (segment_id, size) = value?;
        result.insert(segment_id, size);
    }
    Ok(result)
}

fn move_segment_grids(conn: &Connection, src_segment_id: i64, dst_segment_id: i64,
                      shift: Vec2i) -> rusqlite::Result<usize> {
    conn.execute_named(
        MOVE_SEGMENT_GRIDS,
        named_params! {
            ":src_segment_id": src_segment_id,
            ":dst_segment_id": dst_segment_id,
            ":shift_x": shift.x(),
            ":shift_y": shift.y(),
        },
    )
}

#[derive(Debug)]
struct CachedTile {
    cached_at: Instant,
    value: Option<Arc<Mutex<Tile>>>,
}

impl Tile {
    fn from_sqlite_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Tile {
            id: row.get(0)?,
            version: row.get(1)?,
            name: row.get(2)?,
            color: row.get(3)?,
        })
    }
}

#[derive(Clone, Debug)]
struct CachedGrid {
    cached_at: Instant,
    value: Option<Arc<Mutex<Grid>>>,
}

impl Grid {
    fn from_sqlite_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Grid {
            id: row.get(0)?,
            revision: row.get(1)?,
            segment_id: row.get(2)?,
            position: Vec2i::new(row.get(3)?, row.get(4)?),
            heights: serde_json::from_slice(&(row.get::<usize, Vec<u8>>(5)?)).unwrap(),
            tiles: serde_json::from_slice(&(row.get::<usize, Vec<u8>>(6)?)).unwrap(),
        })
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Coordi {
    segment_id: i64,
    position: Vec2i,
}

#[cfg(test)]
mod tests {
    use std::fs::remove_file;
    use std::path::Path;

    use super::*;

    #[test]
    fn add_grid_should_store_grid() {
        let path = RemovePath("add_single_grid_map.db");
        let map_db = make_map_db(&path);
        let grid_id = 1;
        let heights = vec![1.0, 2.0, 3.0];
        let tiles = vec![4, 5, 6];
        let neighbours = Vec::new();
        map_db.add_grid(grid_id, &heights, &tiles, &neighbours);
        assert_eq!(map_db.get_grids(), vec![
            Grid {
                id: grid_id,
                revision: 1,
                segment_id: grid_id,
                position: Vec2i::zero(),
                heights,
                tiles,
            }
        ]);
    }

    #[test]
    fn adjacent_grids_should_be_stored_in_a_single_segment() {
        let path = RemovePath("adjacent_grids_should_be_stored_in_a_single_segment.db");
        let map_db = make_map_db(&path);
        map_db.add_grid(1, &Vec::new(), &Vec::new(), &Vec::new());
        map_db.add_grid(2, &Vec::new(), &Vec::new(), &vec![
            GridNeighbour { id: 1, offset: Vec2i::new(1, 0) },
        ]);
        assert_eq!(
            map_db.get_grids().iter().map(|v| (v.id, v.segment_id, v.position)).collect::<Vec<_>>(),
            vec![
                (1, 1, Vec2i::zero()),
                (2, 1, Vec2i::new(-1, 0)),
            ]
        );
    }

    #[test]
    fn separate_grids_should_be_stored_in_different_segments() {
        let path = RemovePath("separate_grids_should_be_stored_in_different_segments.db");
        let map_db = make_map_db(&path);
        map_db.add_grid(1, &Vec::new(), &Vec::new(), &Vec::new());
        map_db.add_grid(2, &Vec::new(), &Vec::new(), &Vec::new());
        assert_eq!(
            map_db.get_grids().iter().map(|v| (v.id, v.segment_id, v.position)).collect::<Vec<_>>(),
            vec![
                (1, 1, Vec2i::zero()),
                (2, 2, Vec2i::zero()),
            ]
        );
    }

    #[test]
    fn adjacent_grid_to_separated_segments_should_merge_them() {
        let path = RemovePath("adjacent_grid_to_separated_segments_should_merge_them.db");
        let map_db = make_map_db(&path);
        map_db.add_grid(1, &Vec::new(), &Vec::new(), &Vec::new());
        map_db.add_grid(2, &Vec::new(), &Vec::new(), &Vec::new());
        map_db.add_grid(3, &Vec::new(), &Vec::new(), &vec![
            GridNeighbour { id: 1, offset: Vec2i::new(-1, -1) },
            GridNeighbour { id: 2, offset: Vec2i::new(0, 1) },
        ]);
        assert_eq!(
            map_db.get_grids().iter().map(|v| (v.id, v.segment_id, v.position)).collect::<Vec<_>>(),
            vec![
                (1, 1, Vec2i::zero()),
                (2, 1, Vec2i::new(1, 2)),
                (3, 1, Vec2i::new(1, 1)),
            ]
        );
    }

    #[test]
    fn update_grid_should_invalidate_cache() {
        let path = RemovePath("update_grid_should_invalidate_cache.db");
        let map_db = make_map_db(&path);
        let grid_id = 1;
        let mut heights = vec![1.0, 2.0, 3.0];
        let tiles = vec![4, 5, 6];
        let neighbours = Vec::new();
        map_db.add_grid(grid_id, &heights, &tiles, &neighbours);
        assert_eq!(
            map_db.get_grid(1, Vec2i::zero()).map(|v| v.lock().unwrap().clone()),
            Some(Grid {
                id: grid_id,
                revision: 1,
                segment_id: grid_id,
                position: Vec2i::zero(),
                heights: heights.clone(),
                tiles: tiles.clone(),
            })
        );
        heights.push(7.0);
        map_db.update_grid(grid_id, &heights, &tiles);
        assert_eq!(
            map_db.get_grid(1, Vec2i::zero()).map(|v| v.lock().unwrap().clone()),
            Some(Grid {
                id: grid_id,
                revision: 2,
                segment_id: grid_id,
                position: Vec2i::zero(),
                heights,
                tiles,
            })
        );
    }

    #[test]
    fn get_grid_should_invalidate_cache_by_ttl() {
        let path = RemovePath("get_grid_should_invalidate_cache_by_ttl.db");
        let map_db = make_map_db_with_cache_ttl(&path, Duration::new(0, 0));
        let grid_id = 1;
        let mut heights = vec![1.0, 2.0, 3.0];
        let tiles = vec![4, 5, 6];
        let neighbours = Vec::new();
        map_db.add_grid(grid_id, &heights, &tiles, &neighbours);
        assert_eq!(
            map_db.get_grid(1, Vec2i::zero())
                .map(|v| v.lock().unwrap().clone()),
            Some(Grid {
                id: grid_id,
                revision: 1,
                segment_id: grid_id,
                position: Vec2i::zero(),
                heights: heights.clone(),
                tiles: tiles.clone(),
            })
        );
        heights.push(7.0);
        let conn = Connection::open(&path).unwrap();
        assert_eq!(update_grid(&conn, grid_id, &heights, &tiles), Ok(1));
        assert_eq!(
            map_db.get_grid(1, Vec2i::zero())
                .map(|v| v.lock().unwrap().clone()),
            Some(Grid {
                id: grid_id,
                revision: 2,
                segment_id: grid_id,
                position: Vec2i::zero(),
                heights,
                tiles,
            })
        );
    }

    #[test]
    fn get_grid_should_get_cached_before_ttl_ends() {
        let path = RemovePath("get_grid_should_get_cached_before_ttl_ends.db");
        let map_db = make_map_db(&path);
        let grid_id = 1;
        let mut heights = vec![1.0, 2.0, 3.0];
        let tiles = vec![4, 5, 6];
        let neighbours = Vec::new();
        map_db.add_grid(grid_id, &heights, &tiles, &neighbours);
        assert_eq!(
            map_db.get_grid(1, Vec2i::zero())
                .map(|v| v.lock().unwrap().clone()),
            Some(Grid {
                id: grid_id,
                revision: 1,
                segment_id: grid_id,
                position: Vec2i::zero(),
                heights: heights.clone(),
                tiles: tiles.clone(),
            })
        );
        heights.push(7.0);
        let conn = Connection::open(&path).unwrap();
        assert_eq!(update_grid(&conn, grid_id, &heights, &tiles), Ok(1));
        assert_eq!(
            map_db.get_grid(1, Vec2i::zero())
                .map(|v| v.lock().unwrap().clone()),
            Some(Grid {
                id: grid_id,
                revision: 1,
                segment_id: grid_id,
                position: Vec2i::zero(),
                heights: vec![1.0, 2.0, 3.0],
                tiles,
            })
        );
    }

    #[test]
    fn set_tile_should_store_tile() {
        let path = RemovePath("set_tile_should_store_tile.db");
        let map_db = make_map_db(&path);
        let tile = Tile { id: 1, version: 1, name: String::from("ground"), color: 0xFFFFFF };
        map_db.set_tile(&tile);
        assert_eq!(map_db.get_tiles(), vec![tile]);
    }

    #[test]
    fn set_tile_should_update_stored_tile_with_greater_version() {
        let path = RemovePath("set_tile_should_update_stored_tile_with_greater_version.db");
        let map_db = make_map_db(&path);
        let mut tile = Tile { id: 1, version: 1, name: String::from("ground"), color: 0xFFFFFF };
        map_db.set_tile(&tile);
        tile.version = 2;
        map_db.set_tile(&tile);
        assert_eq!(map_db.get_tiles(), vec![tile]);
    }

    #[test]
    fn set_tile_should_not_update_stored_tile_with_less_version() {
        let path = RemovePath("set_tile_should_not_update_stored_tile_with_less_version.db");
        let map_db = make_map_db(&path);
        let tile = Tile { id: 1, version: 2, name: String::from("ground"), color: 0xFFFFFF };
        map_db.set_tile(&tile);
        let mut updated_tile = tile.clone();
        updated_tile.version = 1;
        map_db.set_tile(&tile);
        assert_eq!(map_db.get_tiles(), vec![tile]);
    }

    #[test]
    fn set_tile_should_invalidate_cache() {
        let path = RemovePath("set_tile_should_invalidate_cache.db");
        let map_db = make_map_db(&path);
        let mut tile = Tile { id: 1, version: 1, name: String::from("ground"), color: 0xFFFFFF };
        map_db.set_tile(&tile);
        assert_eq!(map_db.get_tile_id_by_name(&String::from("ground")), Some(tile.id));
        tile.version = 2;
        tile.name = String::from("water");
        map_db.set_tile(&tile);
        assert_eq!(map_db.get_tile_id_by_name(&String::from("ground")), None);
        assert_eq!(map_db.get_tile_id_by_name(&String::from("water")), Some(tile.id));
    }

    #[test]
    fn get_tile_should_invalidate_cache_by_ttl() {
        let path = RemovePath("get_tile_should_invalidate_cache_by_ttl.db");
        let map_db = make_map_db_with_cache_ttl(&path, Duration::new(0, 0));
        let mut tile = Tile { id: 1, version: 1, name: String::from("ground"), color: 0xFFFFFF };
        map_db.set_tile(&tile);
        assert_eq!(map_db.get_tile_id_by_name(&String::from("ground")), Some(tile.id));
        tile.version = 2;
        tile.name = String::from("water");
        let conn = Connection::open(&path).unwrap();
        assert_eq!(set_tile(&conn, &tile), Ok(1));
        assert_eq!(map_db.get_tile_id_by_name(&String::from("ground")), None);
        assert_eq!(map_db.get_tile_id_by_name(&String::from("water")), Some(tile.id));
    }

    #[test]
    fn get_tile_should_not_invalidate_cache_before_ttl_ends() {
        let path = RemovePath("get_tile_should_not_invalidate_cache_before_ttl_ends.db");
        let map_db = make_map_db(&path);
        let tile = Tile { id: 1, version: 1, name: String::from("ground"), color: 0xFFFFFF };
        map_db.set_tile(&tile);
        assert_eq!(map_db.get_tile_id_by_name(&String::from("ground")), Some(tile.id));
        let mut updated_tile = tile.clone();
        updated_tile.version = 2;
        updated_tile.name = String::from("water");
        let conn = Connection::open(&path).unwrap();
        assert_eq!(set_tile(&conn, &updated_tile), Ok(1));
        assert_eq!(map_db.get_tile_id_by_name(&String::from("ground")), Some(tile.id));
    }

    fn make_map_db<P: AsRef<Path> + Copy>(path: P) -> SqliteMapDb {
        make_map_db_with_cache_ttl(path, Duration::new(std::u64::MAX, 0))
    }

    fn make_map_db_with_cache_ttl<P: AsRef<Path> + Copy>(path: P, cache_ttl: Duration) -> SqliteMapDb {
        match remove_file(path) { _ => () };
        let conn = Connection::open(path).unwrap();
        SqliteMapDb::new(conn, cache_ttl)
    }

    #[derive(Clone)]
    struct RemovePath<'a>(&'a str);

    impl<'a> AsRef<Path> for RemovePath<'a> {
        fn as_ref(&self) -> &Path {
            self.0.as_ref()
        }
    }

    impl<'a> Drop for RemovePath<'a> {
        fn drop(&mut self) {
            match remove_file(self.0) { _ => () }
        }
    }
}
