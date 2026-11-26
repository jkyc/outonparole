//! Tiny mesh builder: every "model" in the game is boxes, so shading is baked into vertex colours.
use macroquad::prelude::*;

pub fn col(r: u8, g: u8, b: u8) -> Color { Color::from_rgba(r, g, b, 255) }
pub fn shade(c: Color, f: f32) -> Color { Color::new((c.r * f).min(1.0), (c.g * f).min(1.0), (c.b * f).min(1.0), c.a) }
pub fn alpha(c: Color, a: f32) -> Color { Color::new(c.r, c.g, c.b, a) }
pub fn lerpc(a: Color, b: Color, t: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}
pub fn hash(a: i32, b: i32) -> f32 {
    let mut h = (a as u32).wrapping_mul(374761393) ^ (b as u32).wrapping_mul(668265263).wrapping_add(0x9e3779b9);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) as f32) / 4294967296.0
}

pub struct Mb {
    pub v: Vec<Vertex>,
    pub i: Vec<u16>,
    pub tex: Option<Texture2D>,
    pub auto: bool, // flush itself when nearly full (per-frame dynamic batch)
}

impl Mb {
    pub fn new() -> Mb { Mb { v: Vec::new(), i: Vec::new(), tex: None, auto: false } }
    pub fn dynamic() -> Mb { Mb { v: Vec::new(), i: Vec::new(), tex: None, auto: true } }

    pub fn quad(&mut self, p: [Vec3; 4], uv: [Vec2; 4], c: Color) {
        if self.auto && self.v.len() > 64000 { self.flush(); }
        let b = self.v.len() as u16;
        for k in 0..4 { self.v.push(Vertex::new(p[k].x, p[k].y, p[k].z, uv[k].x, uv[k].y, c)); }
        self.i.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    /// Box. `c` is the centre of its base, `s` the size (x width, y height, z length), rotated by `yaw` about Y.
    pub fn bx(&mut self, c: Vec3, s: Vec3, yaw: f32, color: Color) {
        let (sn, cs) = yaw.sin_cos();
        let (hx, hz) = (s.x / 2.0, s.z / 2.0);
        let rot = |lx: f32, ly: f32, lz: f32| -> Vec3 { vec3(c.x + lx * cs + lz * sn, c.y + ly, c.z - lx * sn + lz * cs) };
        let nrot = |x: f32, z: f32| -> Vec3 { vec3(x * cs + z * sn, 0.0, -x * sn + z * cs) };
        let l = vec3(0.4, 0.8, 0.35).normalize();
        let f = |n: Vec3| -> f32 { 0.55 + 0.5 * n.dot(l).max(0.0) };
        let uv = [vec2(0., 0.), vec2(1., 0.), vec2(1., 1.), vec2(0., 1.)];
        let y1 = s.y;
        self.quad([rot(-hx, 0., hz), rot(hx, 0., hz), rot(hx, y1, hz), rot(-hx, y1, hz)], uv, shade(color, f(nrot(0., 1.))));
        self.quad([rot(hx, 0., -hz), rot(-hx, 0., -hz), rot(-hx, y1, -hz), rot(hx, y1, -hz)], uv, shade(color, f(nrot(0., -1.))));
        self.quad([rot(hx, 0., hz), rot(hx, 0., -hz), rot(hx, y1, -hz), rot(hx, y1, hz)], uv, shade(color, f(nrot(1., 0.))));
        self.quad([rot(-hx, 0., -hz), rot(-hx, 0., hz), rot(-hx, y1, hz), rot(-hx, y1, -hz)], uv, shade(color, f(nrot(-1., 0.))));
        self.quad([rot(-hx, y1, hz), rot(hx, y1, hz), rot(hx, y1, -hz), rot(-hx, y1, -hz)], uv, shade(color, f(vec3(0., 1., 0.))));
    }

    /// Flat quad lying on the ground (road markings, shadows, decals).
    pub fn flat(&mut self, x0: f32, z0: f32, x1: f32, z1: f32, y: f32, color: Color) {
        let uv = [vec2(0., 0.), vec2(1., 0.), vec2(1., 1.), vec2(0., 1.)];
        self.quad([vec3(x0, y, z0), vec3(x1, y, z0), vec3(x1, y, z1), vec3(x0, y, z1)], uv, color);
    }

    pub fn flush(&mut self) {
        if self.i.is_empty() { return; }
        let m = Mesh { vertices: std::mem::take(&mut self.v), indices: std::mem::take(&mut self.i), texture: self.tex.clone() };
        draw_mesh(&m);
    }

    pub fn mesh(self) -> Mesh { Mesh { vertices: self.v, indices: self.i, texture: self.tex } }
}

/// Window texture for a building face: `floors` rows of 6 windows. White wall so the vertex colour tints it.
pub fn facade(floors: u32, variant: u32) -> Texture2D {
    let w = 96u16;
    let h = (floors * 16) as u16;
    let mut img = Image::gen_image_color(w, h, WHITE);
    for fl in 0..floors {
        for c in 0..6u32 {
            let r = hash((fl * 13 + variant * 101) as i32, c as i32 + 7);
            let color = if r > 0.82 { col(250, 238, 190) } else if r > 0.4 { col(78, 104, 138) } else { col(46, 60, 86) };
            for y in 0..9u32 {
                for x in 0..9u32 { img.set_pixel(c * 16 + 4 + x, fl * 16 + 4 + y, color); }
            }
        }
    }
    let t = Texture2D::from_image(&img);
    t.set_filter(FilterMode::Nearest);
    t
}
