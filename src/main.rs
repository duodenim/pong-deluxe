use specs::{Component, VecStorage, NullStorage, World, Builder, System, Read, ReadStorage, WriteStorage, DispatcherBuilder};
use specs_derive::{Component};

mod render;
use render::RenderComponent;
mod fy_math;
use fy_math::{Vec2,TransformComponent};

#[derive(Component)]
#[storage(VecStorage)]
struct PhysicsComponent {
    x: f32,
    y: f32
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
struct Time(f32);

struct PhysicsSystem;

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (Read<'a, Time>, WriteStorage<'a, PhysicsComponent>);

    fn run(&mut self, (time, mut phys_c): Self::SystemData) {
        use specs::Join;

        for p in (&mut phys_c).join() {
            p.y = time.0.sin();
        }
    }
}

struct BallSystem;

impl<'a> System<'a> for BallSystem {
    type SystemData = (ReadStorage<'a, Ball>, WriteStorage<'a, TransformComponent>, Read<'a, Time>);

    fn run(&mut self, (ball_storage, mut transform_storage, time): Self::SystemData) {
        use specs::Join;

        let time = time.0;
        for (_, t) in (&ball_storage, &mut transform_storage).join() {
            t.position.x = time.sin();
            t.position.y = 0.0;
        }
    }
}

struct UpdatePaddles;

impl<'a> System<'a> for UpdatePaddles {
    type SystemData = (ReadStorage<'a, Paddle>, ReadStorage<'a, PhysicsComponent>);

    fn run(&mut self, (paddle_storage, physics_storage): Self::SystemData) {
        use specs::Join;

        for (paddles, physics) in (&paddle_storage, &physics_storage).join() {
            println!("Paddle player index: {} is at position ( {}, {} )", paddles.player_idx, physics.x, physics.y);
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

    let _paddle1 = {
        let pos = PhysicsComponent {
            x: 1.0,
            y: 0.0
        };
        let paddle = Paddle {
            player_idx: 0
        };
        world.create_entity().with(pos).with(paddle).build()
    };

    let _paddle2 = {
        let pos = PhysicsComponent {
            x: -1.0,
            y: 0.0
        };

        let paddle = Paddle {
            player_idx: 1
        };
        world.create_entity().with(pos).with(paddle).build()
    };

    let _ball = {
        let transform = TransformComponent {
            position: Vec2 {
                x: 0.5,
                y: 0.0
            }
        };
        world.create_entity().with(RenderComponent).with(Ball).with(transform).build();
    };

    let sdl_context = sdl2::init().unwrap();
    let video_context = sdl_context.video().unwrap();
    let mut events = sdl_context.event_pump().unwrap();
    let window = video_context.window("Pong2", 640, 480).vulkan().build().unwrap();

    let renderer = render::RenderContext::new(&window, 640, 480);

    world.add_resource(Time(0.0));

    let num_threads = num_cpus::get();
    let thread_pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();
    let thread_pool = std::sync::Arc::new(thread_pool);

    let mut dispatcher = DispatcherBuilder::new()
        .with(PhysicsSystem, "physics", &[])
        .with(BallSystem, "ball", &["physics"])
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
        let mut time = world.write_resource::<Time>();
        *time = Time(time.0 + 0.01);
        drop(time);
        dispatcher.dispatch(&mut world.res);
        world.maintain();
    }
    
}
