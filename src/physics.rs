use specs::{Builder, Component, VecStorage, System, Read, Write, WriteStorage, ReadStorage, Entities, world::Index, Entity};
use specs_derive::{Component};

use crate::fy_math::{TransformComponent, Vec2};
use crate::render::{Vertex};

struct AABB {
    top_right: Vec2,
    bot_left: Vec2
}

pub struct Collision {
    pub other: Entity,
    pub mtv: Vec2
}

#[derive(Component)]
#[storage(VecStorage)]
pub struct PhysicsComponent {
    pub velocity: Vec2,
    bbox: AABB,
    pub collided_objects: Vec<Collision>
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

    fn check_collision(&self, other: &AABB) -> Option<Vec2> {
        //Simplified SAT implementation, used instead of AABB test to get collision normal

        let mut overlap = std::f32::MAX;
        let mut axis = Vec2::new(1.0, 0.0);
        //Project onto X axis
        {
            let this_min_x = self.bot_left.x;
            let this_max_x = self.top_right.x;
            let other_min_x = other.bot_left.x;
            let other_max_x = other.top_right.x;

            if this_max_x < other_min_x || other_max_x < this_min_x {
                return None;
            } else {
                let new_overlap = this_max_x.min(other_max_x) - this_min_x.max(other_min_x);
                if new_overlap < overlap {
                    overlap = new_overlap;
                }
            }
        }

        //Project onto Y axis
        {
            let this_min_y = self.bot_left.y;
            let this_max_y = self.top_right.y;
            let other_min_y = other.bot_left.y;
            let other_max_y = other.top_right.y;
            if this_max_y < other_min_y || other_max_y < this_min_y {
                return None;
            } else {
                let new_overlap = this_max_y.min(other_max_y) - this_min_y.max(other_min_y);
                if new_overlap < overlap {
                    overlap = new_overlap;
                    axis = Vec2::new(0.0, 1.0);
                }
            }
        }

        return Some(axis);
    }
}

pub struct PhysicsSystem;

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (WriteStorage<'a, PhysicsComponent>, WriteStorage<'a, TransformComponent>, Entities<'a>);

    fn run(&mut self, (mut physics_storage, mut transform_storage, entities): Self::SystemData) {
        use specs::Join;
        use itertools::Itertools;
        let num_colliders = physics_storage.count();
        let mut collision_map: Vec<(Entity, Entity, Vec2)> = Vec::new();
        for combination in (&physics_storage, &transform_storage, &entities).join().combinations(2) {
            let (collider1, transform1, e1) = combination[0];
            let (collider2, transform2, e2) = combination[1];
            let box1 = collider1.bbox.adjust_position(transform1.position);
            let box2 = collider2.bbox.adjust_position(transform2.position);

            match box1.check_collision(&box2) {
                None => {},
                Some(axis) => {
                    let t2_to_t1 = transform2.position - transform1.position;
                    if axis.dot(&t2_to_t1) >= 0.0 {
                        collision_map.push((e1, e2, -1.0 * axis));
                    } else {
                        collision_map.push((e1, e2, axis));
                    }
                }
            }
        }

        for phys_obj in (&mut physics_storage).join() {
            phys_obj.collided_objects.clear();
        }

        for collision in collision_map.iter() {
            let (e1, e2, mtv) = collision;
            let phys_comp1 = match physics_storage.get_mut(*e1) {
                None => {
                    panic!("Collision from unknown entity occured!");
                },
                Some(comp) => {
                    comp
                }
            };
            phys_comp1.collided_objects.push(Collision {
                other: *e2,
                mtv: *mtv
            });
            let phys_comp2 = match physics_storage.get_mut(*e2) {
                None => {
                    panic!("Collision from unknown entity occured!");
                },
                Some(comp) => {
                    comp
                }
            };
            phys_comp2.collided_objects.push(Collision {
                other: *e1,
                mtv: *mtv
            });
        }
    }
}