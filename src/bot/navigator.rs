use std::iter::repeat;

use crate::bot::map::{Grid, GRID_SIZE, tile_index_to_tile_pos, tile_pos_to_tile_index};
use crate::bot::vec2::Vec2i;

#[derive(Debug, PartialEq)]
pub struct Area {
    pub tile: i32,
    pub border: Vec<Vec2i>,
}

pub fn make_areas(grid: &Grid) -> Vec<Area> {
    let mut areas = divide_into_areas(&grid);
    divide_loop_areas(&mut areas, &grid);

    (1..areas.number).into_iter()
        .map(|area| {
            let position = areas.iter()
                .enumerate()
                .find(|(_, v)| **v == area)
                .map(|(i, _)| tile_index_to_tile_pos(i))
                .unwrap();
            Area {
                tile: grid.get_tile(position),
                border: remove_redundant_border_positions(
                    smooth_area_border(
                        make_area_contour(
                            remove_redundant_border_positions(
                                make_area_border(position, area, &areas)
                            )
                        )
                    )
                ),
            }
        })
        .collect()
}

#[derive(PartialOrd, PartialEq, Debug)]
struct Areas {
    number: u16,
    size: usize,
    values: Vec<u16>,
}

impl Areas {
    fn get(&self, index: usize) -> u16 {
        self.values[index]
    }

    fn get_by_pos(&self, position: Vec2i) -> u16 {
        self.values[pos_to_index(position, self.size)]
    }

    fn contains(&self, position: Vec2i) -> bool {
        0 <= position.x() && position.x() < self.size as i32
            && 0 <= position.y() && position.y() < self.size as i32
    }

    fn set(&mut self, index: usize, value: u16) {
        self.values[index] = value;
    }

    fn copy(&mut self, src: usize, dst: usize) {
        self.values[dst] = self.values[src];
    }

    fn iter(&self) -> impl Iterator<Item=&u16> {
        self.values.iter()
    }
}

fn pos_to_index(position: Vec2i, size: usize) -> usize {
    position.x() as usize + position.y() as usize * size as usize
}

fn divide_into_areas(grid: &Grid) -> Areas {
    let mut values: Vec<u16> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
    let mut next_area_number = 2;

    values[0] = 1;

    for i in 1..(GRID_SIZE * GRID_SIZE) as usize {
        let tile = grid.tiles[i];
        if i % GRID_SIZE as usize > 0 && grid.tiles[i - 1] == tile {
            values[i] = values[i - 1];
        } else if i >= GRID_SIZE as usize && grid.tiles[i - GRID_SIZE as usize] == tile {
            values[i] = values[i - GRID_SIZE as usize];
        } else if i >= GRID_SIZE as usize && i % (GRID_SIZE as usize) < GRID_SIZE as usize - 1
            && grid.tiles[i + 1] == tile && grid.tiles[i + 1 - GRID_SIZE as usize] == tile {
            values[i] = values[i + 1 - GRID_SIZE as usize];
        } else {
            values[i] = next_area_number;
            next_area_number += 1;
        }
    }

    Areas { number: next_area_number, size: GRID_SIZE as usize, values }
}

fn divide_loop_areas(areas: &mut Areas, grid: &Grid) {
    for i in GRID_SIZE as usize..(GRID_SIZE * GRID_SIZE) as usize {
        if i % GRID_SIZE as usize > 0 && i >= GRID_SIZE as usize && i % (GRID_SIZE as usize) < GRID_SIZE as usize - 1
            && areas.get(i) == areas.get(i - 1) && areas.get(i) < areas.get(i - GRID_SIZE as usize) && grid.tiles[i] != grid.tiles[i - GRID_SIZE as usize] {
            areas.set(i, areas.number);
            areas.number += 1;
        } else if i % GRID_SIZE as usize > 0 && areas.get(i) < areas.get(i - 1) && grid.tiles[i] == grid.tiles[i - 1] {
            areas.copy(i - 1, i);
        } else if i >= GRID_SIZE as usize && areas.get(i) < areas.get(i - GRID_SIZE as usize) && grid.tiles[i] == grid.tiles[i - GRID_SIZE as usize] {
            areas.copy(i - GRID_SIZE as usize, i);
        }
    }
}

fn make_area_border(initial_position: Vec2i, area: u16, areas: &Areas) -> Vec<Vec2i> {
    let mut border = Vec::new();
    let mut shifts = get_shifts(Vec2i::only_x(1));
    let mut position = initial_position;

    loop {
        border.push(position);
        let mut next_shift = None;
        for i in 0..shifts.len() {
            let next_position = position + shifts[i];
            if next_position == initial_position {
                return border;
            }
            if !areas.contains(next_position) {
                continue;
            }
            let current_area = areas.get_by_pos(next_position);
            if current_area == area {
                next_shift = Some(shifts[i]);
                position = next_position;
                break;
            }
        }
        if let Some(shift) = next_shift {
            shifts = get_shifts(shift);
        } else {
            return border;
        }
    }
}

