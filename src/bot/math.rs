use std::ops::Mul;

pub trait Square: Mul + Copy {
    fn square(self) -> Self::Output {
        self * self
    }
}

impl Square for f64 {}

pub const fn floor_div_i32(lhs: i32, rhs: i32) -> i32 {
    if lhs < 0 {
        (lhs + 1) / rhs - 1
    } else {
        lhs / rhs
    }
}

pub fn as_score(value: f64) -> i32 {
    (value * 100000.0).round() as i32
}
