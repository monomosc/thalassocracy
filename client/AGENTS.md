## bevy considerations

Note, that many APIs have been changed from requiring a `Color` struct to a`LinearRgba` struct.

### No more bundles

Bundles have been deprecated; replaced by "Required Components" - one effect of this is that `PbrBundle` does not exist, instead you should manually add the `Mesh`, `MeshMaterial3d`, and `Transform` components. 
