use specs::{Component, VecStorage, Entity, World, Builder, System, Read, ReadStorage, WriteStorage, DispatcherBuilder};
use specs_derive::{Component};

use rand::{thread_rng, Rng};

mod render;
use render::{RenderComponent, Vertex};
mod fy_math;
use fy_math::{Vec2,TransformComponent};
mod physics;
use physics::{PhysicsComponent, PhysicsSystem};

const AXIS_MAX: f32 = 32768.0;

const BOUNCE_OFFSET: f32 = 15.0;

const BALL_VERTICES: [Vertex; 4] = [Vertex { position: Vec2{ x: -0.05, y: 0.05} },
                               Vertex { position: Vec2{ x: 0.05, y: 0.05}  },
                               Vertex { position: Vec2{ x: 0.05, y: -0.05} },
                               Vertex { position: Vec2{ x: -0.05, y: -0.05} }];

const PADDLE_VERTICES: [Vertex; 4] = [Vertex { position: Vec2{ x: -0.07, y: 0.2} },
                               Vertex { position: Vec2{ x: 0.07, y: 0.2}  },
                               Vertex { position: Vec2{ x: 0.07, y: -0.2} },
                               Vertex { position: Vec2{ x: -0.07, y: -0.2} }];

const WALL_VERTICES: [Vertex; 4] = [Vertex { position: Vec2{ x: -1.0, y: 0.05} },
                               Vertex { position: Vec2{ x: 1.0, y: 0.05}  },
                               Vertex { position: Vec2{ x: 1.0, y: -0.05} },
                               Vertex { position: Vec2{ x: -1.0, y: -0.05} }];


const INDICES: [u32; 6] = [0,1,2,0,2,3];

#[derive(Component)]
#[storage(VecStorage)]
struct Ball {
    left_paddle: Entity,
    right_paddle: Entity
}

impl Ball {
    fn new(left_paddle: Entity, right_paddle: Entity) -> Ball {
        Ball {
            left_paddle,
            right_paddle
        }
    }
}

#[derive(Component)]
#[storage(VecStorage)]
struct Paddle {
    player_idx: u32
}

#[derive(Default)]
struct DeltaTime(f32);

#[derive(Default)]
struct TotalTime(f32);

struct ControllerState {
    left_axis_x: f32,
    left_axis_y: f32
}

#[derive(Default)]
struct Controllers(std::vec::Vec<ControllerState>);

struct UpdateBall;

impl<'a> System<'a> for UpdateBall {
    type SystemData = (ReadStorage<'a, Ball>, WriteStorage<'a, TransformComponent>, WriteStorage<'a, PhysicsComponent>, Read<'a, DeltaTime>);

    fn run(&mut self, (ball_storage, mut transform_storage, mut physics_storage, deltatime): Self::SystemData) {
        use specs::Join;
        let deltatime = deltatime.0;
        for (ball, t, phys_c) in (&ball_storage, &mut transform_storage, &mut physics_storage).join() {
            //Check for collision against paddles
            for other_collider in phys_c.collided_objects.iter() {
                if *other_collider == ball.left_paddle {
                    let mut rng = thread_rng();
                    let angle = rng.gen_range(-1.0 * BOUNCE_OFFSET, 1.0 * BOUNCE_OFFSET);
                    let y_offset = angle.to_radians().sin();
                    phys_c.velocity.x *= -1.0;
                    phys_c.velocity.y += y_offset;
                } else if *other_collider == ball.right_paddle {
                    let mut rng = thread_rng();
                    let angle = rng.gen_range(-1.0 * BOUNCE_OFFSET, 1.0 * BOUNCE_OFFSET);
                    let y_offset = angle.to_radians().sin();
                    phys_c.velocity.x *= -1.0;
                    phys_c.velocity.y += y_offset;
                } else {
                    phys_c.velocity.y *= -1.0;
                }
            }
            t.position.x = t.position.x + phys_c.velocity.x * deltatime;
            t.position.y = t.position.y + phys_c.velocity.y * deltatime;

            //Check for score conditions
            let mut reset = false;
            if t.position.x > 1.3 {
                println!("Player 2 has scored!");
                reset = true;
            } else if t.position.x < -1.3 {
                println!("Player 1 has scored!");
                reset = true;
            }

            if reset {
                t.position = Vec2::new(0.0, 0.0);
                let mut rng = thread_rng();
                let angle: f32 = rng.gen_range(0.0, 360.0);
                let x = angle.to_radians().cos();
                let y = angle.to_radians().sin();
                phys_c.velocity = 0.5 * Vec2::new(x, y);
            }
        }
    }
}

struct UpdatePaddles;

