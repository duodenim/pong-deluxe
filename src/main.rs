use specs::{Component, VecStorage, NullStorage, World, Builder, System, Read, ReadStorage, WriteStorage, DispatcherBuilder};
use specs_derive::{Component};

mod render;
use render::{RenderComponent, Vertex};
mod fy_math;
use fy_math::{Vec2,TransformComponent};

const AXIS_MAX: f32 = 32768.0;

const VERTICES: [Vertex; 4] = [Vertex { position: Vec2{ x: -0.5, y: 0.5} },
                               Vertex { position: Vec2{ x: 0.5, y: 0.5}  },
                               Vertex { position: Vec2{ x: 0.5, y: -0.5} },
                               Vertex { position: Vec2{ x: -0.5, y: -0.5} }];

#[derive(Component)]
#[storage(VecStorage)]
struct PhysicsComponent {
    velocity: Vec2
}

#[derive(Component, Default)]
#[storage(NullStorage)]
struct Ball;

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

struct PhysicsSystem;

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (Read<'a, DeltaTime>, Read<'a, TotalTime>, WriteStorage<'a, PhysicsComponent>, WriteStorage<'a, TransformComponent>);

    fn run(&mut self, _: Self::SystemData) {
    }
}

struct UpdateBall;

impl<'a> System<'a> for UpdateBall {
    type SystemData = (ReadStorage<'a, Ball>, WriteStorage<'a, TransformComponent>, Read<'a, Controllers>);

    fn run(&mut self, (ball_storage, mut transform_storage, controller_storage): Self::SystemData) {
        use specs::Join;
        let (x_axis, y_axis) = if controller_storage.0.len() > 0 {
            (controller_storage.0[0].left_axis_x, controller_storage.0[0].left_axis_y)
        } else {
            (0.0, 0.0)
        };
        for (_, t) in (&ball_storage, &mut transform_storage).join() {
            t.position.x = x_axis;
            t.position.y = y_axis;
        }
    }
}

struct UpdatePaddles;

impl<'a> System<'a> for UpdatePaddles {
    type SystemData = (ReadStorage<'a, Paddle>, ReadStorage<'a, TransformComponent>);

    fn run(&mut self, (paddle_storage, transform_storage): Self::SystemData) {
        use specs::Join;

        for (paddles, t) in (&paddle_storage, &transform_storage).join() {
            
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

     let _paddle1 = {
        let transform = TransformComponent {
            position: Vec2::new(0.9, 0.0)
        };
        let physics = PhysicsComponent {
            velocity: Vec2::new(-0.6, 1.3)
        };
        let paddle = Paddle {
            player_idx: 0
        };

        let model = RenderComponent::new(&mut renderer, &VERTICES);
        world.create_entity().with(transform).with(paddle).with(model).with(physics).build()
    };

    let _paddle2 = {
        let transform = TransformComponent {
            position: Vec2::new(-0.9, 0.0)
        };
        let physics = PhysicsComponent {
            velocity: Vec2::new(-0.9, 0.05)
        };

        let paddle = Paddle {
            player_idx: 1
        };
        let model = RenderComponent::new(&mut renderer, &VERTICES);
        world.create_entity().with(transform).with(paddle).with(model).with(physics).build()
    };

    let _ball = {
        let transform = TransformComponent {
            position: Vec2::new(0.0, 0.0)
        };
        let physics = PhysicsComponent {
            velocity: Vec2::new(0.5, 0.4)
        };
        let model = RenderComponent::new(&mut renderer, &VERTICES);
        world.create_entity().with(model).with(Ball).with(transform).with(physics).build();
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
