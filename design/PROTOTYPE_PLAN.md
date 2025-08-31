# 🛠 Thalassocracy — Prototype Plan

**Goal:** a runnable, networked prototype that showcases the solo mining loop for multiple players.  
Core loop: fly skiff → fight current → mine node → haul back → dock → sell.

---

## High-Level Architecture

- **Client:** Bevy (Rust). ECS for gameplay; simple 3D cave, skiff, HUD, basic physics.
- **Server:** standalone Rust binary. Maintains world state, validates movement/mining, owns resource nodes and station wallet.
- **Networking:** UDP/QUIC via `quinn` or ENet-style via `bevy_renet`. Use client prediction + server reconciliation.
- **Persistence:** in-memory; optional JSON snapshot for credits.

---

## Milestones

### Milestone 0 — Scaffolding
- [ ] Rust workspace with `client/`, `server/`, and `protocol/` crate.
- [ ] Define protocol messages (serde).
- [ ] Clients can connect to server.

### Milestone 1 — World Greybox
- [ ] One station cavern, one ΔP tunnel, one mining chamber.
- [ ] Flow field in tunnel.
- [ ] Docking pad volume.

### Milestone 2 — Skiff and HUD
- [ ] Skiff entity: thrust, ballast, drag; server authoritative.
- [ ] Client inputs with prediction + reconciliation.
- [ ] HUD: speed, hull, cargo, pressure, current arrow.
- [ ] ΔP adds to physics tick.

### Milestone 3 — Mining and Cargo
- [ ] Resource nodes with remaining mass.
- [ ] Mining interaction: hold key → add ore → cargo cap.
- [ ] Cargo weight affects handling.
- [ ] Node depletes, despawns.

### Milestone 4 — Station Economy Stub
- [ ] Dock → “Press E” prompt.
- [ ] Server sells cargo for credits.
- [ ] Wallet tracked per player.

### Milestone 5 — Pressure & Failure
- [ ] Pressure rises with depth/distance.
- [ ] Above threshold → hull damage over time.
- [ ] Implosion = respawn at station, credits persist.

### Milestone 6 — Polish & Ops
- [ ] Interpolation + dead reckoning for remote entities.
- [ ] AOI (area of interest) to reduce network traffic.
- [ ] Headless Linux build + Dockerfile.
- [ ] Playtest with 10–20 clients on VPS.

---

## To-Do List by Subsystem

### Networking / Protocol
- [ ] Implement messages: Hello, JoinAck, InputTick, StateDelta, MineRequest/Ack, DockRequest/Ack.
- [ ] Client prediction with rollback.
- [ ] Snapshot deltas and AOI culling.

### Server Gameplay
- [ ] ECS: players, skiffs, nodes, station, flow field.
- [ ] Physics tick (30 Hz).
- [ ] Pressure damage system.
- [ ] Mining system with validation.
- [ ] Docking system with credit payout.

### Client Gameplay
- [ ] Input buffer + prediction.
- [ ] HUD overlays.
- [ ] Interpolation of remote skiffs.

### Content
- [ ] Greybox meshes (.glb): station cavern, tunnel, chamber.
- [ ] Placeholder skiff (capsule) and ore node (sphere).
- [ ] Flat-color materials; emissive for dock.

### Build & Ops
- [ ] Server config file (port, max clients, tick rate).
- [ ] Dockerfile + docker-compose.
- [ ] Logging and basic metrics.

---

## Scope Guardrails

- ❌ No combat, divers, rails, factions, chips.  
- ❌ No advanced art, lighting, or economy systems.  
- ✅ Focus: the **feel** of piloting, mining, hauling, and selling in multiplayer.

---

## Playtest Flow

1. Spawn at station with empty skiff.  
2. Navigate ΔP tunnel, fighting current.  
3. Mine ore in chamber; cargo fills, handling worsens.  
4. Return through tunnel, pressure rising.  
5. Dock, sell ore, see credits tick up.  
6. Repeat alongside other players to feel “shared space.”