//! The city: a grid of blocks separated by roads. Built once into meshes; collision is lot rectangles.
use crate::gfx::*;
use macroquad::prelude::*;
use std::collections::HashMap;

pub const BLOCK: f32 = 64.0;
pub const ROAD: f32 = 14.0;
pub const N: i32 = 12;
pub const EXT: f32 = BLOCK * N as f32 + ROAD; // world is [0, EXT] on x and z
pub const LOT: f32 = 21.0;
const LOT0: f32 = ROAD + 3.0;

pub struct Lot { pub min: Vec2, pub max: Vec2, pub h: f32 }
pub struct Blk { pub park: bool, pub lots: Vec<Lot>, pub centre: Vec2 }

pub struct World {
    pub blk: Vec<Blk>,
    pub plain: Vec<Mesh>,
    pub lines: Vec<Mesh>,
    pub fac: Vec<Vec<Mesh>>,
    pub ground: Mesh,
    pub police: Vec2,
    pub hospital: Vec2,
    pub start: Vec2,
}

/// 0 green, 1 yellow, 2 red for traffic travelling along x (axis_x) or z.
pub fn light(axis_x: bool, t: f32) -> u8 {
    let ph = (t + if axis_x { 0.0 } else { 10.0 }) % 20.0;
    if ph < 7.0 { 0 } else if ph < 9.0 { 1 } else { 2 }
}