impl<'a> System<'a> for UpdatePaddles {
    type SystemData = (ReadStorage<'a, Paddle>, WriteStorage<'a, TransformComponent>, Read<'a, Controllers>);

    fn run(&mut self, (paddle_storage, mut transform_storage, controller_storage): Self::SystemData) {
        use specs::Join;

        for (paddle, t) in (&paddle_storage, &mut transform_storage).join() {
            let position = if paddle.player_idx < controller_storage.0.len() as u32 {
                controller_storage.0[paddle.player_idx as usize].left_axis_y
            } else {
                0.0
            };
            t.position.y = position;
        }
    }
}

fn main() {
    let mut world = World::new();
    world.register::<PhysicsComponent>();
    world.register::<Ball>();
    world.register::<Paddle>();
    world.register::<RenderComponent>();
    world.register::<TransformComponent>();

    let sdl_context = sdl2::init().unwrap();

    //Print off information about connected controllers
    let controller_system = sdl_context.game_controller().unwrap();

    let num_sticks = controller_system.num_joysticks().unwrap();
    println!("{} game controllers are connected", num_sticks);

    let mut controllers = Vec::new();
    let mut controller_data = Vec::new();
    for i in 0..num_sticks {
        let name = controller_system.name_for_index(i).unwrap();
        println!("{}", name);
        if controller_system.is_game_controller(i) {
            let mut c = controller_system.open(i).unwrap();
            c.set_rumble(0xffff, 0xffff, 300).unwrap();
            controllers.push(c);
            let c_data = ControllerState {
                left_axis_x: 0.0,
                left_axis_y: 0.0
            };
            controller_data.push(c_data);
        }
    }
    let video_context = sdl_context.video().unwrap();
    let mut events = sdl_context.event_pump().unwrap();
    let window = video_context.window("Pong2", 640, 480).vulkan().build().unwrap();

    world.add_resource(DeltaTime(0.01));
    world.add_resource(TotalTime(0.0));
    world.add_resource(Controllers(controller_data));

    let num_threads = num_cpus::get();
    let thread_pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();
    let thread_pool = std::sync::Arc::new(thread_pool);

    let mut renderer = render::RenderContext::new(&window, 640, 480, thread_pool.clone(), num_threads);

     let paddle1 = {
        let transform = TransformComponent {
            position: Vec2::new(0.9, 0.0)
        };
        let physics = PhysicsComponent::new(&PADDLE_VERTICES);
        let paddle = Paddle {
            player_idx: 0
        };

        let model = RenderComponent::new(&mut renderer, &PADDLE_VERTICES, &INDICES);
        world.create_entity().with(transform).with(paddle).with(model).with(physics).build()
    };

    let paddle2 = {
        let transform = TransformComponent {
            position: Vec2::new(-0.9, 0.0)
        };
        let physics = PhysicsComponent::new(&PADDLE_VERTICES);

        let paddle = Paddle {
            player_idx: 1
        };
        let model = RenderComponent::new(&mut renderer, &PADDLE_VERTICES, &INDICES);
        world.create_entity().with(transform).with(paddle).with(model).with(physics).build()
    };

    let _ball = {
        let transform = TransformComponent {
            position: Vec2::new(0.0, 0.0)
        };
        let physics = PhysicsComponent::with_velocity(&BALL_VERTICES, Vec2::new(0.5, 0.0));
        let model = RenderComponent::new(&mut renderer, &BALL_VERTICES, &INDICES);
        let ball = Ball::new(paddle2, paddle1);
        world.create_entity().with(model).with(ball).with(transform).with(physics).build();
    };

    let _top_wall = {
        let transform = TransformComponent {
            position: Vec2::new(0.0, -0.9)
        };
        let physics = PhysicsComponent::new(&WALL_VERTICES);
        let model = RenderComponent::new(&mut renderer, &WALL_VERTICES, &INDICES);
        world.create_entity().with(transform).with(physics).with(model).build()
    };

    let _bot_wall = {
        let transform = TransformComponent {
            position: Vec2::new(0.0, 0.9)
        };
        let physics = PhysicsComponent::new(&WALL_VERTICES);
        let model = RenderComponent::new(&mut renderer, &WALL_VERTICES, &INDICES);
        world.create_entity().with(transform).with(physics).with(model).build()
    };

    let mut dispatcher = DispatcherBuilder::new()
        .with(PhysicsSystem, "physics", &[])
        .with(UpdateBall, "ball", &["physics"])
        .with(UpdatePaddles, "paddles", &["physics"])
        .with(renderer, "rendering", &["ball", "paddles"])
        .with_pool(thread_pool)
        .build();

    'mainloop: loop {
        for event in events.poll_iter() {
            match event {
                sdl2::event::Event::Quit {..} => {
                    break 'mainloop
                },
                _ => {}
            }
        }
        let mut controller_data = world.write_resource::<Controllers>();
        for (i, controller) in controllers.iter().enumerate() {
            let x = controller.axis(sdl2::controller::Axis::LeftX);
            let y = controller.axis(sdl2::controller::Axis::LeftY);
            let x = x as f32 / AXIS_MAX;
            let y = y as f32 / AXIS_MAX;
            controller_data.0[i].left_axis_x = x;
            controller_data.0[i].left_axis_y = y;
        }
        drop(controller_data);
        let mut time = world.write_resource::<TotalTime>();
        let dt = world.read_resource::<DeltaTime>();
        time.0 += dt.0;
        drop(time);
        drop(dt);
        dispatcher.dispatch(&mut world.res);
        world.maintain();
    }
    
}
