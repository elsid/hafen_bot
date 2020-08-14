use crate::bot::vec2::{Vec2f, Vec2i};

pub struct WalkGrid {
    ax: f64,
    ay: f64,
    nx: f64,
    ny: f64,
    sign_x: i32,
    sign_y: i32,
    avx: f64,
    avy: f64,
    to_border_x: f64,
    to_border_y: f64,
    fraction_x: f64,
    fraction_y: f64,
    point: Vec2i,
}

impl WalkGrid {
    #[inline(always)]
    pub fn new(begin: Vec2f, end: Vec2f) -> Self {
        let fraction_x = adjust_fraction(begin.x().fract());
        let fraction_y = adjust_fraction(begin.y().fract());
        let point = Vec2i::new(begin.x() as i32, begin.y() as i32);
        let to = end - make_position(point, fraction_x, fraction_y);
        let v = to.normalized();
        let sign_x = v.x().signum() as i32;
        let sign_y = v.y().signum() as i32;
        let to_border_x = if sign_x >= 0 {
            1.0 - fraction_x
        } else {
            fraction_x
        };
        let to_border_y = if sign_y >= 0 {
            1.0 - fraction_y
        } else {
            fraction_y
        };
        Self {
            ax: 0.0,
            ay: 0.0,
            nx: to.x().abs(),
            ny: to.y().abs(),
            sign_x,
            sign_y,
            avx: v.x().abs(),
            avy: v.y().abs(),
            to_border_x,
            to_border_y,
            fraction_x,
            fraction_y,
            point,
        }
    }
}

fn adjust_fraction(value: f64) -> f64 {
    if 0.0 < value && value < 1e-9 {
        1e-9
    } else if 1.0 - 1e-9 < value && value < 1.0 {
        1.0 - 1e-9
    } else {
        value
    }
}

impl Iterator for WalkGrid {
    type Item = Vec2f;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.avx != 0.0 && self.avy != 0.0 && self.ax <= self.nx && self.ay <= self.ny {
            let point = self.point;
            let dtx = self.to_border_x / self.avx;
            let dty = self.to_border_y / self.avy;

            if dtx < dty {
                self.point.add_x(self.sign_x);
                let dy = self.avy * dtx;
                self.ax += self.to_border_x;
                self.ay += dy;
                self.to_border_x = 1.0;
                self.to_border_y = (self.to_border_y - dy).max(0.0);
            } else {
                self.point.add_y(self.sign_y);
                let dx = self.avx * dty;
                self.ax += dx;
                self.ay += self.to_border_y;
                self.to_border_x = (self.to_border_x - dx).max(0.0);
                self.to_border_y = 1.0;
            }

            Some(make_position(point, self.fraction_x, self.fraction_y))
        } else if self.avx != 0.0 && self.avy == 0.0 && self.ax <= self.nx {
            let point = self.point;

            self.point.add_x(self.sign_x);
            self.ax += self.to_border_x;
            self.to_border_x = 1.0;

            Some(make_position(point, self.fraction_x, self.fraction_y))
        } else if self.avx == 0.0 && self.avy != 0.0 && self.ay <= self.ny {
            let point = self.point;

            self.point.add_y(self.sign_y);
            self.ay += self.to_border_y;
            self.to_border_y = 1.0;

            Some(make_position(point, self.fraction_x, self.fraction_y))
        } else {
            None
        }
    }
}

fn make_position(point: Vec2i, fraction_x: f64, fraction_y: f64) -> Vec2f {
    Vec2f::new(point.x() as f64 + fraction_x, point.y() as f64 + fraction_y)
}
