use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

use crate::bot::math::{floor_div_i32, Square};

#[derive(Default, Clone, Copy, Debug, PartialOrd, Serialize, Deserialize)]
pub struct Vec2f {
    x: f64,
    y: f64,
}

impl Vec2f {
    #[inline(always)]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[inline(always)]
    pub const fn x(&self) -> f64 {
        self.x
    }

    #[inline(always)]
    pub const fn y(&self) -> f64 {
        self.y
    }

    #[inline(always)]
    pub fn norm(&self) -> f64 {
        (self.x.square() + self.y.square()).sqrt()
    }

    #[inline(always)]
    pub fn distance(&self, other: Self) -> f64 {
        (other - *self).norm()
    }

    #[inline(always)]
    pub fn signum(&self) -> Self {
        Self { x: self.x.signum(), y: self.y.signum() }
    }

    #[inline(always)]
    pub fn floor(&self) -> Self {
        Self::new(self.x.floor(), self.y.floor())
    }

    #[inline(always)]
    pub fn floor_by(&self, other: f64) -> Self {
        (*self / other).floor()
    }
}

impl From<Vec2i> for Vec2f {
    fn from(value: Vec2i) -> Self {
        Self::new(value.x() as f64, value.y() as f64)
    }
}

impl Add for Vec2f {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2f {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f64> for Vec2f {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Mul for Vec2f {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y)
    }
}

impl Div<f64> for Vec2f {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: f64) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl Div for Vec2f {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.x / rhs.x, self.y / rhs.y)
    }
}

impl Neg for Vec2f {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

impl PartialEq for Vec2f {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        (self.x, self.y).eq(&(rhs.x, rhs.y))
    }
}

impl Eq for Vec2f {}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Vec2i {
    x: i32,
    y: i32,
}

impl Vec2i {
    #[inline(always)]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[inline(always)]
    pub const fn zero() -> Self {
        Self { x: 0, y: 0 }
    }

    #[inline(always)]
    pub const fn x(&self) -> i32 {
        self.x
    }

    #[inline(always)]
    pub const fn y(&self) -> i32 {
        self.y
    }

    #[inline(always)]
    pub const fn only_x(x: i32) -> Self {
        Self { x, y: 0 }
    }

    #[inline(always)]
    pub const fn only_y(y: i32) -> Self {
        Self { x: 0, y }
    }

    #[inline(always)]
    pub const fn with_x(&self, x: i32) -> Self {
        Self::new(x, self.y)
    }

    #[inline(always)]
    pub const fn with_y(&self, y: i32) -> Self {
        Self::new(self.x, y)
    }

    #[inline(always)]
    pub fn center(&self) -> Vec2f {
        Vec2f::new(self.x as f64 + 0.5, self.y as f64 + 0.5)
    }

    #[inline(always)]
    pub const fn floor_div_i32(&self, value: i32) -> Self {
        Self::new(floor_div_i32(self.x, value), floor_div_i32(self.y, value))
    }
}

impl From<Vec2f> for Vec2i {
    fn from(value: Vec2f) -> Self {
        Self::new(value.x() as i32, value.y() as i32)
    }
}

impl Add for Vec2i {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Vec2i::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2i {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Vec2i::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul for Vec2i {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Vec2i::new(self.x * rhs.x, self.y * rhs.y)
    }
}

impl Mul<i32> for Vec2i {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: i32) -> Self::Output {
        Vec2i::new(self.x * rhs, self.y * rhs)
    }
}

impl AddAssign for Vec2i {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            x: self.x + other.x,
            y: self.y + other.y,
        };
    }
}
