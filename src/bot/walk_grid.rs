use crate::bot::vec2::Vec2f;

pub fn walk_grid<F: FnMut(Vec2f) -> bool>(begin: Vec2f, end: Vec2f, mut f: F) -> bool {
    let to = end - begin;
    if to.x() != 0.0 && to.y() != 0.0 {
        walk_grid_diagonal(begin, end, to, f)
    } else if to.y() == 0.0 {
        walk_grid_horizontal(begin, end.x(), to.x().signum(), f)
    } else if to.x() == 0.0 {
        walk_grid_vertical(begin, end.y(), to.y().signum(), f)
    } else {
        f(begin.floor())
    }
}

fn walk_grid_diagonal<F: FnMut(Vec2f) -> bool>(begin: Vec2f, end: Vec2f, to: Vec2f, mut f: F) -> bool {
    let sign = to.signum();
    let mut position = begin;
    if !f(get_grid(begin, sign)) {
        return false;
    }
    loop {
        let border = get_border(position, sign);
        let to_border = border - position;
        let by_x = to * (to_border.x() / to.x());
        let by_y = to * (to_border.y() / to.y());
        let by_x_norm = by_x.norm();
        let by_y_norm = by_y.norm();
        let (next_position, both) = if by_x_norm < by_y_norm {
            (Vec2f::new(border.x(), position.y() + by_x.y()), false)
        } else if by_x_norm > by_y_norm {
            (Vec2f::new(position.x() + by_y.x(), border.y()), false)
        } else {
            (border, true)
        };
        if (end - next_position).signum() != sign {
            break;
        }
        if both {
            if !f(get_grid(Vec2f::new(border.x(), position.y()), sign)) {
                return false;
            }
            if !f(get_grid(Vec2f::new(position.x(), border.y()), sign)) {
                return false;
            }
        }
        position = next_position;
        if !f(get_grid(position, sign)) {
            return false;
        }
    }
    true
}

fn walk_grid_horizontal<F: FnMut(Vec2f) -> bool>(begin: Vec2f, end_x: f64, sign: f64, mut f: F) -> bool {
    let grid_y = get_grid_coord(begin.y(), sign);
    let both = begin.y().fract() == 0.0;
    let mut x = begin.x();
    loop {
        let grid_x = get_grid_coord(x, sign);
        if both {
            if !f(Vec2f::new(grid_x, grid_y - sign)) {
                return false;
            }
        }
        if !f(Vec2f::new(grid_x, grid_y)) {
            return false;
        }
        x = get_border_coord(x, sign);
        let left = end_x - x;
        if left != 0.0 && left.signum() != sign {
            break;
        }
    }
    true
}

fn walk_grid_vertical<F: FnMut(Vec2f) -> bool>(begin: Vec2f, end_y: f64, sign: f64, mut f: F) -> bool {
    let grid_x = get_grid_coord(begin.x(), sign);
    let both = begin.x().fract() == 0.0;
    let mut y = begin.y();
    loop {
        let grid_y = get_grid_coord(y, sign);
        if both {
            if !f(Vec2f::new(grid_x - sign, grid_y)) {
                return false;
            }
        }
        if !f(Vec2f::new(grid_x, grid_y)) {
            return false;
        }
        y = get_border_coord(y, sign);
        let left = end_y - y;
        if left != 0.0 && left.signum() != sign {
            break;
        }
    }
    true
}

fn get_border(position: Vec2f, sign: Vec2f) -> Vec2f {
    Vec2f::new(
        get_border_coord(position.x(), sign.x()),
        get_border_coord(position.y(), sign.y()),
    )
}

fn get_border_coord(value: f64, sign: f64) -> f64 {
    if sign > 0.0 {
        value.floor() + 1.0
    } else {
        value.ceil() - 1.0
    }
}

fn get_grid(position: Vec2f, sign: Vec2f) -> Vec2f {
    Vec2f::new(
        get_grid_coord(position.x(), sign.x()),
        get_grid_coord(position.y(), sign.y()),
    )
}

fn get_grid_coord(value: f64, sign: f64) -> f64 {
    if sign > 0.0 {
        value.floor()
    } else {
        (value - 1.0).ceil()
    }
}

#[test]
pub fn test_walk_grid_center_by_horizontal() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(0.5, 0.5), Vec2f::new(2.5, 0.5), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 0.0), Vec2f::new(2.0, 0.0)]
    );
}

#[test]
pub fn test_walk_grid_center_by_horizontal_for_negative_coordinates() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(-0.5, -0.5), Vec2f::new(-2.5, -0.5), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![Vec2f::new(-1.0, -1.0), Vec2f::new(-2.0, -1.0), Vec2f::new(-3.0, -1.0)]
    );
}

#[test]
pub fn test_walk_grid_border_by_horizontal() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(0.0, 0.0), Vec2f::new(2.0, 0.0), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![
            Vec2f::new(0.0, -1.0), Vec2f::new(0.0, 0.0), Vec2f::new(1.0, -1.0),
            Vec2f::new(1.0, 0.0), Vec2f::new(2.0, -1.0), Vec2f::new(2.0, 0.0),
        ]
    );
}