fn make_area_contour(border: Vec<Vec2i>) -> Vec<Vec2i> {
    if border.is_empty() {
        return border;
    }

    if border.len() == 1 {
        return vec![
            border[0],
            border[0] + Vec2i::only_x(1),
            border[0] + Vec2i::new(1, 1),
            border[0] + Vec2i::only_y(1),
        ];
    }

    let mut result = vec![border[0]];
    let mut i = 1;

    while i < border.len() {
        let prev = border[i - 1];
        let current = border[i];
        let from_prev = (current - prev).signum();
        let to_next = (border[(i + 1) % border.len()] - current).signum();
        let last = result[result.len() - 1];
        if border[i].x() == last.x() && border[i].y() > last.y() {
            result.push(last + Vec2i::only_x(1));
        }
        if to_next + from_prev == Vec2i::zero() {
            if from_prev.x() > 0 {
                result.push(current + Vec2i::only_x(1));
                result.push(current + Vec2i::new(1, 1));
            } else if from_prev.x() < 0 {
                result.push(current + Vec2i::only_y(1));
                result.push(current);
            } else if from_prev.y() > 0 {
                result.push(current + Vec2i::new(1, 1));
                result.push(current + Vec2i::only_y(1));
            } else if from_prev.y() < 0 {
                result.push(current);
                result.push(current + Vec2i::only_x(1));
            }
        } else {
            result.push(Vec2i::new(
                current.x() + (from_prev.y() > 0 || to_next.y() > 0) as i32,
                current.y() + (from_prev.x() < 0 || to_next.x() < 0) as i32
            ));
        }
        i += 1;
    }

    let to_end = result[0] - result[result.len() - 1];
    if to_end.x() != 0 && to_end.y() != 0 {
        if to_end.x().abs() > to_end.y().abs() {
            result.push(result[result.len() - 1] + Vec2i::only_x(to_end.x()));
        } else {
            result.push(result[result.len() - 1] + Vec2i::only_y(to_end.y()));
        }
    }

    result
}

pub fn smooth_area_border(border: Vec<Vec2i>) -> Vec<Vec2i> {
    if border.len() <= 2 {
        return border;
    }

    let mut result = Vec::new();
    let mut i = 1;

    while i < border.len() - 1 {
        if (border[i - 1] - border[i]).manhattan() != 1 {
            result.push(border[i - 1]);
            i += 1;
            continue;
        }
        if (border[i] - border[i + 1]).manhattan() != 1 {
            result.push(border[i - 1]);
            result.push(border[i]);
            i += 2;
            continue;
        }
        result.push(border[i - 1]);
        i += 2;
    }

    for j in i - 1..border.len() {
        result.push(border[j]);
    }

    result
}

fn remove_redundant_border_positions(border: Vec<Vec2i>) -> Vec<Vec2i> {
    if border.len() <= 2 {
        return border;
    }

    let mut result = vec![border[0]];

    for i in 1..border.len() {
        let from_prev = (border[i] - border[i - 1]).signum();
        let to_next = (border[(i + 1) % border.len()] - border[i]).signum();
        if from_prev != to_next {
            result.push(border[i]);
        }
    }

    result
}

const SHIFTS: [&'static [Vec2i]; 5] = [
    &[
        Vec2i::only_y(-1),
        Vec2i::only_x(1),
        Vec2i::only_y(1),
        Vec2i::only_x(-1),
    ],
    &[
        Vec2i::only_x(1),
        Vec2i::only_y(1),
        Vec2i::only_x(-1),
        Vec2i::only_y(-1),
    ],
    &[
        Vec2i::only_y(1),
        Vec2i::only_x(-1),
        Vec2i::only_y(-1),
        Vec2i::only_x(1),
    ],
    &[
        Vec2i::only_x(-1),
        Vec2i::only_y(-1),
        Vec2i::only_x(1),
        Vec2i::only_y(1),
    ],
    &[],
];

fn get_shifts(shift: Vec2i) -> &'static [Vec2i] {
    if shift == Vec2i::only_x(1) {
        SHIFTS[0]
    } else if shift == Vec2i::only_y(1) {
        SHIFTS[1]
    } else if shift == Vec2i::only_x(-1) {
        SHIFTS[2]
    } else if shift == Vec2i::only_y(-1) {
        SHIFTS[3]
    } else {
        SHIFTS[4]
    }
}

