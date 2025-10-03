#[cfg(test)]
mod integration {
    use std::net::UdpSocket;
    use std::time::Duration;

    use anyhow::Result;
    use bevy_app::{App, Startup, Update};
    use bevy_ecs::prelude::*;
    use bevy_renet::renet::{DefaultChannel, RenetClient};
    use bevy_time::Time;
    use bevy_transform::components::{GlobalTransform, Transform};
    use client::net::FilteredServerState;
    use client::scene::submarine::{
        AngularVelocity, SubPhysics, SubStateComp, Submarine, Velocity,
    };
    use client::{build_minimal_client_app, Args as ClientArgs};
    use levels::{subspecs::small_skiff_spec, Quatf, SubState, Vec3f};
    use protocol::ClientToServer;
    use server::{build_server_app, Config, ServerAddresses, SubStateComp as ServerSubStateComp};

    const HARD_THRESHOLD: f32 = 0.2;
    const SOFT_THRESHOLD: f32 = 0.1;
    const HANDSHAKE_DT: f32 = 1.0 / 60.0;
    const SIM_DT: f32 = 1.0 / 30.0;
    const HANDSHAKE_STEPS: usize = 600;
    const WARMUP_STEPS: usize = 120;
    const SIM_STEPS: usize = 10_000;
    const IGNORE_STEPS: usize = 128;

    #[derive(Resource, Default)]
    struct TestThrottleState {
        tick: u64,
    }

    fn reserve_udp_port() -> u16 {
        UdpSocket::bind(("127.0.0.1", 0))
            .expect("bind temp udp socket")
            .local_addr()
            .expect("local_addr")
            .port()
    }

    fn advance_app(app: &mut App, dt: f32) {
        if let Some(mut time) = app.world_mut().get_resource_mut::<Time>() {
            time.advance_by(Duration::from_secs_f32(dt));
        }
        app.update();
    }

    fn spawn_test_submarine(mut commands: Commands) {
        let spec = small_skiff_spec();
        let ballast = vec![0.5; spec.ballast_tanks.len()];
        commands.spawn((
            Submarine,
            Transform::default(),
            GlobalTransform::default(),
            Velocity::default(),
            AngularVelocity::default(),
            SubPhysics(spec),
            SubStateComp(SubState {
                position: Vec3f::new(0.0, 0.0, 0.0),
                velocity: Vec3f::new(0.0, 0.0, 0.0),
                orientation: Quatf::IDENTITY,
                ang_mom: Vec3f::new(0.0, 0.0, 0.0),
                ballast_fill: ballast,
            }),
        ));
    }

    fn drive_full_throttle(
        client: Option<ResMut<RenetClient>>,
        mut throttle: ResMut<TestThrottleState>,
    ) {
        let Some(mut client) = client else {
            return;
        };
        if !client.is_connected() {
            return;
        }

        throttle.tick = throttle.tick.wrapping_add(1);
        let msg = ClientToServer::InputTick(protocol::InputTick {
            tick: throttle.tick,
            thrust: 1.0,
            yaw: 0.0,
            pump_fwd: 0.0,
            pump_aft: 0.0,
        });
        if let Ok(bytes) = protocol::encode(&msg) {
            client.send_message(DefaultChannel::ReliableOrdered, bytes);
        }
    }

    fn server_sub_position(app: &App) -> Option<[f32; 3]> {
        app.world().iter_entities().find_map(|entity| {
            entity.get::<ServerSubStateComp>().map(|state| {
                let pos = state.0.position;
                [pos.x, pos.y, pos.z]
            })
        })
    }

    fn client_latest_position(app: &App) -> Option<[f32; 3]> {
        app.world()
            .get_resource::<FilteredServerState>()
            .and_then(|filtered| {
                if filtered.initialized {
                    Some([filtered.pos.x, filtered.pos.y, filtered.pos.z])
                } else {
                    None
                }
            })
    }

    fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    #[test]
    fn client_prediction_stays_close_to_server() -> Result<()> {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let port = reserve_udp_port();
        let cfg = Config {
            port,
            ..Config::default()
        };

        let mut server_app = build_server_app(cfg);
        for _ in 0..HANDSHAKE_STEPS {
            advance_app(&mut server_app, HANDSHAKE_DT);
        }

        let server_addr = {
            let world = server_app.world();
            world
                .get_resource::<ServerAddresses>()
                .map(|addr| addr.public)
                .unwrap_or_else(|| format!("127.0.0.1:{port}").parse().unwrap())
        };

        let client_args = ClientArgs {
            server: server_addr.to_string(),
            headless: true,
            name: Some("integration-test".to_string()),
            connect_timeout_secs: 5,
        };

        let mut client_app = build_minimal_client_app(client_args);
        client_app.add_systems(Startup, spawn_test_submarine);
        client_app.insert_resource(TestThrottleState::default());
        client_app.add_systems(Update, drive_full_throttle);

        let mut connected = false;
        for _ in 0..HANDSHAKE_STEPS {
            advance_app(&mut server_app, HANDSHAKE_DT);
            advance_app(&mut client_app, HANDSHAKE_DT);
            if !connected
                && client_app
                    .world()
                    .get_resource::<RenetClient>()
                    .map(|c| c.is_connected())
                    .unwrap_or(false)
            {
                connected = true;
            }
            if connected
                && server_sub_position(&server_app).is_some()
                && client_latest_position(&client_app).is_some()
            {
                break;
            }
        }

        assert!(
            connected,
            "client failed to connect within handshake window"
        );
        assert!(
            server_sub_position(&server_app).is_some(),
            "server never spawned player submarine"
        );
        assert!(
            client_latest_position(&client_app).is_some(),
            "client never received latest server state"
        );

        for _ in 0..WARMUP_STEPS {
            advance_app(&mut server_app, SIM_DT);
            advance_app(&mut client_app, SIM_DT);
        }

        let mut max_delta = 0.0f32;
        for step in 0..SIM_STEPS {
            advance_app(&mut server_app, SIM_DT);
            advance_app(&mut client_app, SIM_DT);

            if let (Some(server_pos), Some(filtered_pos)) = (
                server_sub_position(&server_app),
                client_latest_position(&client_app),
            ) {
                let delta = distance(server_pos, filtered_pos);
                if step >= IGNORE_STEPS {
                    max_delta = max_delta.max(delta);
                    assert!(
                        delta < HARD_THRESHOLD,
                        "predicted divergence {delta:.5} exceeded hard limit"
                    );
                }
            }
        }

        assert!(
            max_delta < SOFT_THRESHOLD,
            "max divergence {max_delta:.5} exceeded target {SOFT_THRESHOLD}"
        );

        Ok(())
    }
}
