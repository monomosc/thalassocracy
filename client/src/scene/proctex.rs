use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor, ImageAddressMode, ImageFilterMode};

#[derive(Resource, Default)]
pub struct ProcTexAssets {
    pub stone_albedo: Handle<Image>,
}

pub struct ProcTexPlugin;

impl Plugin for ProcTexPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProcTexAssets>()
            .add_systems(Startup, generate_stone_texture);
    }
}

fn generate_stone_texture(mut images: ResMut<Assets<Image>>, mut out: ResMut<ProcTexAssets>) {
    // Small, tileable grayscale noise composed into an sRGB RGBA image
    let w: u32 = 512;
    let h: u32 = 512;
    let seed: u32 = 0x00C0_FFEE;
    let data = make_improved_rock_rgba(w as usize, h as usize, seed);
    let mut image = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    });
    out.stone_albedo = images.add(image);
}

// ---------------------- noise helpers ----------------------

fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

fn hash2(ix: i32, iy: i32, seed: u32) -> u32 {
    // A simple 2D integer hash (mix) — enough for procedural noise
    let mut x = ix as u32;
    let mut y = iy as u32 ^ seed;
    x = x.wrapping_mul(0x27d4eb2d);
    y = y.wrapping_mul(0x85ebca6b);
    let mut h = x ^ y ^ (seed.rotate_left(13));
    // final avalanche
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846ca68b);
    h ^= h >> 16;
    h
}

fn grad(ix: i32, iy: i32, seed: u32) -> (f32, f32) {
    let h = hash2(ix, iy, seed);
    // Map to angle [0, 2pi)
    let a = (h as f32 / u32::MAX as f32) * std::f32::consts::TAU;
    (a.cos(), a.sin())
}

fn perlin2_periodic(x: f32, y: f32, period_x: i32, period_y: i32, seed: u32) -> f32 {
    // Periodic gradient noise over integer lattice with wrapping periods
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let u = fade(xf);
    let v = fade(yf);

    let x0 = xi.rem_euclid(period_x);
    let y0 = yi.rem_euclid(period_y);
    let x1 = (xi + 1).rem_euclid(period_x);
    let y1 = (yi + 1).rem_euclid(period_y);

    let (gx00, gy00) = grad(x0, y0, seed);
    let (gx10, gy10) = grad(x1, y0, seed);
    let (gx01, gy01) = grad(x0, y1, seed);
    let (gx11, gy11) = grad(x1, y1, seed);

    let n00 = gx00 * xf + gy00 * yf;
    let n10 = gx10 * (xf - 1.0) + gy10 * yf;
    let n01 = gx01 * xf + gy01 * (yf - 1.0);
    let n11 = gx11 * (xf - 1.0) + gy11 * (yf - 1.0);

    let nx0 = lerp(n00, n10, u);
    let nx1 = lerp(n01, n11, u);
    lerp(nx0, nx1, v)
}

fn fbm2_tileable(x: f32, y: f32, base_period: i32, octaves: i32, seed: u32) -> f32 {
    let mut f = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for o in 0..octaves {
        let p = (base_period as f32 / freq).round().max(1.0) as i32;
        let n = perlin2_periodic(x * freq, y * freq, p, p, seed ^ (o as u32).wrapping_mul(0x9E37_79B9));
        f += n * amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    f
}

fn make_improved_rock_rgba(w: usize, h: usize, seed: u32) -> Vec<u8> {
    let mut data = vec![0u8; w * h * 4];
    let base_period = w.min(h) as i32;
    // Coarse domain warp fields (periodic)
    let warp_amp = 0.7;
    let warp_freq = 1.8;
    for y in 0..h {
        for x in 0..w {
            // normalized tile space scaled to base_period for periodic sampling
            let nx0 = x as f32 / w as f32 * base_period as f32;
            let ny0 = y as f32 / h as f32 * base_period as f32;

            // Low-frequency warp (two channels)
            let wx = fbm2_tileable(nx0 * warp_freq + 11.3, ny0 * warp_freq + 7.1, base_period, 3, seed ^ 0xA1B2_C3D4);
            let wy = fbm2_tileable(nx0 * warp_freq - 5.7, ny0 * warp_freq - 9.4, base_period, 3, seed ^ 0x33EE_7731);
            let nx = nx0 + wx * warp_amp;
            let ny = ny0 + wy * warp_amp;

            // Ridge/turbulence base
            let base = fbm2_tileable(nx * 2.0, ny * 2.0, base_period, 5, seed ^ 0x9E37_79B9);
            let ridge = (base.abs()).powf(0.75);

            // Veins: periodic sin stripes with warped phase
            let stripes_u = ((nx0 * std::f32::consts::TAU * 2.5) + wx * 2.7).sin();
            let stripes_v = ((ny0 * std::f32::consts::TAU * 3.1) + wy * 2.9).sin();
            let mut veins = 1.0 - (stripes_u.abs().min(stripes_v.abs()));
            veins = veins.powf(6.0); // thin lines

            // Cavity mask: darker pits
            let cav = fbm2_tileable(nx * 3.7 + 3.0, ny * 3.7 - 2.0, base_period, 4, seed ^ 0x517C_C881);
            let cav_mask = ((-cav).max(0.0)).powf(1.6);

            // Compose luminance
            let mut lum = 0.30 + 0.55 * ridge - 0.25 * cav_mask + 0.20 * veins;
            lum = lum.clamp(0.0, 1.0);

            // Subtle hue variation between cool and warm rock tints
            let hue = fbm2_tileable(nx * 0.9 + 1.7, ny * 0.9 - 4.2, base_period, 2, seed ^ 0xDEAD_BEEF);
            let tint_t = (hue * 0.5 + 0.5).clamp(0.0, 1.0);
            let cool = (0.62, 0.66, 0.70);
            let warm = (0.58, 0.57, 0.55);
            let r = lum * (warm.0 * (1.0 - tint_t) + cool.0 * tint_t);
            let g = lum * (warm.1 * (1.0 - tint_t) + cool.1 * tint_t);
            let b = lum * (warm.2 * (1.0 - tint_t) + cool.2 * tint_t);

            // Minor speckle for grain
            let speck = perlin2_periodic(nx * 12.0, ny * 12.0, base_period, base_period, seed ^ 0x1357_9BDF);
            let s = (speck * 0.5 + 0.5) * 0.05; // +/-5%
            let rr = (r + s).clamp(0.0, 1.0);
            let gg = (g + s * 0.8).clamp(0.0, 1.0);
            let bb = (b + s * 0.6).clamp(0.0, 1.0);

            let idx = (y * w + x) * 4;
            data[idx] = (rr * 255.0) as u8;
            data[idx + 1] = (gg * 255.0) as u8;
            data[idx + 2] = (bb * 255.0) as u8;
            data[idx + 3] = 255;
        }
    }
    data
}