impl World {
    pub fn new() -> World {
        let mut texs: HashMap<u32, Texture2D> = HashMap::new();
        let walls = [col(176, 120, 100), col(190, 180, 160), col(150, 160, 170), col(205, 205, 210), col(140, 112, 100), col(120, 138, 156)];
        let (mut blk, mut plain, mut lines, mut fac) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for bx in 0..N {
            for bz in 0..N {
                let ox = bx as f32 * BLOCK;
                let oz = bz as f32 * BLOCK;
                let park = hash(bx, bz) < 0.12 && !(bx == N / 2 && bz == N / 2);
                let inner = BLOCK - ROAD;
                let mut lots = Vec::new();
                let mut pm = Mb::new();
                let mut fm: Vec<Mesh> = Vec::new();
                let mut lm = Mb::new();
                pm.bx(vec3(ox + ROAD + inner / 2.0, 0.0, oz + ROAD + inner / 2.0), vec3(inner, 0.25, inner), 0.0,
                      if park { col(74, 128, 62) } else { col(152, 152, 146) });
                if park {
                    for t in 0..9 {
                        let tx = ox + ROAD + 6.0 + hash(bx * 9 + t, bz) * (inner - 12.0);
                        let tz = oz + ROAD + 6.0 + hash(bx, bz * 9 + t + 40) * (inner - 12.0);
                        pm.bx(vec3(tx, 0.25, tz), vec3(0.45, 2.6, 0.45), 0.0, col(96, 70, 44));
                        pm.bx(vec3(tx, 2.2, tz), vec3(3.4, 2.4, 3.4), 0.0, col(48, 112, 52));
                        pm.bx(vec3(tx, 4.4, tz), vec3(2.2, 1.8, 2.2), 0.0, col(58, 128, 60));
                    }
                } else {
                    for i in 0..2 {
                        for j in 0..2 {
                            let x0 = ox + LOT0 + i as f32 * (LOT + 2.0);
                            let z0 = oz + LOT0 + j as f32 * (LOT + 2.0);
                            let (x1, z1) = (x0 + LOT, z0 + LOT);
                            let hh = hash(bx * 7 + i, bz * 5 + j + 100);
                            let dc = (((bx as f32 + 0.5) / N as f32 - 0.5).powi(2) + ((bz as f32 + 0.5) / N as f32 - 0.5).powi(2)).sqrt();
                            let floors = ((2.0 + hh * 4.0 + (0.62 - dc).max(0.0) * 20.0 * hash(bx + j, bz + i + 50)).round() as u32).clamp(2, 14);
                            let h = floors as f32 * 3.6;
                            lots.push(Lot { min: vec2(x0, z0), max: vec2(x1, z1), h });
                            let wall = walls[(hash(bx + i * 3, bz + j * 3 + 9) * walls.len() as f32) as usize % walls.len()];
                            let variant = ((hh * 3.0) as u32).min(2);
                            let tex = texs.entry(floors * 10 + variant).or_insert_with(|| facade(floors, variant)).clone();
                            let mut fb = Mb::new();
                            fb.tex = Some(tex);
                            let uv = [vec2(0., 1.), vec2(1., 1.), vec2(1., 0.), vec2(0., 0.)];
                            let y0 = 0.25;
                            fb.quad([vec3(x0, y0, z1), vec3(x1, y0, z1), vec3(x1, h, z1), vec3(x0, h, z1)], uv, shade(wall, 0.85));
                            fb.quad([vec3(x1, y0, z0), vec3(x0, y0, z0), vec3(x0, h, z0), vec3(x1, h, z0)], uv, shade(wall, 0.62));
                            fb.quad([vec3(x1, y0, z1), vec3(x1, y0, z0), vec3(x1, h, z0), vec3(x1, h, z1)], uv, shade(wall, 0.74));
                            fb.quad([vec3(x0, y0, z0), vec3(x0, y0, z1), vec3(x0, h, z1), vec3(x0, h, z0)], uv, shade(wall, 0.68));
                            fm.push(fb.mesh());
                            pm.flat(x0, z0, x1, z1, h + 0.02, col(92, 92, 98));
                            pm.bx(vec3((x0 + x1) / 2.0 + (hh - 0.5) * 8.0, h, (z0 + z1) / 2.0 + (hash(bx, bz + i + j) - 0.5) * 8.0),
                                  vec3(3.0, 1.3, 3.0), 0.0, col(150, 150, 156));
                        }
                    }
                }
                // street lamps on two corners
                for &(lx, lz) in &[(ox + ROAD + 1.0, oz + ROAD + 1.0), (ox + BLOCK - 1.0, oz + BLOCK - 1.0)] {
                    pm.bx(vec3(lx, 0.25, lz), vec3(0.18, 5.5, 0.18), 0.0, col(70, 72, 76));
                    pm.bx(vec3(lx, 5.5, lz), vec3(1.0, 0.22, 0.4), 0.0, col(240, 230, 170));
                }
                // lane markings: dashed centre lines + solid edge lines of the roads on this block's west and north sides
                let cx = ox + ROAD / 2.0;
                let cz = oz + ROAD / 2.0;
                let mut z = oz + ROAD + 3.0;
                while z < oz + BLOCK - 3.0 { lm.flat(cx - 0.15, z, cx + 0.15, z + 4.0, 0.08, col(225, 195, 70)); z += 8.0; }
                let mut x = ox + ROAD + 3.0;
                while x < ox + BLOCK - 3.0 { lm.flat(x, cz - 0.15, x + 4.0, cz + 0.15, 0.08, col(225, 195, 70)); x += 8.0; }
                for &e in &[0.4f32, ROAD - 0.6] {
                    lm.flat(ox + e, oz + ROAD, ox + e + 0.2, oz + BLOCK, 0.08, col(205, 205, 205));
                    lm.flat(ox + ROAD, oz + e, ox + BLOCK, oz + e + 0.2, 0.08, col(205, 205, 205));
                }
                blk.push(Blk { park, lots, centre: vec2(ox + BLOCK / 2.0, oz + BLOCK / 2.0) });
                plain.push(pm.mesh());
                lines.push(lm.mesh());
                fac.push(fm);
            }
        }
        let mut g = Mb::new();
        g.flat(-500.0, -500.0, EXT + 500.0, EXT + 500.0, -0.1, col(84, 112, 74));
        g.flat(0.0, 0.0, EXT, EXT, 0.0, col(58, 60, 64));
        World {
            blk, plain, lines, fac, ground: g.mesh(),
            police: vec2(4.0 * BLOCK + ROAD / 2.0, 7.0 * BLOCK + ROAD / 2.0),
            hospital: vec2(8.0 * BLOCK + ROAD / 2.0, 3.0 * BLOCK + ROAD / 2.0),
            start: vec2(6.0 * BLOCK + ROAD / 2.0, 6.0 * BLOCK + ROAD / 2.0),
        }
    }

    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        if x < 0.0 || z < 0.0 || x >= EXT || z >= EXT { return 99.0; }
        let (bx, bz) = ((x / BLOCK) as i32, (z / BLOCK) as i32);
        if bx >= N || bz >= N { return 0.0; }
        for l in &self.blk[(bx * N + bz) as usize].lots {
            if x >= l.min.x && x < l.max.x && z >= l.min.y && z < l.max.y { return l.h; }
        }
        0.0
    }
    pub fn solid(&self, x: f32, z: f32) -> bool { self.height_at(x, z) > 0.0 }
    /// Is a disc of radius r at p inside a building (or outside the world)?
    pub fn blocked(&self, p: Vec2, r: f32) -> bool {
        self.solid(p.x, p.y) || self.solid(p.x + r, p.y) || self.solid(p.x - r, p.y) || self.solid(p.x, p.y + r) || self.solid(p.x, p.y - r)
    }
    pub fn on_road(&self, x: f32, z: f32) -> bool {
        if x < 0.0 || z < 0.0 || x >= EXT || z >= EXT { return false; }
        let (lx, lz) = (x % BLOCK, z % BLOCK);
        let (bx, bz) = ((x / BLOCK) as i32, (z / BLOCK) as i32);
        lx < ROAD || lz < ROAD || bx >= N || bz >= N
    }
    pub fn ground(&self, x: f32, z: f32) -> f32 { if self.on_road(x, z) { 0.0 } else { 0.25 } }
}
