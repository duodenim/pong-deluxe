use specs::{Builder, Component, VecStorage, System, ReadStorage};
use specs_derive::{Component};

use crate::fy_math::{Vec2};
use crate::render::{Vertex};

struct AABB {
    top_right: Vec2,
    bot_left: Vec2
}

#[derive(Component)]
#[storage(VecStorage)]
pub struct PhysicsComponent {
    velocity: Vec2,
    bbox: AABB
}

impl PhysicsComponent {
    pub fn new(vertices: &[Vertex]) -> PhysicsComponent {
        PhysicsComponent {
            velocity: Vec2::new(0.0, 0.0),
            bbox: AABB::from_vertices(vertices)
        }
    }
}

impl AABB {
    pub fn new(top_right: Vec2, bot_left: Vec2) -> AABB {
        AABB {
            top_right,
            bot_left
        }
    }

    pub fn from_vertices(vertices: &[Vertex]) -> AABB {
        assert!(vertices.len() >= 2, "Cannot build a bbox around a single point!");
        let first = vertices[0].position;
        let (min, max) = vertices.iter().fold((first, first), | (curr_min, curr_max), vtx | {
            let new_max_x = if vtx.position.x > curr_max.x {
                vtx.position.x
            } else {
                curr_max.x
            };
            let new_max_y = if vtx.position.y > curr_max.y {
                vtx.position.y
            } else {
                curr_max.y
            };
            let new_min_x = if vtx.position.x < curr_min.x {
                vtx.position.x
            } else {
                curr_min.x
            };
            let new_min_y = if vtx.position.y < curr_min.y {
                vtx.position.y
            } else {
                curr_min.y
            };
            (Vec2::new(new_min_x, new_min_y), Vec2::new(new_max_x, new_max_y))
        });
        AABB {
            top_right: max,
            bot_left: min
        }
    }
}