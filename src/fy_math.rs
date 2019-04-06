use specs::{Component, DenseVecStorage};
use specs_derive::{Component};
use std::ops;

#[derive(Default, Copy, Clone)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32
}

#[repr(C)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32
}

#[repr(C)]
pub struct Mat4 {
    pub x: Vec4,
    pub y: Vec4,
    pub z: Vec4,
    pub w: Vec4
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Vec2 {
        Vec2 {
            x,
            y
        }
    }
}

impl ops::Mul<f32> for Vec2 {
    type Output = Vec2;

    fn mul(self, _rhs: f32) -> Vec2 {
        Vec2 {
            x: self.x * _rhs,
            y: self.y * _rhs
        }
    }
}

impl ops::Mul<Vec2> for f32 {
    type Output = Vec2;

    fn mul(self, _rhs: Vec2) -> Vec2 {
        _rhs * self
    }
}

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct TransformComponent {
    pub position: Vec2
}