#[test]
pub fn test_walk_grid_center_by_vertical() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(0.5, 0.5), Vec2f::new(0.5, 2.5), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![Vec2f::new(0.0, 0.0), Vec2f::new(0.0, 1.0), Vec2f::new(0.0, 2.0)]
    );
}

#[test]
pub fn test_walk_grid_center_by_vertical_for_negative_coordinates() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(-0.5, -0.5), Vec2f::new(-0.5, -2.5), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![Vec2f::new(-1.0, -1.0), Vec2f::new(-1.0, -2.0), Vec2f::new(-1.0, -3.0)]
    );
}

#[test]
pub fn test_walk_grid_border_by_vertical() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(0.0, 0.0), Vec2f::new(0.0, 2.0), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![
            Vec2f::new(-1.0, 0.0), Vec2f::new(0.0, 0.0), Vec2f::new(-1.0, 1.0),
            Vec2f::new(0.0, 1.0), Vec2f::new(-1.0, 2.0), Vec2f::new(0.0, 2.0),
        ]
    );
}

#[test]
pub fn test_walk_grid_center() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(0.5, 0.5), Vec2f::new(1.5, 2.5), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![Vec2f::new(0.0, 0.0), Vec2f::new(0.0, 1.0), Vec2f::new(1.0, 1.0), Vec2f::new(1.0, 2.0)]
    );
}

#[test]
pub fn test_walk_grid_center_for_negative_coordinates() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(-0.5, -0.5), Vec2f::new(-1.5, -2.5), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![Vec2f::new(-1.0, -1.0), Vec2f::new(-1.0, -2.0), Vec2f::new(-2.0, -2.0), Vec2f::new(-2.0, -3.0)]
    );
}

#[test]
pub fn test_walk_grid_from_border_to_border() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(0.5, 0.1), Vec2f::new(2.5, 0.9), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 0.0), Vec2f::new(2.0, 0.0)]
    );
}

#[test]
pub fn test_walk_grid_from_center_to_border() {
    let mut result = Vec::new();
    walk_grid(Vec2f::new(0.5, 0.5), Vec2f::new(2.5, 0.9), |v| {
        result.push(v);
        true
    });
    assert_eq!(
        result,
        vec![Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 0.0), Vec2f::new(2.0, 0.0)]
    );
}

#[test]
pub fn test_walk_grid() {
    let begin = Vec2f::new(38.000000000000455, 12.066666666666693);
    let end = Vec2f::new(1.549171296296291, 11.9);
    let mut previous = begin;
    walk_grid(begin, end, |position| {
        assert!((position.x() - previous.x()).abs() <= 1.0 && (position.y() - previous.y()).abs() <= 1.0,
                "{:?} {:?} {}", position, previous, (position.x() - previous.x()).abs());
        previous = position;
        true
    });
    assert_eq!(previous, Vec2f::new(1.0, 11.0));
}

#[test]
pub fn test_walk_grid_2() {
    let begin = Vec2f::new(25.121666666678834, 18.066666667666514);
    let end = Vec2f::new(9.549999999, 16.516666666667387);
    let mut previous = begin;
    walk_grid(begin, end, |position| {
        assert!((position.x() - previous.x()).abs() <= 1.0 && (position.y() - previous.y()).abs() <= 1.0,
                "{:?} {:?} {}", position, previous, (position.x() - previous.x()).abs());
        previous = position;
        true
    });
    assert_eq!(previous, Vec2f::new(9.0, 16.0));
}

#[test]
pub fn test_walk_grid_3() {
    let begin = Vec2f::new(25.788333333344895, 17.900000001);
    let end = Vec2f::new(9.831666665666711, 17.900000001);
    let mut previous = begin;
    walk_grid(begin, end, |position| {
        assert!((position.x() - previous.x()).abs() <= 1.0 && (position.y() - previous.y()).abs() <= 1.0,
                "{:?} {:?} {}", position, previous, (position.x() - previous.x()).abs());
        previous = position;
        true
    });
    assert_eq!(previous, Vec2f::new(9.0, 17.0));
}

#[test]
pub fn test_walk_grid_4() {
    let begin = Vec2f::new(4.999999999999999, 22.10000000000406);
    let end = Vec2f::new(31.185097964514384, 30.2);
    let mut previous = begin;
    walk_grid(begin, end, |position| {
        assert!((position.x() - previous.x()).abs() <= 1.0 && (position.y() - previous.y()).abs() <= 1.0,
                "{:?} {:?} {}", position, previous, (position.x() - previous.x()).abs());
        previous = position;
        true
    });
    assert_eq!(previous, Vec2f::new(31.0, 30.0));
}

#[test]
pub fn test_walk_grid_5() {
    let begin = Vec2f::new(17.999999999999847, 16.90000000000372);
    let end = Vec2f::new(18.000000000082558, 0.0);
    let mut previous = begin;
    walk_grid(begin, end, |position| {
        assert!((position.x() - previous.x()).abs() <= 1.0 && (position.y() - previous.y()).abs() <= 1.0,
                "{:?} {:?} {}", position, previous, (position.x() - previous.x()).abs());
        previous = position;
        true
    });
    assert_eq!(previous, Vec2f::new(18.0, 0.0));
}