fn get_border_position(position: Vec2i, shift: Vec2i, previous: Vec2i) -> Vec2i {
    if shift.x() == 0 {
        let y = if shift.y() < 0 {
            position.y()
        } else {
            position.y() + shift.y()
        };
        Vec2i::new(previous.x(), y)
    } else {
        let x = if shift.x() < 0 {
            position.x()
        } else {
            position.x() + shift.x()
        };
        Vec2i::new(x, previous.y())
    }
}

fn print_area_border(areas: &Areas, border: &Vec<Vec2i>) {
    let mut result: Vec<u8> = repeat('.' as u8).take(areas.size * areas.size).collect();
    if !border.is_empty() {
        result[pos_to_index(border[0], areas.size)] = '^' as u8;
        for i in 1..border.len() - 1 {
            result[pos_to_index(border[i], areas.size)] = '*' as u8;
        }
        if border.len() > 1 {
            result[pos_to_index(border[border.len() - 1], areas.size)] = '$' as u8;
        }
    }
    for y in 0..areas.size {
        for x in 0..areas.size {
            print!("{} ", result[pos_to_index(Vec2i::new(x as i32, y as i32), areas.size)] as char);
        }
        println!();
    }
    println!();
}

fn print_areas(areas: &Areas) {
    for y in 0..areas.size {
        for x in 0..areas.size {
            print!("{} ", areas.get_by_pos(Vec2i::new(x as i32, y as i32)));
        }
        println!();
    }
    println!();
}

