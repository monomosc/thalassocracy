# Volumetric Floodlight Debug Modes

The `VolumetricConeShaderDebugSettings::debug_mode` slider controls what the cone shader writes to the color buffer.

| Mode | Output | Description |
| ---- | ------ | ----------- |
| 0 | Beauty | Normal volumetric shading. |
| 1 | Hit Ratio | Fraction of march steps that contributed (bright = most steps inside the cone, dark = no contributions). Helps spot pixels where the march keeps skipping samples. |
| 2 | Weight Ratio | Average per-step weight (angular/radial). Highlights pixels that sit near the rim or outside the cone even if steps were taken. |
| 3 | Clamped Length | Length of the ray segment after depth/shadow clamp, normalized to the cone range (with contrast boost). Reveals where depth or shadows truncate the march. |
| 4 | Raw Length | The analytical cone intersection length before any depth clamp, normalized to the cone range (contrast boosted). Shows how much of the cone the pixel could traverse in an unobstructed scene. |
| 5 | Depth Sample | Visualises the depth value used for clamping (brighter = geometry closer to the camera). Useful for spotting depth buffer discontinuities that cause the speckle. |
