# 🌊 Thalassocracy — Design Document

## Game Concept

*Thalassocracy* is a persistent, player-driven submarine game set inside a stratified world of flooded caves.  
The surface ocean boils under a nearby sun; perpetual rainfall meets volcanic craters that spit molten salt.  
Where salt and rain collide, dense “brinefalls” plunge through the crust into vast caverns below.  
These cascades carve the only reliable vertical arteries. Everything else is rock, pressure, and risk.

Players pilot submarines to explore, mine, trade, and fight. Stations—built and owned by players—anchor local economies: they buy raw materials, refine them, sell upgrades and fuel, and provide docking and repair.  
The single NPC “starter hub” imports high-tech chips (the only real money sink), while all other logistics, services, and prices are set by players.  
Small hulls thrive in the shallows; deep, strategic resources require organized groups that can manage distance, pressure, and supply lines.

Identity is persistent. Every hull broadcasts a registry ID when it transmits. Piracy is viable, but anonymity must be paid for: a comm-scrambler chip masks a pirate’s ID in the moment yet leaks “forensic fingerprints” hours later, creating accountability without killing ransom gameplay.  
Wars are not blob-DPS contests; they are logistics struggles over rails, hydrolocks, brinefalls, and the stations that depend on them.

---

## World and Resources

Caves form recognizable biomes:

- **Shallows:** biomass pockets, copper, iron.
- **Mid-depth caverns:** crystal veins, ΔP tunnels with violent currents.
- **Lower caverns:** brine lakes, volatile gas pockets.
- **Brine-bottom:** titanium nodules and uranium veins.

Ordinary refined brine fuels small craft cheaply but has poor energy density.  
Capital ships run on uranium rods; in low-power “brine-sustain” mode they can keep lights and life support indefinitely but cannot maneuver or fight meaningfully.  
A rare **abyssal brine** exists — 100× denser than ordinary brine — found only in deep fissures or fauna nests.  
It grants extraordinary range to small and mid hulls and lets capitals operate between rod shipments, but it’s too scarce to replace rods for extreme maneuvers like climbing a brinefall.

The economy is simple and legible:

- Raw resources: iron, copper, biomass, volatiles, titanium, uranium, abyssal brine.
- Station owners refine or resell resources.
- The starter hub posts floor prices and sells chips, with chip prices tied to supply.
- Stations expose buy/sell curves tied to storage levels, quotas, and escrow.

This creates natural negative feedback without heavy NPC intervention.

---

## Travel and Movement

Travel is play, not downtime.

- ΔP tunnels shove hulls off axis; pilots trim ballast and yaw, engineers juggle grapples and thrusters.
- Some tunnels are hydrolocked and safe; many are not.
- Factions affix **anchor rails** along walls; subs **ratchet** upward by latching and winching one catchpoint at a time.
- Rails create chokepoints, tolls, and ambush spaces.
- Brinefalls are the only vertical arteries into/out of the brine-bottom.

Dreadnoughts can descend easily but can only ascend via extraordinary, public “extraction” operations: tug fleets, kilometers of cable, staged fuel, and hours of real-time ratcheting.  
This makes committing a capital hull to the bottom a strategic decision with weight.

---

## Roles and Loops

- **Solo miners:** sorties from shallow/mid stations, mine small nodes, smuggle rarities later.
- **Explorers:** map new caves, sell intel, mark rails, hazards.
- **Traders:** run convoys, thread hydrolocks, profit on arbitrage.
- **Pirates:** intercept and ransom via scrambled comms; registry reveals traces hours later.
- **Station owners:** tune price/storage curves, run refineries, commission rails.
- **Companies/Syndicates/Thalassocracies:** coordinate logistics, own stations, wage slow wars over brinefalls.

---

## Dynamic Map

Caves are impermanent. Each has a **lifetime of ~1 month**:

- Collapse seals it; new caves open elsewhere.
- Global activity controls new cave spawn rate → world shrinks with low players, expands with high.
- Prevents permanent monopolies and keeps exploration relevant.
- Future: factions can stabilize caves with hydrolocks/braces.

---

## Social Contract and Safety

- Registry ties speech to ships; comm scramblers allow temporary anonymity.
- Stations define safe bubbles; camping docks is unprofitable.
- Insurance/black-box payouts cushion catastrophic losses without trivializing risk.
- Escrow contracts let broke players work as crew, ensuring solvency.
- Piracy is allowed, griefing is costly due to registry exposure, bounties, and docking penalties.

**Philosophy:** harsh but fair — not a second job.

---

## Tech Notes

- Math Types: the `levels` crate uses `bevy_math` 0.16 for vectors/quaternions.
  - Re-exports: `levels::Vec3f` = `bevy_math::Vec3`, `levels::Quatf` = `bevy_math::Quat`.
  - Serde enabled for cross-crate serialization.
- Coordinates & Conventions: see `design/COORDINATES_AND_CONVENTIONS.md` for the definitive basis/signs used across physics, HUD, and camera.
- Heading/Yaw: compute from the rotated forward vector in XZ (body +Z forward).
  - `let f = orientation * Vec3f::new(0.0, 0.0, 1.0);`
  - `let yaw = (-f.x).atan2(f.z); // +yaw turns nose left`
- Submarine Physics: split into focused modules under `levels/src/submarine_physics/`.
  - `types.rs`: public structs (`SubState`, `SubInputs`, `SubStepDebug`).
  - `flow.rs`: flow field sampling.
  - `dynamics.rs`: thrust/rudder/buoyancy/drag integration.
