use std::ops::Mul;

pub trait Square: Mul + Copy {
    fn square(self) -> Self::Output {
        self * self
    }
}

impl Square for f64 {}

pub trait Clamp1: PartialOrd + Sized {
    fn clamp1(self, min: Self, max: Self) -> Self {
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

impl Clamp1 for f32 {}

impl Clamp1 for f64 {}

impl Clamp1 for usize {}

pub fn floor_div_i32(lhs: i32, rhs: i32) -> i32 {
    if lhs < 0 {
        (lhs + 1) / rhs - 1
    } else {
        lhs / rhs
    }
}

pub fn as_score(value: f64) -> i32 {
    (value * 100000.0).round() as i32
}
