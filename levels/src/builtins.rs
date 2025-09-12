use crate::{LevelSpec, RoomSpec, TunnelSpec, ChamberSpec, TorusTunnelSpec, TorusExitSpec, FlowFieldSpec, Vec3f};

// Mirrors the current greybox layout used in the prototype.
pub fn greybox_level() -> LevelSpec {
    // Station room
    let room_w = 240.0;
    let room_h = 48.0;
    let room_d = 240.0;
    let wall_thick = 2.0;

    // Tunnel
    let tunnel_len = 288.0;
    let tunnel_h = 24.0;
    let tunnel_w = 32.0;
    let tunnel_pos = Vec3f::new(room_w * 0.5 + tunnel_len * 0.5, 4.0, 0.0);

    // Chamber at end of tunnel
    let chamber_size = Vec3f::new(160.0, 40.0, 160.0);
    let chamber_pos = Vec3f::new(room_w * 0.5 + tunnel_len + chamber_size.x * 0.5, 4.0, 0.0);

    LevelSpec {
        room: RoomSpec {
            size: Vec3f::new(room_w, room_h, room_d),
            wall_thickness: wall_thick,
            dock_size: Vec3f::new(12.0, 0.8, 12.0),
            dock_pos: Vec3f::new(-16.0, 0.4, -16.0),
        },
        tunnel: TunnelSpec {
            size: Vec3f::new(tunnel_len, tunnel_h, tunnel_w),
            pos: tunnel_pos,
            shell_thickness: wall_thick,
            flow: FlowFieldSpec::Uniform { flow: Vec3f::new(1.5, 0.0, 0.0), variance: 0.2 },
        },
        chamber: ChamberSpec {
            size: chamber_size,
            pos: chamber_pos,
        },
        torus_tunnel: None,
    }
}

/// A more complex layout featuring a torus‑shaped tunnel (ring) between the
/// station room and the mining chamber. The ring has two exits roughly 160°
/// apart: one oriented toward the station dock, one toward the mining chamber.
pub fn torus_two_exit_level() -> LevelSpec {
    // Base dimensions (reuse the greybox proportions for room/chamber placement)
    let room_w = 240.0;
    let room_h = 48.0;
    let room_d = 240.0;
    let wall_thick = 2.0;

    // Straight tunnel is retained (for compatibility/physics sampling) but
    // the torus ring sits around its midpoint and can be used by the client
    // to render a curved path with two branches.
    let tunnel_len = 288.0;
    let tunnel_h = 24.0;
    let tunnel_w = 32.0;
    let tunnel_pos = Vec3f::new(room_w * 0.5 + tunnel_len * 0.5, 4.0, 0.0);

    // Mining chamber at the end of the straight tunnel
    let chamber_size = Vec3f::new(160.0, 40.0, 160.0);
    let chamber_pos = Vec3f::new(room_w * 0.5 + tunnel_len + chamber_size.x * 0.5, 4.0, 0.0);

    // Torus ring parameters
    // Place centered on the straight tunnel center, lying flat in XZ plane
    let torus_center = tunnel_pos;
    let torus_axis = Vec3f::new(0.0, 1.0, 0.0); // horizontal ring
    let major_radius = 60.0; // ring radius
    let minor_radius = 10.0; // tube interior radius (open space)
    let torus_wall = 2.0;    // shell thickness similar to room/tunnel

    // Exits: choose two angles ~160° apart.
    //  - Mining chamber exit: angled slightly toward +X (e.g., +20°)
    //  - Dock exit: opposite side at +180° (roughly 160° separation from +20°)
    let exit_to_chamber = TorusExitSpec { angle_deg: 20.0, width_deg: 35.0, label: "mining_chamber".to_string() };
    let exit_to_dock = TorusExitSpec { angle_deg: 180.0, width_deg: 35.0, label: "dock".to_string() };

    LevelSpec {
        room: RoomSpec {
            size: Vec3f::new(room_w, room_h, room_d),
            wall_thickness: wall_thick,
            dock_size: Vec3f::new(12.0, 0.8, 12.0),
            dock_pos: Vec3f::new(-16.0, 0.4, -16.0),
        },
        tunnel: TunnelSpec {
            size: Vec3f::new(tunnel_len, tunnel_h, tunnel_w),
            pos: tunnel_pos,
            shell_thickness: wall_thick,
            // Mild forward flow through the straight section (+X in world)
            flow: FlowFieldSpec::Uniform { flow: Vec3f::new(2.0, 0.0, 0.2), variance: 0.15 },
        },
        chamber: ChamberSpec { size: chamber_size, pos: chamber_pos },
        torus_tunnel: Some(TorusTunnelSpec {
            center: torus_center,
            axis: torus_axis,
            major_radius,
            minor_radius,
            wall_thickness: torus_wall,
            // Uniform magnitude along +X; the client/physics may choose to align to local tangent.
            flow: FlowFieldSpec::Uniform { flow: Vec3f::new(2.5, 0.0, 0.0), variance: 0.2 },
            exits: [exit_to_dock, exit_to_chamber],
        }),
    }
}
