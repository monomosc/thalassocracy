## bevy considerations

Note, that many APIs have been changed from requiring a `Color` struct to a`LinearRgba` struct.

### No more bundles

Bundles have been deprecated; replaced by "Required Components" - one effect of this is that `PbrBundle` does not exist, instead you should manually add the `Mesh`, `MeshMaterial3d`, and `Transform` components. 

### Assets

The asset path for the client is fixed at "assets/fonts" not "client/assets/fonts"

### Egui

Egui UI systems need to happen in the EguiPrimaryContextPass Schedule


### Misc

For each logical system you implement, try to think of telemetry/metrics and maybe expose them via DebugVis in whatever way feels best (in order of preference):

- a moving graph
- a render gizmo like one or many arrows or lines
- numbers shown top-left

If you don't implement observability, tell me why.