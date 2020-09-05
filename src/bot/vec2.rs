use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

use crate::bot::common::{Clamp1, floor_div_i32, Square};

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
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    #[inline(always)]
    pub const fn i() -> Self {
        Self::only_x(1.0)
    }

    #[inline(always)]
    pub const fn only_x(x: f64) -> Self {
        Self { x, y: 0.0 }
    }

    #[inline(always)]
    pub const fn only_y(y: f64) -> Self {
        Self { x: 0.0, y }
    }

    #[inline(always)]
    pub const fn both(value: f64) -> Self {
        Self { x: value, y: value }
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
    pub fn set_x(&mut self, value: f64) {
        self.x = value;
    }

    #[inline(always)]
    pub fn set_y(&mut self, value: f64) {
        self.y = value;
    }

    #[inline(always)]
    pub fn add_x(&mut self, value: f64) {
        self.x += value;
    }

    #[inline(always)]
    pub fn add_y(&mut self, value: f64) {
        self.y += value;
    }

    #[inline(always)]
    pub fn normalized(&self) -> Self {
        *self / self.norm()
    }

    #[inline(always)]
    pub fn rotated(&self, angle: f64) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self::new(self.x * cos - self.y * sin, self.y * cos + self.x * sin)
    }

    #[inline(always)]
    pub fn atan(&self) -> f64 {
        self.y.atan2(self.x)
    }

    #[inline(always)]
    pub fn cos(&self, other: Self) -> f64 {
        (self.dot(other) / (self.norm() * other.norm())).clamp1(-1.0, 1.0)
    }

    #[inline(always)]
    pub fn dot(&self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    #[inline(always)]
    pub fn angle(&self) -> f64 {
        self.y.atan2(self.x)
    }

    #[inline(always)]
    pub fn signum(&self) -> Self {
        Self { x: self.x.signum(), y: self.y.signum() }
    }

    #[inline(always)]
    pub fn abs(&self) -> Self {
        Self { x: self.x.abs(), y: self.y.abs() }
    }

    #[inline(always)]
    pub fn left(&self) -> Self {
        Self { x: -self.y, y: self.x }
    }

    #[inline(always)]
    pub fn right(&self) -> Self {
        Self { x: self.y, y: -self.x }
    }

    #[inline(always)]
    pub fn rotation(&self, other: Self) -> f64 {
        self.cos(other).acos()
    }

    #[inline(always)]
    pub fn floor(&self) -> Vec2i {
        Vec2i::new(self.x.floor() as i32, self.y.floor() as i32)
    }

    #[inline(always)]
    pub fn floor_by(&self, other: f64) -> Vec2i {
        (*self / other).floor()
    }
}

impl Add for Vec2f {
    type Output = Vec2f;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Vec2f::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2f {
    type Output = Vec2f;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Vec2f::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f64> for Vec2f {
    type Output = Vec2f;

    #[inline(always)]
    fn mul(self, rhs: f64) -> Self::Output {
        Vec2f::new(self.x * rhs, self.y * rhs)
    }
}

impl Mul for Vec2f {
    type Output = Vec2f;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Vec2f::new(self.x * rhs.x, self.y * rhs.y)
    }
}

impl Div<f64> for Vec2f {
    type Output = Vec2f;

    #[inline(always)]
    fn div(self, rhs: f64) -> Self::Output {
        Vec2f::new(self.x / rhs, self.y / rhs)
    }
}

impl Div for Vec2f {
    type Output = Vec2f;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        Vec2f::new(self.x / rhs.x, self.y / rhs.y)
    }
}

impl Neg for Vec2f {
    type Output = Vec2f;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        Vec2f::new(-self.x, -self.y)
    }
}

impl PartialEq for Vec2f {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        (self.x, self.y).eq(&(rhs.x, rhs.y))
    }
}

impl Eq for Vec2f {}

impl From<Vec2i> for Vec2f {
    fn from(value: Vec2i) -> Self {
        Self::new(value.x() as f64, value.y() as f64)
    }
}

impl AddAssign for Vec2f {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            x: self.x + other.x,
            y: self.y + other.y,
        };
    }
}

impl SubAssign for Vec2f {
    fn sub_assign(&mut self, other: Self) {
        *self = Self {
            x: self.x - other.x,
            y: self.y - other.y,
        };
    }
}

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
    pub fn add_x(&mut self, x: i32) {
        self.x += x;
    }

    #[inline(always)]
    pub fn add_y(&mut self, y: i32) {
        self.y += y;
    }

    #[inline(always)]
    pub fn with_x(&self, x: i32) -> Self {
        Self::new(x, self.y)
    }

    #[inline(always)]
    pub fn with_y(&self, y: i32) -> Self {
        Self::new(self.x, y)
    }

    #[inline(always)]
    pub fn center(&self) -> Vec2f {
        Vec2f::new(self.x as f64 + 0.5, self.y as f64 + 0.5)
    }

    #[inline(always)]
    pub fn floor_div_i32(&self, value: i32) -> Self {
        Self::new(floor_div_i32(self.x, value), floor_div_i32(self.y, value))
    }
}

impl Add for Vec2i {
    type Output = Vec2i;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Vec2i::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2i {
    type Output = Vec2i;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Vec2i::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul for Vec2i {
    type Output = Vec2i;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Vec2i::new(self.x * rhs.x, self.y * rhs.y)
    }
}

impl Mul<i32> for Vec2i {
    type Output = Vec2i;

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
