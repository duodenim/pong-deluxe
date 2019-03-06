use specs::{Component, DenseVecStorage};
use specs_derive::{Component};

#[derive(Default)]
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
    pub fn new() -> Vec2 {
        Vec2 {
            x: 0.0,
            y: 0.0
        }
    }
}

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct TransformComponent {
    pub position: Vec2
}