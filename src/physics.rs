use specs::{Builder, Component, VecStorage, System, Read, Write, WriteStorage, ReadStorage, Entities, world::Index, Entity};
use specs_derive::{Component};

use crate::fy_math::{TransformComponent, Vec2};
use crate::render::{Vertex};

struct AABB {
    top_right: Vec2,
    bot_left: Vec2
}

#[derive(Component)]
#[storage(VecStorage)]
pub struct PhysicsComponent {
    pub velocity: Vec2,
    bbox: AABB,
    pub collided_objects: Vec<Entity>
}

impl PhysicsComponent {
    pub fn new(vertices: &[Vertex]) -> PhysicsComponent {
        PhysicsComponent {
            velocity: Vec2::new(0.0, 0.0),
            bbox: AABB::from_vertices(vertices),
            collided_objects: Vec::new()
        }
    }

    pub fn with_velocity(vertices: &[Vertex], velocity: Vec2) -> PhysicsComponent {
        PhysicsComponent {
            velocity,
            bbox: AABB::from_vertices(vertices),
            collided_objects: Vec::new()
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

    fn adjust_position(&self, position: Vec2) -> AABB {
        let new_top = Vec2::new(self.top_right.x + position.x, self.top_right.y + position.y);
        let new_bot = Vec2::new(self.bot_left.x + position.x, self.bot_left.y + position.y);
        AABB {
            top_right: new_top,
            bot_left: new_bot
        }
    }

    fn check_collision(&self, other: &AABB) -> bool {
        if self.top_right.x < other.bot_left.x {
            return false;
        }
        if self.bot_left.x > other.top_right.x {
            return false;
        }
        if self.top_right.y < other.bot_left.y {
            return false;
        }
        if self.bot_left.y > other.top_right.y {
            return false;
        }

        return true;
    }
}

pub struct PhysicsSystem;

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (WriteStorage<'a, PhysicsComponent>, WriteStorage<'a, TransformComponent>, Entities<'a>);

    fn run(&mut self, (mut physics_storage, mut transform_storage, entities): Self::SystemData) {
        use specs::Join;
        use itertools::Itertools;
        let num_colliders = physics_storage.count();
        let mut collision_map: Vec<(Entity, Entity)> = Vec::new();
        for combination in (&physics_storage, &transform_storage, &entities).join().combinations(2) {
            let (collider1, transform1, e1) = combination[0];
            let (collider2, transform2, e2) = combination[1];
            let box1 = collider1.bbox.adjust_position(transform1.position);
            let box2 = collider2.bbox.adjust_position(transform2.position);
            if box1.check_collision(&box2) {
                collision_map.push((e1, e2));
            }
        }

        for phys_obj in (&mut physics_storage).join() {
            phys_obj.collided_objects.clear();
        }

        for collision in collision_map.iter() {
            let (e1, e2) = collision;
            let phys_comp1 = match physics_storage.get_mut(*e1) {
                None => {
                    panic!("Collision from unknown entity occured!");
                },
                Some(comp) => {
                    comp
                }
            };
            phys_comp1.collided_objects.push(*e2);
            let phys_comp2 = match physics_storage.get_mut(*e2) {
                None => {
                    panic!("Collision from unknown entity occured!");
                },
                Some(comp) => {
                    comp
                }
            };
            phys_comp2.collided_objects.push(*e1);
        }
    }
}