fn print_areas_diff(lhs: &Vec<u16>, rhs: &Vec<u16>) {
    for y in 0..GRID_SIZE {
        for x in 0..GRID_SIZE {
            let l = lhs[tile_pos_to_tile_index(Vec2i::new(x, y))];
            let r = rhs[tile_pos_to_tile_index(Vec2i::new(x, y))];
            if l == r {
                print!("* ");
            } else {
                print!("{} ", l);
            }
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_areas_should_return_borders_for_each_area() {
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles: repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        assert_eq!(make_areas(&grid), vec![
            Area {
                tile: 0,
                border: vec![
                    Vec2i::new(0, 0), Vec2i::new(100, 0),
                    Vec2i::new(100, 100), Vec2i::new(0, 100),
                ],
            },
        ]);
    }

    #[test]
    fn make_areas_should_divide_different_tiles_into_areas() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in GRID_SIZE / 2..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        assert_eq!(make_areas(&grid), vec![
            Area {
                tile: 0,
                border: vec![
                    Vec2i::new(0, 0), Vec2i::new(50, 0),
                    Vec2i::new(50, 100), Vec2i::new(0, 100),
                ],
            },
            Area {
                tile: 1,
                border: vec![
                    Vec2i::new(50, 0), Vec2i::new(100, 0),
                    Vec2i::new(100, 100), Vec2i::new(50, 100),
                ],
            },
        ]);
    }

    #[test]
    fn make_areas_should_support_nested_areas() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for x in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
            for y in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        assert_eq!(make_areas(&grid), vec![
            Area {
                tile: 0,
                border: vec![
                    Vec2i::new(0, 0), Vec2i::new(100, 0),
                    Vec2i::new(100, 66), Vec2i::new(66, 66),
                    Vec2i::new(66, 33), Vec2i::new(33, 33),
                    Vec2i::new(33, 100), Vec2i::new(0, 100),
                ],
            },
            Area {
                tile: 1,
                border: vec![
                    Vec2i::new(33, 33), Vec2i::new(66, 33),
                    Vec2i::new(66, 66), Vec2i::new(33, 66),
                ],
            },
            Area {
                tile: 0,
                border: vec![
                    Vec2i::new(33, 66), Vec2i::new(100, 66),
                    Vec2i::new(100, 100), Vec2i::new(33, 100),
                ],
            },
        ]);
    }

    #[test]
    fn make_areas_should_support_diagonal_left_to_right_border() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in y..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        assert_eq!(make_areas(&grid), vec![
            Area {
                tile: 1,
                border: vec![
                    Vec2i::new(0, 0), Vec2i::new(100, 0), Vec2i::new(100, 100),
                    Vec2i::new(1, 1), Vec2i::new(1, 0),
                ],
            },
            Area {
                tile: 0,
                border: vec![
                    Vec2i::new(0, 1), Vec2i::new(99, 100), Vec2i::new(0, 100)
                ],
            },
        ]);
    }

    #[test]
    fn make_areas_should_support_diagonal_right_to_left_border() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in GRID_SIZE - y..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        assert_eq!(make_areas(&grid), vec![
            Area {
                tile: 0,
                border: vec![
                    Vec2i::new(0, 0), Vec2i::new(100, 0), Vec2i::new(0, 100),
                ],
            },
            Area {
                tile: 1,
                border: vec![
                    Vec2i::new(99, 1), Vec2i::new(100, 1), Vec2i::new(100, 100),
                    Vec2i::new(1, 100), Vec2i::new(99, 2),
                ],
            },
        ]);
    }

    #[test]
    fn divide_into_areas_should_fill_all_grid_with_only_one_tile() {
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles: repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        let areas = divide_into_areas(&grid);
        assert_eq!(
            areas,
            Areas {
                number: 2,
                size: GRID_SIZE as usize,
                values: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            },
        );
    }

    #[test]
    fn divide_into_areas_should_separate_different_tiles_into_different_areas() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in GRID_SIZE / 2..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 1,
                1 => 2,
                _ => 0,
            })
            .collect();
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let areas = divide_into_areas(&grid);
        assert_eq!(areas, Areas { number: 3, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_into_areas_should_put_horizontally_separated_same_tile_into_different_areas() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let mut expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 1,
                1 => 2,
                _ => 0,
            })
            .collect();
        for y in 0..GRID_SIZE {
            for x in 2 * GRID_SIZE / 3..GRID_SIZE {
                expected_areas[tile_pos_to_tile_index(Vec2i::new(x, y))] = 3;
            }
        }
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let areas = divide_into_areas(&grid);
        assert_eq!(areas, Areas { number: 4, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_into_areas_should_put_vertically_separated_same_tile_into_different_areas() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
            for x in 0..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let mut expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 1,
                1 => 2,
                _ => 0,
            })
            .collect();
        for y in 2 * GRID_SIZE / 3..GRID_SIZE {
            for x in 0..GRID_SIZE {
                expected_areas[tile_pos_to_tile_index(Vec2i::new(x, y))] = 3;
            }
        }
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let areas = divide_into_areas(&grid);
        assert_eq!(areas, Areas { number: 4, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_into_areas_should_support_nested_areas() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for x in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
            for y in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 1,
                1 => 2,
                _ => 0,
            })
            .collect();
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let areas = divide_into_areas(&grid);
        assert_eq!(areas, Areas { number: 3, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_into_areas_should_support_diagonal_left_to_right_border() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in y..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 2,
                1 => 1,
                _ => 0,
            })
            .collect();
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let areas = divide_into_areas(&grid);
        assert_eq!(areas, Areas { number: 3, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_into_areas_should_support_diagonal_right_to_left_border() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in GRID_SIZE - y..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 1,
                1 => 2,
                _ => 0,
            })
            .collect();
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let areas = divide_into_areas(&grid);
        assert_eq!(areas, Areas { number: 3, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_into_areas_should_support_single_different_tile() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        tiles[0] = 1;
        let expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 2,
                1 => 1,
                _ => 0,
            })
            .collect();
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let areas = divide_into_areas(&grid);
        assert_eq!(areas, Areas { number: 3, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_loop_areas_should_support_nested_areas() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for x in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
            for y in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let mut expected_areas: Vec<u16> = repeat(1).take(tiles.len()).collect();
        for y in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
            for x in GRID_SIZE / 3..2 * GRID_SIZE / 3 {
                expected_areas[tile_pos_to_tile_index(Vec2i::new(x, y))] = 2;
            }
        }
        for y in 2 * GRID_SIZE / 3..GRID_SIZE {
            for x in GRID_SIZE / 3..GRID_SIZE {
                expected_areas[tile_pos_to_tile_index(Vec2i::new(x, y))] = 3;
            }
        }
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let mut areas = divide_into_areas(&grid);
        divide_loop_areas(&mut areas, &grid);
        assert_eq!(areas, Areas { number: 4, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_loop_areas_should_support_diagonal_left_to_right_border() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in y..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 2,
                1 => 1,
                _ => 0,
            })
            .collect();
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let mut areas = divide_into_areas(&grid);
        divide_loop_areas(&mut areas, &grid);
        assert_eq!(areas, Areas { number: 3, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_loop_areas_should_support_diagonal_right_to_left_border() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE {
            for x in GRID_SIZE - y..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 1,
                1 => 2,
                _ => 0,
            })
            .collect();
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let mut areas = divide_into_areas(&grid);
        divide_loop_areas(&mut areas, &grid);
        assert_eq!(areas, Areas { number: 3, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_loop_areas_should_support_single_different_tile() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        tiles[0] = 1;
        let expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 2,
                1 => 1,
                _ => 0,
            })
            .collect();
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let mut areas = divide_into_areas(&grid);
        divide_loop_areas(&mut areas, &grid);
        assert_eq!(areas, Areas { number: 3, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn divide_loop_areas_should_support_spiral() {
        let mut tiles: Vec<i32> = repeat(0).take((GRID_SIZE * GRID_SIZE) as usize).collect();
        for y in 0..GRID_SIZE / 3 {
            for x in 0..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        for y in GRID_SIZE / 3..GRID_SIZE {
            for x in 3 * GRID_SIZE / 4..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        for y in 4 * GRID_SIZE / 5..GRID_SIZE {
            for x in 0..GRID_SIZE {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        for y in 3 * GRID_SIZE / 7..4 * GRID_SIZE / 5 {
            for x in 0..GRID_SIZE / 6 {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        for y in 3 * GRID_SIZE / 7..4 * GRID_SIZE / 7 {
            for x in 0..5 * GRID_SIZE / 8 {
                tiles[tile_pos_to_tile_index(Vec2i::new(x, y))] = 1;
            }
        }
        let mut expected_areas: Vec<u16> = tiles.iter()
            .map(|v| match *v as u16 {
                0 => 2,
                1 => 1,
                _ => 0,
            })
            .collect();
        for y in 4 * GRID_SIZE / 5..GRID_SIZE {
            for x in GRID_SIZE / 6..GRID_SIZE {
                expected_areas[tile_pos_to_tile_index(Vec2i::new(x, y))] = 5;
            }
        }
        for y in 3 * GRID_SIZE / 7..GRID_SIZE {
            for x in 0..GRID_SIZE / 6 {
                expected_areas[tile_pos_to_tile_index(Vec2i::new(x, y))] = 3;
            }
        }
        for y in 3 * GRID_SIZE / 7..4 * GRID_SIZE / 7 {
            for x in 0..5 * GRID_SIZE / 8 {
                expected_areas[tile_pos_to_tile_index(Vec2i::new(x, y))] = 3;
            }
        }
        for y in 4 * GRID_SIZE / 7..4 * GRID_SIZE / 5 {
            for x in GRID_SIZE / 6..3 * GRID_SIZE / 4 {
                expected_areas[tile_pos_to_tile_index(Vec2i::new(x, y))] = 4;
            }
        }
        let grid = Grid {
            id: 0,
            revision: 0,
            segment_id: 0,
            position: Vec2i::zero(),
            heights: repeat(0.0).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
            tiles,
        };
        let mut areas = divide_into_areas(&grid);
        divide_loop_areas(&mut areas, &grid);
        assert_eq!(areas, Areas { number: 6, size: GRID_SIZE as usize, values: expected_areas });
    }

    #[test]
    fn make_area_border_should_return_border_for_square() {
        let areas = Areas {
            number: 2,
            size: 3,
            values: repeat(1).take(9).collect(),
        };
        assert_eq!(
            area_border_to_string(&areas, &make_area_border(Vec2i::zero(), 1, &areas)),
            String::from("^**\
                             $.*\
                             ***\
                             ")
        );
    }

    #[test]
    fn make_area_border_should_return_border_for_cross_like_area() {
        let mut areas = Areas {
            number: 3,
            size: 9,
            values: repeat(1).take(81).collect(),
        };
        for y in 3..6 {
            for x in 0..9 {
                set_area_by_pos(&mut areas, Vec2i::new(x, y), 2);
                set_area_by_pos(&mut areas, Vec2i::new(y, x), 2);
            }
        }
        assert_eq!(
            area_border_to_string(&areas, &make_area_border(Vec2i::new(3, 0), 2, &areas)),
            String::from("...^**...\
                             ...$.*...\
                             ...*.*...\
                             ****.****\
                             *.......*\
                             ****.****\
                             ...*.*...\
                             ...*.*...\
                             ...***...\
                             ")
        );
    }

    #[test]
    fn make_area_border_should_return_border_for_area_with_single_tile_width_part() {
        let mut areas = Areas {
            number: 3,
            size: 7,
            values: repeat(1).take(49).collect(),
        };
        for y in 2..5 {
            for x in 2..5 {
                set_area_by_pos(&mut areas, Vec2i::new(x, y), 2);
            }
        }
        for n in 0..7 {
            set_area_by_pos(&mut areas, Vec2i::new(n, 3), 2);
            set_area_by_pos(&mut areas, Vec2i::new(3, n), 2);
        }
        assert_eq!(make_area_border(Vec2i::new(2, 2), 2, &areas), vec![
            Vec2i::new(2, 2), Vec2i::new(3, 2), Vec2i::new(3, 1),
            Vec2i::new(3, 0), Vec2i::new(3, 1), Vec2i::new(3, 2),
            Vec2i::new(4, 2), Vec2i::new(4, 3), Vec2i::new(5, 3),
            Vec2i::new(6, 3), Vec2i::new(5, 3), Vec2i::new(4, 3),
            Vec2i::new(4, 4), Vec2i::new(3, 4), Vec2i::new(3, 5),
            Vec2i::new(3, 6), Vec2i::new(3, 5), Vec2i::new(3, 4),
            Vec2i::new(2, 4), Vec2i::new(2, 3), Vec2i::new(1, 3),
            Vec2i::new(0, 3), Vec2i::new(1, 3), Vec2i::new(2, 3),
        ]);
    }

    #[test]
    fn make_area_border_should_return_border_for_area_with_single_tile_width_line() {
        let mut areas = Areas {
            number: 3,
            size: 5 as usize,
            values: repeat(1).take(25).collect(),
        };
        for n in 0..3 {
            set_area_by_pos(&mut areas, Vec2i::new(0, n), 2);
        }
        for n in 0..4 {
            set_area_by_pos(&mut areas, Vec2i::new(n, 2), 2);
        }
        for n in 3..5 {
            set_area_by_pos(&mut areas, Vec2i::new(3, n), 2);
        }
        assert_eq!(make_area_border(Vec2i::zero(), 2, &areas), vec![
            Vec2i::new(0, 0), Vec2i::new(0, 1), Vec2i::new(0, 2),
            Vec2i::new(1, 2), Vec2i::new(2, 2), Vec2i::new(3, 2),
            Vec2i::new(3, 3), Vec2i::new(3, 4), Vec2i::new(3, 3),
            Vec2i::new(3, 2), Vec2i::new(2, 2), Vec2i::new(1, 2),
            Vec2i::new(0, 2), Vec2i::new(0, 1),
        ]);
    }

    #[test]
    fn make_area_border_should_return_border_for_area_with_diagonal_border_1() {
        let initial_position = Vec2i::zero();
        let mut areas = Areas {
            number: 2,
            size: 3,
            values: repeat(1).take(9).collect(),
        };
        for y in 0..3 {
            for x in y..3 {
                set_area_by_pos(&mut areas, Vec2i::new(x, y), 2);
            }
        }
        assert_eq!(make_area_border(Vec2i::zero(), 2, &areas), vec![
            Vec2i::new(0, 0), Vec2i::new(1, 0), Vec2i::new(2, 0),
            Vec2i::new(2, 1), Vec2i::new(2, 2), Vec2i::new(2, 1),
            Vec2i::new(1, 1), Vec2i::new(1, 0),
        ]);
    }

    #[test]
    fn make_area_border_should_return_border_for_area_with_diagonal_border_2() {
        let initial_position = Vec2i::zero();
        let mut areas = Areas {
            number: 2,
            size: 3,
            values: repeat(1).take(9).collect(),
        };
        for y in 0..3 {
            for x in y + 1..3 {
                set_area_by_pos(&mut areas, Vec2i::new(x, y), 2);
            }
        }
        assert_eq!(make_area_border(Vec2i::zero(), 1, &areas), vec![
            Vec2i::new(0, 0), Vec2i::new(0, 1), Vec2i::new(1, 1),
            Vec2i::new(1, 2), Vec2i::new(2, 2), Vec2i::new(1, 2),
            Vec2i::new(0, 2), Vec2i::new(0, 1),
        ]);
    }

    #[test]
    fn make_area_border_should_support_single_tile_area() {
        let initial_position = Vec2i::new(GRID_SIZE / 2, GRID_SIZE / 2);
        let area = 2;
        let mut areas = Areas {
            number: area + 1,
            size: GRID_SIZE as usize,
            values: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        set_area_by_pos(&mut areas, initial_position, area);
        assert_eq!(make_area_border(initial_position, area, &areas), vec![initial_position]);
    }

    #[test]
    fn make_area_border_should_support_two_tiles_area() {
        let initial_position = Vec2i::new(GRID_SIZE / 2, GRID_SIZE / 2);
        let area = 2;
        let mut areas = Areas {
            number: area + 1,
            size: GRID_SIZE as usize,
            values: repeat(1).take((GRID_SIZE * GRID_SIZE) as usize).collect(),
        };
        set_area_by_pos(&mut areas, initial_position, area);
        set_area_by_pos(&mut areas, initial_position + Vec2i::only_x(1), area);
        assert_eq!(make_area_border(initial_position, area, &areas), vec![
            initial_position,
            initial_position + Vec2i::only_x(1),
        ]);
    }

    #[test]
    fn make_area_border_should_support_single_tile_width_area() {
        let mut areas = Areas {
            number: 3,
            size: 3,
            values: repeat(1).take(9).collect(),
        };
        for n in 0..3 {
            set_area_by_pos(&mut areas, Vec2i::new(n, 1), 2);
        }
        assert_eq!(make_area_border(Vec2i::new(0, 1), 2, &areas), vec![
            Vec2i::new(0, 1), Vec2i::new(1, 1),
            Vec2i::new(2, 1), Vec2i::new(1, 1),
        ]);
    }

    #[test]
    fn make_area_contour_should_include_tile() {
        let border = vec![
            Vec2i::new(0, 0), Vec2i::new(99, 0),
            Vec2i::new(99, 99), Vec2i::new(0, 99),
        ];
        assert_eq!(make_area_contour(border), vec![
            Vec2i::new(0, 0), Vec2i::new(100, 0),
            Vec2i::new(100, 100), Vec2i::new(0, 100),
        ]);
    }

    #[test]
    fn make_area_contour_should_support_cross_like_border() {
        let border = vec![
            Vec2i::new(33, 0), Vec2i::new(65, 0),
            Vec2i::new(65, 33), Vec2i::new(99, 33),
            Vec2i::new(99, 65), Vec2i::new(65, 65),
            Vec2i::new(65, 99), Vec2i::new(33, 99),
            Vec2i::new(33, 65), Vec2i::new(0, 65),
            Vec2i::new(0, 33), Vec2i::new(33, 33),
        ];
        assert_eq!(make_area_contour(border), vec![
            Vec2i::new(33, 0), Vec2i::new(66, 0),
            Vec2i::new(66, 33), Vec2i::new(100, 33),
            Vec2i::new(100, 66), Vec2i::new(66, 66),
            Vec2i::new(66, 100), Vec2i::new(33, 100),
            Vec2i::new(33, 66), Vec2i::new(0, 66),
            Vec2i::new(0, 33), Vec2i::new(33, 33),
        ]);
    }

    #[test]
    fn make_area_contour_should_support_border_with_single_tile_width_part() {
        let border = vec![
            Vec2i::new(33, 33), Vec2i::new(50, 33),
            Vec2i::new(50, 31), Vec2i::new(50, 33),
            Vec2i::new(65, 33), Vec2i::new(65, 50),
            Vec2i::new(67, 50), Vec2i::new(65, 50),
            Vec2i::new(65, 65), Vec2i::new(50, 65),
            Vec2i::new(50, 67), Vec2i::new(50, 65),
            Vec2i::new(33, 65), Vec2i::new(33, 50),
            Vec2i::new(31, 50), Vec2i::new(33, 50),
        ];
        assert_eq!(make_area_contour(border), vec![
            Vec2i::new(33, 33), Vec2i::new(50, 33),
            Vec2i::new(50, 31), Vec2i::new(51, 31), Vec2i::new(51, 33),
            Vec2i::new(66, 33), Vec2i::new(66, 50),
            Vec2i::new(68, 50), Vec2i::new(68, 51), Vec2i::new(66, 51),
            Vec2i::new(66, 66), Vec2i::new(51, 66),
            Vec2i::new(51, 68), Vec2i::new(50, 68), Vec2i::new(50, 66),
            Vec2i::new(33, 66), Vec2i::new(33, 51),
            Vec2i::new(31, 51), Vec2i::new(31, 50), Vec2i::new(33, 50),
        ]);
    }

    #[test]
    fn make_area_contour_should_support_border_with_single_tile_width() {
        let border = vec![
            Vec2i::new(20, 20), Vec2i::new(20, 30),
            Vec2i::new(30, 30), Vec2i::new(30, 40),
            Vec2i::new(30, 30), Vec2i::new(20, 30),
        ];
        assert_eq!(make_area_contour(border), vec![
            Vec2i::new(20, 20), Vec2i::new(21, 20),
            Vec2i::new(21, 30), Vec2i::new(31, 30),
            Vec2i::new(31, 41), Vec2i::new(30, 41),
            Vec2i::new(30, 31), Vec2i::new(20, 31),
        ]);
    }

    #[test]
    fn make_area_contour_should_support_border_for_single_tile() {
        let border = vec![Vec2i::new(0, 0)];
        assert_eq!(make_area_contour(border), vec![
            Vec2i::new(0, 0), Vec2i::new(1, 0),
            Vec2i::new(1, 1), Vec2i::new(0, 1),
        ]);
    }

    #[test]
    fn make_area_contour_should_support_border_for_two_tiles() {
        let border = vec![Vec2i::new(0, 0), Vec2i::new(1, 0)];
        assert_eq!(make_area_contour(border), vec![
            Vec2i::new(0, 0), Vec2i::new(2, 0),
            Vec2i::new(2, 1), Vec2i::new(0, 1),
        ]);
    }

    #[test]
    fn make_area_contour_should_support_border_for_single_tile_width_by_x() {
        let border = vec![Vec2i::new(0, 0), Vec2i::new(3, 0)];
        assert_eq!(make_area_contour(border), vec![
            Vec2i::new(0, 0), Vec2i::new(4, 0),
            Vec2i::new(4, 1), Vec2i::new(0, 1),
        ]);
    }

    #[test]
    fn make_area_contour_should_support_border_for_single_tile_width_by_y() {
        let border = vec![Vec2i::new(0, 0), Vec2i::new(0, 3)];
        assert_eq!(make_area_contour(border), vec![
            Vec2i::new(0, 0), Vec2i::new(1, 0),
            Vec2i::new(1, 4), Vec2i::new(0, 4),
        ]);
    }

    #[test]
    fn smooth_area_border_should_return_empty_as_is() {
        assert_eq!(smooth_area_border(vec![]), vec![]);
    }

    #[test]
    fn smooth_area_border_should_return_one_value_as_is() {
        assert_eq!(smooth_area_border(vec![Vec2i::zero()]), vec![Vec2i::zero()]);
    }

    #[test]
    fn smooth_area_border_should_return_two_values_as_is() {
        assert_eq!(
            smooth_area_border(vec![Vec2i::new(0, 0), Vec2i::new(2, 0)]),
            vec![Vec2i::new(0, 0), Vec2i::new(2, 0)]
        );
    }

    #[test]
    fn smooth_area_border_should_not_smooth_distant_positions() {
        assert_eq!(
            smooth_area_border(vec![Vec2i::new(0, 0), Vec2i::new(2, 0), Vec2i::new(2, 2)]),
            vec![Vec2i::new(0, 0), Vec2i::new(2, 0), Vec2i::new(2, 2)]
        );
    }

    #[test]
    fn smooth_area_border_should_smooth_close_positions() {
        assert_eq!(
            smooth_area_border(vec![Vec2i::new(0, 0), Vec2i::new(1, 0), Vec2i::new(1, 1)]),
            vec![Vec2i::new(0, 0), Vec2i::new(1, 1)]
        );
    }

    #[test]
    fn smooth_area_border_should_smooth_multiple_close_positions() {
        assert_eq!(
            smooth_area_border(vec![
                Vec2i::new(0, 0), Vec2i::new(1, 0), Vec2i::new(1, 1),
                Vec2i::new(2, 1), Vec2i::new(2, 2),
            ]),
            vec![Vec2i::new(0, 0), Vec2i::new(1, 1), Vec2i::new(2, 2)]
        );
    }

    #[test]
    fn remove_redundant_border_positions_should_support_dead_end() {
        assert_eq!(
            remove_redundant_border_positions(vec![
                Vec2i::new(4, 4), Vec2i::new(4, 5), Vec2i::new(4, 6),
                Vec2i::new(4, 5), Vec2i::new(3, 5), Vec2i::new(3, 4),
            ]),
            vec![
                Vec2i::new(4, 4), Vec2i::new(4, 6),
                Vec2i::new(4, 5), Vec2i::new(3, 5), Vec2i::new(3, 4),
            ]
        );
    }

    #[test]
    fn remove_redundant_border_positions_should_return_empty_as_is() {
        assert_eq!(remove_redundant_border_positions(vec![]), vec![]);
    }

    #[test]
    fn remove_redundant_border_positions_should_return_single_value_as_is() {
        assert_eq!(remove_redundant_border_positions(vec![Vec2i::zero()]), vec![Vec2i::zero()]);
    }

    #[test]
    fn remove_redundant_border_positions_should_return_two_values_as_is() {
        assert_eq!(remove_redundant_border_positions(vec![Vec2i::zero(), Vec2i::zero()]), vec![Vec2i::zero(), Vec2i::zero()]);
    }

    #[test]
    fn remove_redundant_border_positions_should_remove_positions_on_the_same_line() {
        assert_eq!(
            remove_redundant_border_positions(vec![
                Vec2i::new(0, 0), Vec2i::new(1, 1), Vec2i::new(2, 2),
            ]),
            vec![Vec2i::new(0, 0), Vec2i::new(2, 2)]
        );
    }

    #[test]
    fn remove_redundant_border_positions_should_remove_positions_on_the_same_line_2() {
        assert_eq!(
            remove_redundant_border_positions(vec![
                Vec2i::new(0, 0), Vec2i::new(1, 1), Vec2i::new(2, 2),
            ]),
            vec![Vec2i::new(0, 0), Vec2i::new(2, 2)]
        );
    }

    fn area_border_to_string(areas: &Areas, border: &Vec<Vec2i>) -> String {
        let mut result: Vec<u8> = repeat('.' as u8).take(areas.size * areas.size).collect();
        if !border.is_empty() {
            result[pos_to_index(border[0], areas.size)] = '^' as u8;
            for i in 1..border.len() - 1 {
                result[pos_to_index(border[i], areas.size)] = '*' as u8;
            }
            if border.len() > 1 {
                result[pos_to_index(border[border.len() - 1], areas.size)] = '$' as u8;
            }
        }
        String::from_utf8(result).unwrap()
    }

    fn set_area_by_pos(areas: &mut Areas, position: Vec2i, value: u16) {
        areas.values[position.x() as usize + position.y() as usize * areas.size as usize] = value;
    }
}
