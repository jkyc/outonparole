mod gfx;
mod sound;
mod world;

use gfx::*;
use macroquad::audio::{play_sound, play_sound_once, set_sound_volume, stop_sound, PlaySoundParams};
use macroquad::prelude::*;
use macroquad::rand::{gen_range, srand};
use sound::Sounds;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use world::*;

// ---- tuning knobs: flip a sign here if the mouse feels inverted on your platform
const LOOK_X: f32 = 1.7;
const LOOK_Y: f32 = 1.3;
const MAX_CARS: usize = 64;
const MAX_PEDS: usize = 90;

fn rf(a: f32, b: f32) -> f32 { gen_range(a, b) }
fn ri(n: usize) -> usize { gen_range(0, n) }
fn chance(p: f32) -> bool { rf(0.0, 1.0) < p }
fn angdiff(a: f32, b: f32) -> f32 { let mut d = (b - a) % TAU; if d > PI { d -= TAU; } if d < -PI { d += TAU; } d }
fn fwd(yaw: f32) -> Vec2 { vec2(yaw.sin(), yaw.cos()) }
fn v3(p: Vec2, y: f32) -> Vec3 { vec3(p.x, y, p.y) }
fn yaw_of(d: Vec2) -> f32 { d.x.atan2(d.y) }
const DIR_YAW: [f32; 4] = [FRAC_PI_2, 0.0, -FRAC_PI_2, PI];

// ---- weapons
struct Wp { name: &'static str, dmg: f32, rate: f32, spread: f32, range: f32, clip: i32, pellets: i32, reload: f32 }
const WP: [Wp; 4] = [
    Wp { name: "Fists", dmg: 14.0, rate: 0.45, spread: 0.0, range: 2.3, clip: 0, pellets: 1, reload: 0.0 },
    Wp { name: "Pistol", dmg: 34.0, rate: 0.22, spread: 0.008, range: 110.0, clip: 12, pellets: 1, reload: 1.1 },
    Wp { name: "SMG", dmg: 16.0, rate: 0.075, spread: 0.03, range: 80.0, clip: 30, pellets: 1, reload: 1.6 },
    Wp { name: "Shotgun", dmg: 11.0, rate: 0.85, spread: 0.07, range: 45.0, clip: 6, pellets: 8, reload: 2.0 },
];

// ---- cars
struct Dim { len: f32, wid: f32, bh: f32, ch: f32, cl: f32, co: f32 }
const DIMS: [Dim; 6] = [
    Dim { len: 4.6, wid: 1.9, bh: 0.75, ch: 0.62, cl: 2.3, co: -0.25 },  // sedan
    Dim { len: 3.9, wid: 1.75, bh: 0.8, ch: 0.7, cl: 2.1, co: -0.3 },    // hatch
    Dim { len: 5.2, wid: 2.0, bh: 0.95, ch: 0.65, cl: 1.7, co: 0.55 },   // pickup
    Dim { len: 5.0, wid: 2.0, bh: 1.35, ch: 0.6, cl: 3.8, co: -0.3 },    // van
    Dim { len: 4.6, wid: 1.9, bh: 0.75, ch: 0.62, cl: 2.3, co: -0.25 },  // cop
    Dim { len: 4.6, wid: 1.9, bh: 0.75, ch: 0.62, cl: 2.3, co: -0.25 },  // taxi
];
const COP: usize = 4;
const TAXI: usize = 5;
const PAINT: [(u8, u8, u8); 9] = [(192, 57, 43), (41, 128, 185), (39, 174, 96), (200, 200, 205), (70, 70, 78), (230, 126, 34), (142, 68, 173), (236, 240, 241), (22, 160, 133)];

#[derive(Clone, Copy, PartialEq)]
enum Ai { Parked, Traffic, Cop, Wreck }

#[derive(Clone)]
struct Car {
    used: bool, kind: usize, ai: Ai, pos: Vec2, yaw: f32, speed: f32, steer: f32, hp: f32, col: Color,
    dir: usize, lk: i32, maxv: f32, stuck: f32, stuck_t: f32, unloaded: bool, honk_t: f32, burn: f32, wheel: f32, braking: bool,
}
impl Car {
    fn empty() -> Car {
        Car { used: false, kind: 0, ai: Ai::Parked, pos: Vec2::ZERO, yaw: 0.0, speed: 0.0, steer: 0.0, hp: 100.0, col: WHITE, dir: 0, lk: -99,
              maxv: 12.0, stuck: 0.0, stuck_t: 0.0, unloaded: false, honk_t: 0.0, burn: 0.0, wheel: 0.0, braking: false }
    }
    fn corners(&self) -> [Vec2; 4] {
        let d = &DIMS[self.kind];
        let (s, c) = self.yaw.sin_cos();
        let mut out = [Vec2::ZERO; 4];
        for (k, &(lx, lz)) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().enumerate() {
            let (x, z) = (lx * d.wid * 0.5, lz * d.len * 0.5);
            out[k] = self.pos + vec2(x * c + z * s, -x * s + z * c);
        }
        out
    }
    /// point in the car's local frame (x right-ish, z forward)
    fn local(&self, p: Vec2) -> Vec2 {
        let (s, c) = self.yaw.sin_cos();
        let d = p - self.pos;
        vec2(d.x * c - d.y * s, d.x * s + d.y * c)
    }
}

// ---- people
#[derive(Clone, Copy, PartialEq)]
enum Ps { Walk, Idle, Flee, Cower, Chase, Dead }

#[derive(Clone)]
struct Ped {
    used: bool, cop: bool, pos: Vec2, y: f32, yaw: f32, hp: f32, st: Ps, t: f32, shirt: Color, pants: Color, skin: Color, hair: Color,
    anim: f32, cd: f32, dead_t: f32, moving: bool, life: f32,
}
impl Ped {
    fn empty() -> Ped {
        Ped { used: false, cop: false, pos: Vec2::ZERO, y: 0.0, yaw: 0.0, hp: 50.0, st: Ps::Walk, t: 0.0, shirt: WHITE, pants: WHITE, skin: WHITE, hair: WHITE,
              anim: 0.0, cd: 0.0, dead_t: 0.0, moving: false, life: 0.0 }
    }
}

struct Pl {
    pos: Vec2, y: f32, vy: f32, yaw: f32, hp: f32, armor: f32, car: Option<usize>, wp: usize,
    has: [bool; 4], clip: [i32; 4], res: [i32; 4], cd: f32, rel: f32, anim: f32, crouch: bool, aim: bool, punch: f32, moving: bool,
}

#[derive(Clone, Copy)]
enum PkK { Cash(i32), Weapon(usize, i32), Armor, Health }
struct Pk { pos: Vec2, kind: PkK, t: f32 }
struct Particle { p: Vec3, v: Vec3, life: f32, max: f32, col: Color, size: f32, grav: f32 }
struct Tracer { a: Vec3, b: Vec3, t: f32 }
struct Job { stage: u8, a: Vec2, b: Vec2, t: f32 }
struct Heli { pos: Vec3, ang: f32, rot: f32 }

#[derive(Clone, Copy)]
enum HitK { None, World, Ped(usize), Car(usize) }
struct Hit { p: Vec3, k: HitK }

#[derive(PartialEq, Clone, Copy)]
enum Crime { Shot, Assault, Murder, CarJack, RunOver, CopHit, CopKill, Destroy }

struct Game {
    world: World, snd: Sounds,
    cars: Vec<Car>, peds: Vec<Ped>, p: Pl,
    pk: Vec<Pk>, parts: Vec<Particle>, tracers: Vec<Tracer>, blood: Vec<(Vec2, f32)>,
    heli: Option<Heli>, job: Option<Job>,
    cash: i32, heat: f32, heat_t: f32, busted: f32, wasted: f32, hurt_t: f32, shake: f32, time: f32, hours: f32,
    cam_yaw: f32, cam_pitch: f32, cam_mode: usize, cpos: Vec3, vd: Vec3, mouse_idle: f32,
    started: bool, paused: bool, map: bool, radio: usize, toast: (String, f32), objective: String,
    siren_on: bool, eng_on: bool, kills: i32,
}

impl Game {
    fn new(snd: Sounds, world: World) -> Game {
        let start = world.start;
        let mut g = Game {
            world, snd, cars: vec![Car::empty(); MAX_CARS], peds: vec![Ped::empty(); MAX_PEDS],
            p: Pl { pos: start + vec2(8.0, 18.0), y: 0.25, vy: 0.0, yaw: 0.0, hp: 100.0, armor: 0.0, car: None, wp: 1,
                    has: [true, true, false, false], clip: [0, 12, 0, 0], res: [0, 48, 0, 0], cd: 0.0, rel: 0.0, anim: 0.0, crouch: false, aim: false, punch: 0.0, moving: false },
            pk: vec![], parts: vec![], tracers: vec![], blood: vec![], heli: None, job: None,
            cash: 200, heat: 0.0, heat_t: 0.0, busted: 0.0, wasted: 0.0, hurt_t: 0.0, shake: 0.0, time: 0.0, hours: 9.0,
            cam_yaw: 0.0, cam_pitch: 0.35, cam_mode: 0, cpos: Vec3::ZERO, vd: vec3(0.0, 0.0, 1.0), mouse_idle: 0.0,
            started: false, paused: false, map: false, radio: 0, toast: (String::new(), 0.0), objective: String::new(),
            siren_on: false, eng_on: false, kills: 0,
        };
        // the player's own car, parked at the kerb next to the start
        let sx = start.x + 5.6 + 7.0 - 7.0;
        let ci = g.spawn_car(0, vec2(sx + 0.0, start.y + 14.0 + 4.0), 0.0, Ai::Parked).unwrap();
        g.cars[ci].col = col(192, 57, 43);
        g.p.pos = vec2(start.x + 7.0 + 0.5, start.y + 18.0);
        for _ in 0..22 { g.spawn_parked(); }
        for _ in 0..34 { g.spawn_traffic(25.0, 700.0); }
        for _ in 0..60 { g.spawn_ped(false, 15.0, 160.0); }
        for _ in 0..26 { g.spawn_cash(); }
        for k in 0..4 {
            let h = g.world.hospital + vec2(-4.0 + k as f32 * 2.5, 12.0);
            g.pk.push(Pk { pos: h, kind: PkK::Health, t: 0.0 });
            let p = g.world.police + vec2(-4.0 + k as f32 * 2.5, 12.0);
            g.pk.push(Pk { pos: p, kind: PkK::Armor, t: 0.0 });
        }
        g.new_job();
        g.cam_calc();
        g
    }

    fn say(&mut self, s: &str) { self.toast = (s.to_string(), 2.8); }

    // ------------------------------------------------------------------ spawning
    fn spawn_car(&mut self, kind: usize, pos: Vec2, yaw: f32, ai: Ai) -> Option<usize> {
        let i = self.cars.iter().position(|c| !c.used)?;
        let (r, g, b) = PAINT[ri(PAINT.len())];
        let mut c = Car::empty();
        c.used = true; c.kind = kind; c.ai = ai; c.pos = pos; c.yaw = yaw; c.col = col(r, g, b); c.maxv = rf(9.0, 14.0);
        if kind == COP { c.col = col(240, 240, 245); }
        if kind == TAXI { c.col = col(245, 200, 30); }
        self.cars[i] = c;
        Some(i)
    }
    fn road_pos(&self) -> (Vec2, usize) {
        let k = ri((N + 1) as usize) as f32;
        let u = rf(ROAD, EXT - ROAD);
        let lane = 3.5;
        if chance(0.5) {
            let d = if chance(0.5) { 0 } else { 2 };
            (vec2(u, k * BLOCK + ROAD / 2.0 + if d == 0 { lane } else { -lane }), d)
        } else {
            let d = if chance(0.5) { 1 } else { 3 };
            (vec2(k * BLOCK + ROAD / 2.0 + if d == 1 { -lane } else { lane }, u), d)
        }
    }
    fn spawn_traffic(&mut self, dmin: f32, dmax: f32) {
        for _ in 0..20 {
            let (p, d) = self.road_pos();
            let dist = (p - self.p.pos).length();
            if dist < dmin || dist > dmax { continue; }
            let kind = [0, 0, 1, 1, 2, 3, TAXI][ri(7)];
            if let Some(i) = self.spawn_car(kind, p, DIR_YAW[d], Ai::Traffic) {
                self.cars[i].dir = d;
                self.cars[i].speed = self.cars[i].maxv;
            }
            return;
        }
    }
    fn spawn_parked(&mut self) {
        let bx = ri(N as usize) as f32;
        let bz = ri(N as usize) as f32;
        let (p, yaw) = match ri(4) {
            0 => (vec2(bx * BLOCK + ROAD - 1.4, bz * BLOCK + rf(ROAD + 5.0, BLOCK - 5.0)), 0.0),
            1 => (vec2((bx + 1.0) * BLOCK + 1.4, bz * BLOCK + rf(ROAD + 5.0, BLOCK - 5.0)), PI),
            2 => (vec2(bx * BLOCK + rf(ROAD + 5.0, BLOCK - 5.0), bz * BLOCK + ROAD - 1.4), FRAC_PI_2),
            _ => (vec2(bx * BLOCK + rf(ROAD + 5.0, BLOCK - 5.0), (bz + 1.0) * BLOCK + 1.4), -FRAC_PI_2),
        };
        if (p - self.p.pos).length() < 8.0 { return; }
        let kind = ri(4);
        self.spawn_car(kind, p, yaw, Ai::Parked);
    }
    fn walk_pos(&self) -> Vec2 {
        for _ in 0..40 {
            let p = vec2(rf(ROAD, EXT - 2.0), rf(ROAD, EXT - 2.0));
            if !self.world.solid(p.x, p.y) && !self.world.on_road(p.x, p.y) { return p; }
        }
        self.world.start + vec2(8.0, 20.0)
    }
    fn spawn_ped(&mut self, cop: bool, dmin: f32, dmax: f32) {
        let Some(i) = self.peds.iter().position(|p| !p.used) else { return };
        for _ in 0..30 {
            let p = self.walk_pos();
            let d = (p - self.p.pos).length();
            if d < dmin || d > dmax { continue; }
            let shirts = [col(211, 84, 0), col(44, 62, 80), col(127, 140, 141), col(192, 57, 43), col(39, 174, 96), col(142, 68, 173), col(52, 152, 219)];
            let skins = [col(240, 200, 160), col(224, 172, 130), col(180, 128, 92), col(120, 82, 58), col(90, 60, 42)];
            let hairs = [col(30, 24, 20), col(80, 52, 30), col(170, 130, 60), col(120, 120, 120), col(20, 20, 20)];
            let mut q = Ped::empty();
            q.used = true; q.cop = cop; q.pos = p; q.y = 0.25; q.yaw = rf(0.0, TAU); q.hp = if cop { 90.0 } else { 50.0 };
            q.st = if cop { Ps::Chase } else { Ps::Walk }; q.t = rf(1.0, 4.0);
            q.shirt = if cop { col(30, 50, 110) } else { shirts[ri(shirts.len())] };
            q.pants = if cop { col(25, 35, 70) } else { [col(52, 73, 94), col(60, 60, 60), col(100, 90, 70), col(40, 40, 90)][ri(4)] };
            q.skin = skins[ri(skins.len())]; q.hair = hairs[ri(hairs.len())];
            self.peds[i] = q;
            return;
        }
    }
    fn spawn_cash(&mut self) {
        let p = self.walk_pos();
        self.pk.push(Pk { pos: p, kind: PkK::Cash(20 * (1 + ri(8) as i32)), t: rf(0.0, 6.0) });
    }
    fn new_job(&mut self) {
        let a = self.walk_pos();
        self.job = Some(Job { stage: 0, a, b: Vec2::ZERO, t: 0.0 });
        self.objective = "Job: walk to the yellow beacon to pick up a package".into();
    }

    // ------------------------------------------------------------------ crime & damage
    fn crime(&mut self, c: Crime) {
        let add = match c {
            Crime::Shot => 0.04, Crime::Assault => 0.5, Crime::Murder => 1.2, Crime::CarJack => 0.9,
            Crime::RunOver => 0.7, Crime::CopHit => 1.4, Crime::CopKill => 2.4, Crime::Destroy => 0.6,
        };
        self.heat = (self.heat + add).min(5.0);
        self.heat_t = 10.0;
    }
    fn stars(&self) -> i32 { self.heat.ceil() as i32 }

    fn burst(&mut self, p: Vec3, n: usize, color: Color, spd: f32, life: f32, size: f32, grav: f32) {
        for _ in 0..n {
            if self.parts.len() > 700 { return; }
            self.parts.push(Particle { p, v: vec3(rf(-spd, spd), rf(0.0, spd), rf(-spd, spd)), life, max: life, col: color, size, grav });
        }
    }

    fn hurt_player(&mut self, d: f32) {
        if self.wasted > 0.0 || self.busted > 99.0 { return; }
        let ab = self.p.armor.min(d * 0.6);
        self.p.armor -= ab;
        self.p.hp -= d - ab;
        self.hurt_t = 0.3;
        self.shake = (self.shake + d * 0.05).min(0.6);
        play_sound_once(&self.snd.hit);
        if self.p.hp <= 0.0 && self.wasted <= 0.0 {
            self.wasted = 3.5;
            if let Some(ci) = self.p.car.take() { self.cars[ci].speed *= 0.5; }
        }
    }

    fn alarm(&mut self, at: Vec2, radius: f32) {
        for i in 0..self.peds.len() {
            if !self.peds[i].used || self.peds[i].st == Ps::Dead || self.peds[i].cop { continue; }
            let d = (self.peds[i].pos - at).length();
            if d < radius {
                let away = (self.peds[i].pos - at).normalize_or_zero();
                let p = &mut self.peds[i];
                p.st = if d < radius * 0.5 || chance(0.5) { Ps::Flee } else { Ps::Cower };
                p.t = rf(4.0, 8.0);
                if away != Vec2::ZERO { p.yaw = yaw_of(away); }
            }
        }
    }

    fn hurt_ped(&mut self, i: usize, dmg: f32, by_player: bool) {
        if !self.peds[i].used || self.peds[i].st == Ps::Dead { return; }
        self.peds[i].hp -= dmg;
        let (pos, cop, hp, y) = (self.peds[i].pos, self.peds[i].cop, self.peds[i].hp, self.peds[i].y);
        self.burst(v3(pos, y + 1.2), 6, col(160, 20, 20), 2.5, 0.5, 0.12, 9.0);
        if hp <= 0.0 {
            self.peds[i].st = Ps::Dead;
            self.peds[i].dead_t = 0.0;
            self.blood.push((pos, rf(0.8, 1.4)));
            if self.blood.len() > 60 { self.blood.remove(0); }
            if by_player {
                self.kills += 1;
                self.crime(if cop { Crime::CopKill } else { Crime::Murder });
            }
            if cop {
                if chance(0.6) { let w = 2 + ri(2); self.pk.push(Pk { pos, kind: PkK::Weapon(w, if w == 2 { 30 } else { 8 }), t: 0.0 }); }
                else { self.pk.push(Pk { pos, kind: PkK::Cash(100), t: 0.0 }); }
            } else if chance(0.5) {
                self.pk.push(Pk { pos, kind: PkK::Cash(20 + ri(60) as i32), t: 0.0 });
            }
            self.alarm(pos, 40.0);
        } else if by_player {
            self.crime(if cop { Crime::CopHit } else { Crime::Assault });
            if cop { self.peds[i].st = Ps::Chase; } else {
                self.peds[i].st = Ps::Flee; self.peds[i].t = 7.0;
                self.peds[i].yaw = yaw_of((pos - self.p.pos).normalize_or_zero());
            }
            self.alarm(pos, 20.0);
        }
    }

    fn damage_car(&mut self, ci: usize, d: f32, by_player: bool) {
        if !self.cars[ci].used || self.cars[ci].ai == Ai::Wreck { return; }
        self.cars[ci].hp -= d;
        if self.cars[ci].hp <= 0.0 { self.blow_up(ci, by_player); }
    }

    fn blow_up(&mut self, ci: usize, by_player: bool) {
        let pos = self.cars[ci].pos;
        let was_cop = self.cars[ci].kind == COP;
        self.cars[ci].ai = Ai::Wreck;
        self.cars[ci].hp = 0.0;
        self.cars[ci].speed = 0.0;
        if by_player || self.p.car == Some(ci) { self.crime(Crime::Destroy); }
        if was_cop && by_player { self.cash += 150; self.say("Cop car destroyed +$150"); }
        self.explode(pos, 9.0, 110.0, by_player, Some(ci));
    }

    fn explode(&mut self, pos: Vec2, r: f32, dmg: f32, by_player: bool, skip: Option<usize>) {
        play_sound_once(&self.snd.boom);
        self.shake = (self.shake + 0.5).min(0.9);
        let c = v3(pos, 0.8);
        self.burst(c, 30, col(255, 150, 30), 7.0, 0.8, 0.5, -1.0);
        self.burst(c, 24, col(60, 60, 60), 4.0, 1.8, 0.7, -2.0);
        self.burst(c, 14, col(255, 220, 90), 9.0, 0.5, 0.25, 6.0);
        for i in 0..self.peds.len() {
            if self.peds[i].used && self.peds[i].st != Ps::Dead {
                let d = (self.peds[i].pos - pos).length();
                if d < r { self.hurt_ped(i, dmg * (1.0 - d / r), by_player); }
            }
        }
        for j in 0..self.cars.len() {
            if Some(j) == skip || !self.cars[j].used || self.cars[j].ai == Ai::Wreck { continue; }
            let d = (self.cars[j].pos - pos).length();
            if d < r { self.damage_car(j, dmg * (1.0 - d / r) * 0.8, by_player); }
        }
        let d = (self.p.pos - pos).length();
        if d < r && self.wasted <= 0.0 { self.hurt_player(dmg * 0.6 * (1.0 - d / r)); }
        self.alarm(pos, 60.0);
    }

    // ------------------------------------------------------------------ rays
    fn ray(&self, o: Vec3, d: Vec3, range: f32) -> Hit {
        let mut t = 0.0;
        while t < range {
            let p = o + d * t;
            if p.y < 0.0 { return Hit { p, k: HitK::World }; }
            if self.world.height_at(p.x, p.z) > p.y { return Hit { p, k: HitK::World }; }
            for (i, q) in self.peds.iter().enumerate() {
                if q.used && q.st != Ps::Dead && (q.pos - vec2(p.x, p.z)).length() < 0.5 && p.y > q.y && p.y < q.y + 1.85 { return Hit { p, k: HitK::Ped(i) }; }
            }
            for (i, c) in self.cars.iter().enumerate() {
                if !c.used || Some(i) == self.p.car { continue; }
                let l = c.local(vec2(p.x, p.z));
                let dm = &DIMS[c.kind];
                if l.x.abs() < dm.wid / 2.0 && l.y.abs() < dm.len / 2.0 && p.y < 1.7 { return Hit { p, k: HitK::Car(i) }; }
            }
            t += 0.4;
        }
        Hit { p: o + d * range, k: HitK::None }
    }
    fn los(&self, a: Vec2, b: Vec2) -> bool {
        let d = b - a;
        let n = (d.length() / 3.0) as i32;
        for k in 1..n { let p = a + d * (k as f32 / n as f32); if self.world.height_at(p.x, p.y) > 1.8 { return false; } }
        true
    }

    // ------------------------------------------------------------------ camera
    fn cam_calc(&mut self) {
        let (pivot, dist) = match self.p.car {
            Some(ci) => (vec3(self.cars[ci].pos.x, 1.7, self.cars[ci].pos.y), [8.5, 13.0, 4.5][self.cam_mode]),
            None => (vec3(self.p.pos.x, self.p.y + if self.p.crouch { 1.1 } else { 1.6 }, self.p.pos.y), [3.6, 6.5, 2.2][self.cam_mode]),
        };
        let (cy, cp) = (self.cam_yaw, self.cam_pitch);
        let vd = vec3(cy.sin() * cp.cos(), -cp.sin(), cy.cos() * cp.cos());
        let right = vec3(-cy.cos(), 0.0, cy.sin());
        let off = if self.p.car.is_none() { right * 0.6 } else { Vec3::ZERO };
        let mut d = dist;
        let mut pos = pivot - vd * d + off;
        while d > 0.8 && (self.world.height_at(pos.x, pos.z) > pos.y || pos.y < 0.3) { d -= 0.5; pos = pivot - vd * d + off; }
        self.cpos = pos;
        self.vd = vd;
    }

    // ------------------------------------------------------------------ update
    fn update(&mut self, dt: f32) {
        self.time += dt;
        self.hours = (self.hours + dt / 30.0) % 24.0;
        self.toast.1 -= dt;
        self.hurt_t -= dt;
        self.shake *= 0.9;
        if self.wasted > 0.0 {
            self.wasted -= dt;
            if self.wasted <= 0.0 { self.respawn(true); }
        } else {
            self.mouse_look(dt);
            self.input_actions();
            self.update_player(dt);
        }
        self.update_cars(dt);
        self.update_peds(dt);
        self.update_cops(dt);
        self.update_misc(dt);
        self.cam_calc();
        self.update_audio();
    }

    fn mouse_look(&mut self, dt: f32) {
        let md = mouse_delta_position();
        self.cam_yaw -= md.x * LOOK_X;
        self.cam_pitch = (self.cam_pitch + md.y * LOOK_Y).clamp(-0.35, 1.35);
        if md.length() > 0.002 { self.mouse_idle = 0.0; } else { self.mouse_idle += dt; }
        if let Some(ci) = self.p.car {
            let c = &self.cars[ci];
            if c.speed.abs() > 4.0 && self.mouse_idle > 1.0 {
                let target = if c.speed > 0.0 { c.yaw } else { c.yaw + PI };
                self.cam_yaw += angdiff(self.cam_yaw, target) * (dt * 1.8).min(1.0);
            }
        }
    }

    fn input_actions(&mut self) {
        if is_key_pressed(KeyCode::F) { self.toggle_car(); }
        if is_key_pressed(KeyCode::V) { self.cam_mode = (self.cam_mode + 1) % 3; }
        if is_key_pressed(KeyCode::M) { self.map = !self.map; }
        if is_key_pressed(KeyCode::Tab) { self.set_radio((self.radio + 1) % 4); }
        if is_key_pressed(KeyCode::H) && self.p.car.is_some() { play_sound_once(&self.snd.horn); }
        if self.p.car.is_none() {
            if is_key_pressed(KeyCode::C) { self.p.crouch = !self.p.crouch; }
            if is_key_pressed(KeyCode::R) { self.start_reload(); }
            let wheel = mouse_wheel().1;
            if is_key_pressed(KeyCode::E) || wheel < 0.0 { self.next_weapon(1); }
            if is_key_pressed(KeyCode::Q) || wheel > 0.0 { self.next_weapon(3); }
        }
    }

    fn next_weapon(&mut self, step: usize) {
        for _ in 0..4 {
            self.p.wp = (self.p.wp + step) % 4;
            if self.p.has[self.p.wp] { break; }
        }
        self.p.rel = 0.0;
    }
    fn start_reload(&mut self) {
        let w = self.p.wp;
        if w > 0 && self.p.rel <= 0.0 && self.p.clip[w] < WP[w].clip && self.p.res[w] > 0 { self.p.rel = WP[w].reload; }
    }

    fn toggle_car(&mut self) {
        if let Some(ci) = self.p.car {
            let c = &self.cars[ci];
            let side = vec2(c.yaw.cos(), -c.yaw.sin());
            let mut out = c.pos + side * (DIMS[c.kind].wid / 2.0 + 1.0);
            if self.world.blocked(out, 0.4) { out = c.pos - side * (DIMS[c.kind].wid / 2.0 + 1.0); }
            self.p.pos = out;
            self.p.car = None;
            self.p.y = self.world.ground(out.x, out.y);
            if self.cars[ci].ai != Ai::Wreck { self.cars[ci].ai = Ai::Parked; }
            return;
        }
        let mut best = None;
        let mut bd = 4.5;
        for (i, c) in self.cars.iter().enumerate() {
            if !c.used || c.ai == Ai::Wreck { continue; }
            let d = (c.pos - self.p.pos).length();
            if d < bd { bd = d; best = Some(i); }
        }
        if let Some(ci) = best {
            let was = self.cars[ci].ai;
            if was == Ai::Traffic || was == Ai::Cop {
                self.crime(Crime::CarJack);
                self.say("Carjacked!");
                // the driver gets out and runs
                if let Some(pi) = self.peds.iter().position(|q| !q.used) {
                    let c = self.cars[ci].clone();
                    let mut q = Ped::empty();
                    q.used = true; q.pos = c.pos + vec2(c.yaw.cos(), -c.yaw.sin()) * 2.0; q.y = 0.25; q.hp = 50.0; q.st = Ps::Flee; q.t = 8.0;
                    q.yaw = rf(0.0, TAU); q.shirt = col(80, 120, 160); q.pants = col(50, 50, 60); q.skin = col(224, 172, 130); q.hair = col(40, 30, 20);
                    q.cop = was == Ai::Cop;
                    if q.cop { q.st = Ps::Chase; q.hp = 90.0; q.shirt = col(30, 50, 110); }
                    self.peds[pi] = q;
                }
            }
            self.cars[ci].ai = Ai::Parked;
            self.p.car = Some(ci);
            self.p.pos = self.cars[ci].pos;
            self.cam_yaw = self.cars[ci].yaw;
        }
    }

    fn set_radio(&mut self, n: usize) {
        if self.radio > 0 { stop_sound(&self.snd.radio[self.radio - 1]); }
        self.radio = n;
        if n > 0 { play_sound(&self.snd.radio[n - 1], PlaySoundParams { looped: true, volume: 0.35 }); self.say(&format!("Radio {}", n)); }
        else { self.say("Radio off"); }
    }

    fn update_player(&mut self, dt: f32) {
        if let Some(ci) = self.p.car {
            let thr = (is_key_down(KeyCode::W) as i32 - is_key_down(KeyCode::S) as i32) as f32;
            let st = (is_key_down(KeyCode::D) as i32 - is_key_down(KeyCode::A) as i32) as f32;
            self.drive_step(ci, thr, st, is_key_down(KeyCode::Space), dt, 36.0);
            self.p.pos = self.cars[ci].pos;
            self.p.y = 0.0;
            // run people over
            let c = self.cars[ci].clone();
            if c.speed.abs() > 4.0 {
                for i in 0..self.peds.len() {
                    if !self.peds[i].used || self.peds[i].st == Ps::Dead { continue; }
                    let l = c.local(self.peds[i].pos);
                    let dm = &DIMS[c.kind];
                    if l.x.abs() < dm.wid / 2.0 + 0.4 && l.y.abs() < dm.len / 2.0 + 0.4 {
                        self.crime(Crime::RunOver);
                        self.hurt_ped(i, c.speed.abs() * 6.0, true);
                        self.cars[ci].speed *= 0.93;
                        play_sound_once(&self.snd.hit);
                    }
                }
            }
            return;
        }
        let f = fwd(self.cam_yaw);
        let r = vec2(-self.cam_yaw.cos(), self.cam_yaw.sin());
        let mut mv = Vec2::ZERO;
        if is_key_down(KeyCode::W) { mv += f; }
        if is_key_down(KeyCode::S) { mv -= f; }
        if is_key_down(KeyCode::D) { mv += r; }
        if is_key_down(KeyCode::A) { mv -= r; }
        self.p.moving = mv.length() > 0.1;
        if self.p.moving { mv = mv.normalize(); }
        self.p.aim = is_mouse_button_down(MouseButton::Right);
        let mut sp = if self.p.crouch { 1.8 } else if is_key_down(KeyCode::LeftShift) { 7.0 } else { 3.9 };
        if self.p.aim { sp = sp.min(2.8); }
        let step = mv * sp * dt;
        for axis in 0..2 {
            let mut np = self.p.pos;
            if axis == 0 { np.x += step.x; } else { np.y += step.y; }
            if !self.world.blocked(np, 0.4) { self.p.pos = np; }
        }
        for c in &self.cars {
            if c.used { Self::push_out(c, &mut self.p.pos, 0.4); }
        }
        let g = self.world.ground(self.p.pos.x, self.p.pos.y);
        if self.p.y <= g + 0.01 && is_key_pressed(KeyCode::Space) { self.p.vy = 5.5; }
        self.p.vy -= 14.0 * dt;
        self.p.y += self.p.vy * dt;
        if self.p.y < g { self.p.y = g; self.p.vy = 0.0; }
        let firing = is_mouse_button_down(MouseButton::Left);
        if self.p.aim || firing { self.p.yaw += angdiff(self.p.yaw, self.cam_yaw) * (dt * 14.0).min(1.0); }
        else if self.p.moving { self.p.yaw += angdiff(self.p.yaw, yaw_of(mv)) * (dt * 10.0).min(1.0); }
        if self.p.moving { self.p.anim += dt * sp * 2.2; }
        self.p.cd -= dt;
        self.p.punch -= dt;
        if self.p.rel > 0.0 {
            self.p.rel -= dt;
            if self.p.rel <= 0.0 {
                let w = self.p.wp;
                let take = (WP[w].clip - self.p.clip[w]).min(self.p.res[w]);
                self.p.clip[w] += take; self.p.res[w] -= take;
            }
        }
        if firing && self.p.cd <= 0.0 && self.p.rel <= 0.0 && self.started { self.fire(); }
    }

    fn push_out(c: &Car, p: &mut Vec2, r: f32) -> bool {
        let d = &DIMS[c.kind];
        let l = c.local(*p);
        let (hx, hz) = (d.wid / 2.0 + r, d.len / 2.0 + r);
        if l.x.abs() < hx && l.y.abs() < hz {
            let (px, pz) = (hx - l.x.abs(), hz - l.y.abs());
            let mut nl = l;
            if px < pz { nl.x = hx * l.x.signum(); } else { nl.y = hz * l.y.signum(); }
            let (s, co) = c.yaw.sin_cos();
            *p = c.pos + vec2(nl.x * co + nl.y * s, -nl.x * s + nl.y * co);
            return true;
        }
        false
    }

    fn fire(&mut self) {
        let w = self.p.wp;
        let wp = &WP[w];
        let eye = vec3(self.p.pos.x, self.p.y + 1.4, self.p.pos.y);
        if w == 0 {
            self.p.cd = wp.rate;
            self.p.punch = 0.25;
            let f = fwd(self.p.yaw);
            let mut best = None;
            let mut bd = wp.range;
            for (i, q) in self.peds.iter().enumerate() {
                if !q.used || q.st == Ps::Dead { continue; }
                let d = (q.pos - self.p.pos).length();
                if d < bd && (q.pos - self.p.pos).normalize_or_zero().dot(f) > 0.3 { bd = d; best = Some(i); }
            }
            if let Some(i) = best { self.hurt_ped(i, wp.dmg, true); play_sound_once(&self.snd.hit); }
            return;
        }
        if self.p.clip[w] <= 0 { self.start_reload(); return; }
        self.p.clip[w] -= 1;
        self.p.cd = wp.rate;
        let snd = match w { 1 => &self.snd.pistol, 2 => &self.snd.smg, _ => &self.snd.shotgun };
        play_sound_once(snd);
        self.crime(Crime::Shot);
        self.alarm(self.p.pos, 35.0);
        self.cam_pitch = (self.cam_pitch - 0.01).max(-0.35);
        let f = fwd(self.p.yaw);
        let muzzle = eye + vec3(f.x * 0.7, -0.15, f.y * 0.7);
        self.burst(muzzle, 3, col(255, 220, 120), 1.5, 0.08, 0.12, 0.0);
        let start = self.cpos + self.vd * ((eye - self.cpos).length() * 0.9);
        for _ in 0..wp.pellets {
            let sp = wp.spread;
            let d = (self.vd + vec3(rf(-sp, sp), rf(-sp, sp), rf(-sp, sp))).normalize();
            let h = self.ray(start, d, wp.range);
            self.tracers.push(Tracer { a: muzzle, b: h.p, t: 0.06 });
            match h.k {
                HitK::Ped(i) => { self.hurt_ped(i, wp.dmg, true); }
                HitK::Car(i) => {
                    self.burst(h.p, 3, col(255, 210, 120), 3.0, 0.2, 0.08, 6.0);
                    self.damage_car(i, wp.dmg * 0.22, true);
                    if self.cars[i].ai == Ai::Traffic { self.cars[i].maxv *= 1.6; }
                }
                HitK::World => { self.burst(h.p, 3, col(200, 200, 200), 2.5, 0.25, 0.07, 6.0); }
                HitK::None => {}
            }
        }
    }

    // ------------------------------------------------------------------ cars
    fn drive_step(&mut self, ci: usize, thr: f32, steer: f32, hb: bool, dt: f32, maxv: f32) {
        let mut c = self.cars[ci].clone();
        let a = if thr > 0.0 { if c.speed < -0.5 { 22.0 } else { 9.5 } } else if thr < 0.0 { if c.speed > 0.5 { 22.0 } else { 6.0 } } else { 0.0 };
        c.braking = (thr < 0.0 && c.speed > 0.5) || (thr > 0.0 && c.speed < -0.5) || hb;
        c.speed += thr * a * dt;
        c.speed *= 1.0 - (if thr == 0.0 { 0.5 } else { 0.1 }) * dt;
        if hb { c.speed *= 1.0 - 2.4 * dt; }
        c.speed = c.speed.clamp(-8.0, maxv);
        let k = (c.speed / 5.0).clamp(-1.0, 1.0) / (1.0 + c.speed.abs() * 0.06);
        c.yaw -= steer * (if hb { 2.4 } else { 1.7 }) * k * dt;
        c.steer += (steer - c.steer) * (dt * 8.0).min(1.0);
        let delta = fwd(c.yaw) * c.speed * dt;
        let mut crash = 0.0;
        let tries = [c.pos + delta, vec2(c.pos.x + delta.x, c.pos.y), vec2(c.pos.x, c.pos.y + delta.y)];
        let mut moved = false;
        for (n, &np) in tries.iter().enumerate() {
            let mut t = c.clone();
            t.pos = np;
            if !t.corners().iter().any(|q| self.world.solid(q.x, q.y)) {
                c.pos = np;
                moved = true;
                if n > 0 { c.speed *= 1.0 - 1.5 * dt; }
                break;
            }
        }
        if !moved {
            if c.speed.abs() > 5.0 { crash = c.speed.abs() * 1.6; }
            c.speed *= -0.25;
        }
        c.wheel += c.speed * dt;
        let (pos, sp) = (c.pos, c.speed);
        self.cars[ci] = c;
        if crash > 0.0 {
            play_sound_once(&self.snd.crash);
            self.shake = (self.shake + crash * 0.01).min(0.6);
            self.burst(v3(pos, 0.8), 6, col(255, 210, 120), 3.0, 0.3, 0.1, 6.0);
            if self.p.car == Some(ci) { self.hurt_player(crash * 0.12); }
            self.damage_car(ci, crash, self.p.car == Some(ci));
        }
        let _ = sp;
    }

    fn ai_traffic(&mut self, i: usize, dt: f32) {
        let mut c = self.cars[i].clone();
        let dx = [1.0f32, 0.0, -1.0, 0.0];
        let dz = [0.0f32, 1.0, 0.0, -1.0];
        let d = c.dir;
        let f = vec2(dx[d], dz[d]);
        let mut sp = c.maxv;
        let mut blocked = false;
        let look = 10.0 + c.speed * 1.2;
        let ahead = |p: Vec2, r: f32| -> bool {
            let rel = p - c.pos;
            let fw = rel.dot(f);
            let sd = (rel.x * f.y - rel.y * f.x).abs();
            fw > 0.0 && fw < look && sd < r
        };
        for (j, o) in self.cars.iter().enumerate() { if j != i && o.used && ahead(o.pos, 2.4) { blocked = true; } }
        if self.p.car.is_none() && ahead(self.p.pos, 1.6) { blocked = true; }
        for q in &self.peds { if q.used && q.st != Ps::Dead && ahead(q.pos, 1.5) { blocked = true; } }
        if blocked { sp = 0.0; }
        let axis_x = d % 2 == 0;
        let u = if axis_x { c.pos.x } else { c.pos.y };
        let s = if d < 2 { 1.0 } else { -1.0 };
        let (k, dist) = if s > 0.0 {
            let k = ((u + 2.0) / BLOCK).ceil();
            (k, k * BLOCK - 2.0 - u)
        } else {
            let k = ((u - ROAD - 2.0) / BLOCK).floor();
            (k, u - (k * BLOCK + ROAD + 2.0))
        };
        if k >= 0.0 && k <= N as f32 && dist >= -0.5 && dist < 8.0 + c.speed * c.speed * 0.08 {
            let st = light(axis_x, self.time);
            if st == 2 || (st == 1 && dist > c.speed * 0.6) { sp = 0.0; }
        }
        if blocked && c.speed < 0.5 {
            c.honk_t += dt;
            if c.honk_t > 2.5 && (c.pos - self.p.pos).length() < 60.0 { c.honk_t = -4.0; play_sound_once(&self.snd.horn); }
        } else if c.honk_t > 0.0 { c.honk_t = 0.0; }
        c.braking = sp < c.speed - 1.0;
        c.speed += (sp - c.speed) * (dt * 3.0).min(1.0);
        c.pos += f * c.speed * dt;
        let lane = 3.5;
        if axis_x {
            let kz = ((c.pos.y - ROAD / 2.0) / BLOCK).round();
            let tz = kz * BLOCK + ROAD / 2.0 + if d == 0 { lane } else { -lane };
            c.pos.y += (tz - c.pos.y) * (dt * 5.0).min(1.0);
        } else {
            let kx = ((c.pos.x - ROAD / 2.0) / BLOCK).round();
            let tx = kx * BLOCK + ROAD / 2.0 + if d == 1 { -lane } else { lane };
            c.pos.x += (tx - c.pos.x) * (dt * 5.0).min(1.0);
        }
        let un = if axis_x { c.pos.x } else { c.pos.y };
        let kk = ((un - ROAD / 2.0) / BLOCK).round();
        let m = kk * BLOCK + ROAD / 2.0;
        if c.lk != kk as i32 && (u - m) * (un - m) <= 0.0 && c.speed > 0.5 {
            c.lk = kk as i32;
            let nd = (d + [0, 0, 1, 3][ri(4)]) % 4;
            let kx = ((c.pos.x - ROAD / 2.0) / BLOCK).round() as i32;
            let kz = ((c.pos.y - ROAD / 2.0) / BLOCK).round() as i32;
            let ok = match nd { 0 => kx < N, 2 => kx > 0, 1 => kz < N, _ => kz > 0 };
            if ok { c.dir = nd; }
        }
        if c.pos.x < 2.0 || c.pos.x > EXT - 2.0 || c.pos.y < 2.0 || c.pos.y > EXT - 2.0 { c.dir = (c.dir + 2) % 4; }
        c.yaw += angdiff(c.yaw, DIR_YAW[c.dir]) * (dt * 6.0).min(1.0);
        c.wheel += c.speed * dt;
        self.cars[i] = c;
    }

    fn ai_cop(&mut self, i: usize, dt: f32) {
        if self.heat <= 0.0 {
            let c = &mut self.cars[i];
            c.speed *= 1.0 - 1.5 * dt;
            return;
        }
        let tgt = self.p.pos;
        let c = self.cars[i].clone();
        let to = tgt - c.pos;
        let d = to.length();
        let ta = yaw_of(to);
        let maxv = 24.0 + self.heat * 2.5;
        let mut thr: f32;
        let mut steer: f32;
        if c.stuck > 0.0 {
            thr = -1.0; steer = (angdiff(c.yaw, ta)).signum() * 1.0;
            self.cars[i].stuck -= dt;
        } else {
            steer = -(angdiff(c.yaw, ta) * 2.0).clamp(-1.0, 1.0);
            let want = if d > 30.0 { maxv } else { (d * 0.9).clamp(0.0, maxv) };
            thr = ((want - c.speed) * 0.5).clamp(-1.0, 1.0);
            if c.speed.abs() < 1.5 && d > 7.0 {
                self.cars[i].stuck_t += dt;
                if self.cars[i].stuck_t > 1.0 { self.cars[i].stuck = 1.3; self.cars[i].stuck_t = 0.0; }
            } else { self.cars[i].stuck_t = 0.0; }
        }
        if d < 6.0 && c.speed.abs() < 1.0 { thr = 0.0; steer = 0.0; }
        self.drive_step(i, thr, steer, false, dt, maxv);
        // unload officers
        if d < 28.0 && self.stars() >= 2 && !self.cars[i].unloaded && self.p.car.is_none() {
            self.cars[i].unloaded = true;
            let cops = self.peds.iter().filter(|q| q.used && q.cop && q.st != Ps::Dead).count();
            if cops < 3 + self.stars() as usize * 2 {
                for k in 0..2 {
                    if let Some(pi) = self.peds.iter().position(|q| !q.used) {
                        let mut q = Ped::empty();
                        q.used = true; q.cop = true; q.pos = c.pos + vec2(k as f32 * 1.5 + 2.2, 1.0); q.y = 0.25; q.hp = 90.0; q.st = Ps::Chase;
                        q.shirt = col(30, 50, 110); q.pants = col(25, 35, 70); q.skin = col(224, 172, 130); q.hair = col(30, 30, 30);
                        self.peds[pi] = q;
                    }
                }
            }
        }
        // ramming hurts
        if self.p.car.is_some() && d < 4.2 && c.speed.abs() > 8.0 { self.hurt_player(2.0 * dt * c.speed.abs()); }
    }

    fn update_cars(&mut self, dt: f32) {
        for i in 0..self.cars.len() {
            if !self.cars[i].used { continue; }
            if self.p.car == Some(i) { continue; }
            match self.cars[i].ai {
                Ai::Traffic => self.ai_traffic(i, dt),
                Ai::Cop => self.ai_cop(i, dt),
                Ai::Parked | Ai::Wreck => {
                    let c = &mut self.cars[i];
                    c.speed *= 1.0 - 2.0 * dt;
                    if c.speed.abs() > 0.2 {
                        let d = fwd(c.yaw) * c.speed * dt;
                        let np = c.pos + d;
                        let mut t = c.clone();
                        t.pos = np;
                        if !t.corners().iter().any(|q| self.world.solid(q.x, q.y)) { self.cars[i].pos = np; } else { self.cars[i].speed = 0.0; }
                    }
                }
            }
            // burning
            if self.cars[i].ai != Ai::Wreck && self.cars[i].hp < 15.0 { self.cars[i].hp -= 6.0 * dt; if self.cars[i].hp <= 0.0 { let by = false; self.blow_up(i, by); } }
            if self.cars[i].hp < 40.0 || self.cars[i].ai == Ai::Wreck {
                let c = &self.cars[i];
                if chance(dt * if c.hp < 15.0 || c.ai == Ai::Wreck { 14.0 } else { 6.0 }) {
                    let p = v3(c.pos, 1.0);
                    let fire = c.hp < 15.0 || c.ai == Ai::Wreck;
                    self.burst(p, 1, if fire { col(255, 140, 30) } else { col(70, 70, 70) }, 0.8, 1.0, if fire { 0.35 } else { 0.5 }, -1.5);
                }
            }
        }
        // car vs car
        for a in 0..self.cars.len() {
            if !self.cars[a].used { continue; }
            for b in (a + 1)..self.cars.len() {
                if !self.cars[b].used { continue; }
                let d = (self.cars[a].pos - self.cars[b].pos).length();
                if d < 3.8 && d > 0.01 {
                    let n = (self.cars[a].pos - self.cars[b].pos) / d;
                    let pa = self.cars[a].ai == Ai::Parked && self.p.car != Some(a);
                    let pb = self.cars[b].ai == Ai::Parked && self.p.car != Some(b);
                    let push = (3.8 - d) * 0.5;
                    if !pa { self.cars[a].pos += n * push; }
                    if !pb { self.cars[b].pos -= n * push; }
                    let rv = (self.cars[a].speed - self.cars[b].speed).abs();
                    if rv > 6.0 {
                        let by = self.p.car == Some(a) || self.p.car == Some(b);
                        play_sound_once(&self.snd.crash);
                        let mid = (self.cars[a].pos + self.cars[b].pos) * 0.5;
                        self.burst(v3(mid, 0.8), 5, col(255, 210, 120), 3.0, 0.3, 0.1, 6.0);
                        self.damage_car(a, rv * 1.1, by);
                        self.damage_car(b, rv * 1.1, by);
                        if by { self.hurt_player(rv * 0.25); self.shake = (self.shake + 0.2).min(0.6); }
                        self.cars[a].speed *= 0.6;
                        self.cars[b].speed *= 0.6;
                    }
                }
            }
        }
        // recycle
        let pp = self.p.pos;
        for i in 0..self.cars.len() {
            if !self.cars[i].used || self.p.car == Some(i) { continue; }
            let d = (self.cars[i].pos - pp).length();
            let ai = self.cars[i].ai;
            if (ai == Ai::Traffic && d > 400.0) || (ai == Ai::Parked && d > 500.0) || (ai == Ai::Wreck && d > 120.0) || (ai == Ai::Cop && (d > 450.0 || (self.heat <= 0.0 && d > 90.0))) {
                self.cars[i].used = false;
            }
        }
        let n = self.cars.iter().filter(|c| c.used && c.ai == Ai::Traffic).count();
        if n < 34 { self.spawn_traffic(120.0, 330.0); }
        let np = self.cars.iter().filter(|c| c.used && c.ai == Ai::Parked).count();
        if np < 20 && chance(0.05) { self.spawn_parked(); }
    }

    // ------------------------------------------------------------------ people
    fn update_peds(&mut self, dt: f32) {
        let pp = self.p.pos;
        let heat = self.heat;
        for i in 0..self.peds.len() {
            if !self.peds[i].used { continue; }
            let mut p = self.peds[i].clone();
            p.anim += dt * 6.0;
            p.moving = false;
            if p.st == Ps::Dead {
                p.dead_t += dt;
                if p.dead_t > 25.0 { p.used = false; }
                self.peds[i] = p;
                continue;
            }
            let d = (pp - p.pos).length();
            if p.cop {
                p.life += dt;
                if heat <= 0.0 { p.st = Ps::Walk; if p.life > 10.0 && d > 30.0 { p.used = false; } }
                else { p.st = Ps::Chase; }
            }
            match p.st {
                Ps::Walk => {
                    p.t -= dt;
                    if p.t <= 0.0 { p.yaw += rf(-1.2, 1.2); p.t = rf(1.5, 5.0); if chance(0.25) { p.st = Ps::Idle; p.t = rf(2.0, 5.0); } }
                    self.ped_step(&mut p, fwd(p.yaw), 1.4, dt, true);
                }
                Ps::Idle => { p.t -= dt; if p.t <= 0.0 { p.st = Ps::Walk; p.t = rf(2.0, 4.0); } }
                Ps::Flee => {
                    p.t -= dt;
                    self.ped_step(&mut p, fwd(p.yaw), 6.0, dt, false);
                    if p.t <= 0.0 { p.st = Ps::Walk; p.t = 2.0; }
                }
                Ps::Cower => { p.t -= dt; if p.t <= 0.0 { p.st = Ps::Walk; p.t = 2.0; } }
                Ps::Chase => {
                    let dir = (pp - p.pos).normalize_or_zero();
                    p.yaw = yaw_of(dir);
                    let sees = d < 50.0 && self.los(p.pos, pp);
                    if d > 9.0 || !sees { self.ped_step(&mut p, dir, 5.2, dt, false); }
                    p.cd -= dt;
                    if sees && d < 40.0 && p.cd <= 0.0 && self.wasted <= 0.0 {
                        p.cd = rf(0.5, 0.9);
                        let a = vec3(p.pos.x, 1.3, p.pos.y);
                        let b = vec3(pp.x + rf(-0.6, 0.6), 1.2, pp.y + rf(-0.6, 0.6));
                        self.tracers.push(Tracer { a, b, t: 0.06 });
                        play_sound_once(&self.snd.pistol);
                        if chance(0.28) { self.hurt_player(rf(4.0, 8.0)); }
                    }
                }
                Ps::Dead => {}
            }
            p.y = self.world.ground(p.pos.x, p.pos.y);
            for c in &self.cars { if c.used { Self::push_out(c, &mut p.pos, 0.3); } }
            // aiming a gun at a civilian makes them cower
            if !p.cop && self.p.aim && self.p.wp > 0 && d < 14.0 && p.st == Ps::Walk {
                let to = (p.pos - pp).normalize_or_zero();
                if to.dot(fwd(self.p.yaw)) > 0.93 { p.st = Ps::Cower; p.t = 4.0; }
            }
            self.peds[i] = p;
        }
        // recycle civilians
        let mut civs = 0;
        for i in 0..self.peds.len() {
            if self.peds[i].used && !self.peds[i].cop {
                civs += 1;
                if (self.peds[i].pos - pp).length() > 230.0 { self.peds[i].used = false; }
            }
        }
        if civs < 60 { self.spawn_ped(false, 70.0, 190.0); }
    }

    fn ped_step(&self, p: &mut Ped, dir: Vec2, speed: f32, dt: f32, polite: bool) {
        let np = p.pos + dir * speed * dt;
        p.moving = true;
        if self.world.blocked(np, 0.35) {
            if p.st == Ps::Chase { return; }
            p.yaw += PI * 0.5 + rf(0.0, 1.0);
            return;
        }
        if polite && self.world.on_road(np.x, np.y) && !self.world.on_road(p.pos.x, p.pos.y) && chance(0.5) {
            p.yaw += PI;
            return;
        }
        p.pos = np;
    }

    // ------------------------------------------------------------------ police
    fn update_cops(&mut self, dt: f32) {
        let stars = self.stars();
        if stars > 0 {
            let want = (stars * 2).min(8) as usize;
            let have = self.cars.iter().filter(|c| c.used && c.ai == Ai::Cop).count();
            if have < want && chance(dt * 1.5) {
                for _ in 0..10 {
                    let (p, d) = self.road_pos();
                    let dist = (p - self.p.pos).length();
                    if dist > 140.0 && dist < 300.0 {
                        if let Some(i) = self.spawn_car(COP, p, DIR_YAW[d], Ai::Cop) { self.cars[i].dir = d; self.cars[i].hp = 80.0; }
                        break;
                    }
                }
            }
            if stars >= 4 && self.heli.is_none() { self.heli = Some(Heli { pos: v3(self.p.pos + vec2(80.0, 0.0), 30.0), ang: 0.0, rot: 0.0 }); }
        }
        if stars < 4 { self.heli = None; }
        // searching / losing the wanted level
        let mut seen = false;
        for c in &self.cars { if c.used && c.ai == Ai::Cop && (c.pos - self.p.pos).length() < 55.0 { seen = true; } }
        for q in &self.peds { if q.used && q.cop && q.st != Ps::Dead && (q.pos - self.p.pos).length() < 55.0 { seen = true; } }
        if let Some(h) = &mut self.heli {
            h.ang += dt * 0.5; h.rot += dt * 30.0;
            let target = vec3(self.p.pos.x + h.ang.cos() * 28.0, 28.0, self.p.pos.y + h.ang.sin() * 28.0);
            h.pos += (target - h.pos) * (dt * 1.2).min(1.0);
            if (vec2(h.pos.x, h.pos.z) - self.p.pos).length() < 70.0 { seen = true; }
        }
        if self.heat > 0.0 {
            if seen { self.heat_t = 8.0; } else { self.heat_t -= dt; if self.heat_t <= 0.0 { self.heat = (self.heat - 0.15 * dt).max(0.0); } }
        }
        // arrest
        let mut near = false;
        if self.heat > 0.0 && self.wasted <= 0.0 {
            if self.p.car.is_none() {
                for q in &self.peds { if q.used && q.cop && q.st != Ps::Dead && (q.pos - self.p.pos).length() < 2.2 { near = true; } }
                for c in &self.cars { if c.used && c.ai == Ai::Cop && (c.pos - self.p.pos).length() < 3.5 && c.speed.abs() < 3.0 { near = true; } }
            } else if let Some(ci) = self.p.car {
                if self.cars[ci].speed.abs() < 2.0 {
                    for c in &self.cars { if c.used && c.ai == Ai::Cop && (c.pos - self.p.pos).length() < 6.0 { near = true; } }
                }
            }
        }
        if near { self.busted += dt; } else { self.busted = (self.busted - dt * 0.7).max(0.0); }
        if self.busted > 2.0 { self.respawn(false); }
    }

    fn respawn(&mut self, wasted: bool) {
        self.busted = 0.0;
        self.heat = 0.0;
        self.p.hp = 100.0;
        self.p.armor = 0.0;
        self.p.car = None;
        self.p.vy = 0.0;
        if wasted {
            self.cash -= self.cash / 10;
            self.p.pos = self.world.hospital + vec2(0.0, 10.0);
            self.say("WASTED - the hospital kept 10% of your cash");
        } else {
            self.cash /= 2;
            self.p.has = [true, true, false, false];
            self.p.wp = 1; self.p.clip = [0, 12, 0, 0]; self.p.res = [0, 36, 0, 0];
            self.p.pos = self.world.police + vec2(0.0, 10.0);
            self.say("BUSTED - lost half your cash and your guns");
        }
        self.p.y = self.world.ground(self.p.pos.x, self.p.pos.y);
        for c in self.cars.iter_mut() { if c.ai == Ai::Cop { c.used = false; } }
        for q in self.peds.iter_mut() { if q.cop { q.used = false; } }
    }

    // ------------------------------------------------------------------ pickups, jobs, particles
    fn update_misc(&mut self, dt: f32) {
        for t in self.tracers.iter_mut() { t.t -= dt; }
        self.tracers.retain(|t| t.t > 0.0);
        for p in self.parts.iter_mut() { p.p += p.v * dt; p.v.y -= p.grav * dt; p.life -= dt; }
        self.parts.retain(|p| p.life > 0.0);
        let pr = if self.p.car.is_some() { 3.0 } else { 1.5 };
        let mut i = 0;
        let mut respawn_cash = 0;
        while i < self.pk.len() {
            self.pk[i].t += dt;
            if (self.pk[i].pos - self.p.pos).length() < pr && self.wasted <= 0.0 {
                let k = self.pk[i].kind;
                let mut take = true;
                match k {
                    PkK::Cash(v) => { self.cash += v; self.say(&format!("+${}", v)); play_sound_once(&self.snd.cash); respawn_cash += 1; }
                    PkK::Weapon(w, a) => { self.p.has[w] = true; self.p.res[w] += a; self.say(&format!("Picked up {}", WP[w].name)); play_sound_once(&self.snd.cash); }
                    PkK::Armor => { if self.p.armor < 100.0 { self.p.armor = 100.0; self.say("Body armor"); play_sound_once(&self.snd.cash); } else { take = false; } }
                    PkK::Health => { if self.p.hp < 100.0 { self.p.hp = 100.0; self.say("Health restored"); play_sound_once(&self.snd.cash); } else { take = false; } }
                }
                if take { self.pk.swap_remove(i); continue; }
            }
            i += 1;
        }
        for _ in 0..respawn_cash { self.spawn_cash(); }
        // job
        if let Some(job) = &mut self.job {
            job.t += dt;
            let on_foot_or_car = self.p.pos;
            match job.stage {
                0 => {
                    if (job.a - on_foot_or_car).length() < 2.5 {
                        job.stage = 1;
                        job.t = 0.0;
                        let mut b = job.a;
                        for _ in 0..30 {
                            let cand = vec2(rf(ROAD, EXT - ROAD), rf(ROAD, EXT - ROAD));
                            let d = (cand - job.a).length();
                            if d > 250.0 && d < 450.0 && !self.world.solid(cand.x, cand.y) { b = cand; break; }
                        }
                        job.b = b;
                        self.objective = "Job: deliver the package to the green beacon (use a car!)".into();
                    }
                }
                _ => {
                    let limit = (job.a - job.b).length() / 6.0 + 25.0;
                    if (job.b - on_foot_or_car).length() < 5.0 {
                        let reward = 150 + ((job.a - job.b).length() * 0.9) as i32 + ((limit - job.t).max(0.0) * 4.0) as i32;
                        self.cash += reward;
                        self.job = None;
                        self.say(&format!("Delivery done +${}", reward));
                        play_sound_once(&self.snd.cash);
                        self.new_job();
                    } else if job.t > limit {
                        self.job = None;
                        self.say("Too slow - job failed");
                        self.new_job();
                    } else {
                        self.objective = format!("Job: deliver the package to the green beacon  ({:.0}s)", limit - job.t);
                    }
                }
            }
        }
    }

    fn update_audio(&mut self) {
        let near_siren = self.cars.iter().any(|c| c.used && c.ai == Ai::Cop && self.heat > 0.0 && (c.pos - self.p.pos).length() < 120.0);
        if near_siren && !self.siren_on { play_sound(&self.snd.siren, PlaySoundParams { looped: true, volume: 0.25 }); self.siren_on = true; }
        if !near_siren && self.siren_on { stop_sound(&self.snd.siren); self.siren_on = false; }
        let driving = self.p.car.is_some() && self.wasted <= 0.0;
        if driving && !self.eng_on { play_sound(&self.snd.engine, PlaySoundParams { looped: true, volume: 0.0 }); self.eng_on = true; }
        if !driving && self.eng_on { stop_sound(&self.snd.engine); self.eng_on = false; }
        if driving {
            let sp = self.cars[self.p.car.unwrap()].speed.abs();
            set_sound_volume(&self.snd.engine, (0.06 + sp / 36.0 * 0.3).min(0.4));
        }
        if self.radio > 0 { set_sound_volume(&self.snd.radio[self.radio - 1], if driving { 0.35 } else { 0.1 }); }
    }

    // ------------------------------------------------------------------ drawing
    fn sun(&self) -> f32 { ((self.hours - 6.0) / 24.0 * TAU).sin() }
    fn sky(&self) -> Color {
        let s = self.sun();
        let day = col(135, 190, 238);
        let dusk = col(240, 150, 100);
        let night = col(8, 12, 30);
        if s > 0.25 { day } else if s > 0.0 { lerpc(dusk, day, s / 0.25) } else if s > -0.25 { lerpc(night, dusk, 1.0 + s / 0.25) } else { night }
    }

    fn draw_world(&mut self) {
        let cp = vec2(self.cpos.x, self.cpos.z);
        let vf = vec2(self.vd.x, self.vd.z).normalize_or_zero();
        draw_mesh(&self.world.ground);
        for (i, b) in self.world.blk.iter().enumerate() {
            let rel = b.centre - cp;
            let d = rel.length();
            if d > 300.0 { continue; }
            if d > 70.0 && rel.dot(vf) < -50.0 { continue; }
            draw_mesh(&self.world.plain[i]);
            for m in &self.world.fac[i] { draw_mesh(m); }
            if d < 110.0 { draw_mesh(&self.world.lines[i]); }
        }
    }

    fn lp(c: &Car, lx: f32, ly: f32, lz: f32) -> Vec3 {
        let (s, co) = c.yaw.sin_cos();
        vec3(c.pos.x + lx * co + lz * s, ly, c.pos.y - lx * s + lz * co)
    }

    fn draw_car(&self, mb: &mut Mb, c: &Car) {
        let d = &DIMS[c.kind];
        let wreck = c.ai == Ai::Wreck;
        let body = if wreck { col(35, 35, 38) } else { c.col };
        let glass = if wreck { col(15, 15, 15) } else { col(30, 45, 62) };
        let y0 = 0.32;
        mb.bx(Self::lp(c, 0.0, y0, 0.0), vec3(d.wid, d.bh, d.len), c.yaw, body);
        mb.bx(Self::lp(c, 0.0, y0 + d.bh, d.co), vec3(d.wid * 0.9, d.ch, d.cl), c.yaw, glass);
        mb.bx(Self::lp(c, 0.0, y0 + d.bh + d.ch, d.co), vec3(d.wid * 0.92, 0.07, d.cl * 0.94), c.yaw, body);
        for &(sx, sz) in &[(-1.0f32, 0.32f32), (1.0, 0.32), (-1.0, -0.32), (1.0, -0.32)] {
            mb.bx(Self::lp(c, sx * (d.wid / 2.0 - 0.02), 0.0, sz * d.len), vec3(0.26, 0.64, 0.64), c.yaw, col(24, 24, 26));
        }
        if !wreck {
            let night = self.sun() < 0.05;
            let head = if night { col(255, 250, 200) } else { col(235, 232, 190) };
            let tail = if c.braking { col(255, 40, 30) } else { col(150, 20, 20) };
            for &sx in &[-1.0f32, 1.0] {
                mb.bx(Self::lp(c, sx * d.wid * 0.32, y0 + d.bh * 0.35, d.len / 2.0), vec3(0.36, 0.2, 0.06), c.yaw, head);
                mb.bx(Self::lp(c, sx * d.wid * 0.32, y0 + d.bh * 0.35, -d.len / 2.0), vec3(0.36, 0.2, 0.06), c.yaw, tail);
            }
            if c.kind == COP {
                let f = (self.time * 8.0) as i32 % 2 == 0;
                let on = c.ai == Ai::Cop && self.heat > 0.0;
                let (a, b) = if on { if f { (col(255, 40, 40), col(40, 60, 255)) } else { (col(60, 10, 10), col(10, 15, 80)) } } else { (col(80, 20, 20), col(20, 25, 80)) };
                let top = y0 + d.bh + d.ch + 0.07;
                mb.bx(Self::lp(c, -d.wid * 0.2, top, d.co), vec3(0.4, 0.14, 0.3), c.yaw, a);
                mb.bx(Self::lp(c, d.wid * 0.2, top, d.co), vec3(0.4, 0.14, 0.3), c.yaw, b);
                mb.bx(Self::lp(c, 0.0, y0 + 0.2, 0.0), vec3(d.wid + 0.02, 0.22, d.len * 0.5), c.yaw, col(20, 30, 70));
            }
            if c.kind == TAXI { mb.bx(Self::lp(c, 0.0, y0 + d.bh + d.ch + 0.07, d.co), vec3(0.5, 0.2, 0.25), c.yaw, col(255, 250, 220)); }
            if night && (c.ai == Ai::Traffic || self.p.car.map_or(false, |_| true)) && c.speed.abs() > 0.5 {
                let fv = fwd(c.yaw);
                for &sx in &[-0.6f32, 0.6] {
                    let a = Self::lp(c, sx, 0.05, d.len / 2.0);
                    let b0 = Self::lp(c, sx - 1.5, 0.05, d.len / 2.0 + 14.0);
                    let b1 = Self::lp(c, sx + 1.5, 0.05, d.len / 2.0 + 14.0);
                    let _ = (a, b0, b1, fv);
                }
            }
        }
    }

    fn draw_ped(&self, mb: &mut Mb, q: &Ped, is_player: bool) {
        let (pos, y, yaw) = (q.pos, q.y, q.yaw);
        let (s, c) = yaw.sin_cos();
        let at = |lx: f32, ly: f32, lz: f32| vec3(pos.x + lx * c + lz * s, y + ly, pos.y - lx * s + lz * c);
        if q.st == Ps::Dead {
            mb.bx(at(0.0, 0.0, 0.0), vec3(0.5, 0.25, 0.65), yaw, q.shirt);
            mb.bx(at(0.0, 0.0, -0.65), vec3(0.4, 0.2, 0.6), yaw, q.pants);
            mb.bx(at(0.0, 0.0, 0.5), vec3(0.28, 0.26, 0.28), yaw, q.skin);
            return;
        }
        let cr = if is_player && self.p.crouch { 0.7 } else { 1.0 };
        let sw = if q.moving { (q.anim).sin() * 0.28 } else { 0.0 };
        let cower = q.st == Ps::Cower;
        let k = if cower { 0.6 } else { cr };
        mb.bx(at(-0.13, 0.0, sw), vec3(0.22, 0.82 * k, 0.26), yaw, q.pants);
        mb.bx(at(0.13, 0.0, -sw), vec3(0.22, 0.82 * k, 0.26), yaw, q.pants);
        mb.bx(at(0.0, 0.82 * k, 0.0), vec3(0.5, 0.66 * k, 0.28), yaw, q.shirt);
        mb.bx(at(0.0, 1.5 * k, 0.0), vec3(0.26, 0.28, 0.26), yaw, q.skin);
        mb.bx(at(0.0, 1.74 * k, -0.02), vec3(0.28, 0.1, 0.28), yaw, if q.cop { col(20, 30, 70) } else { q.hair });
        let armed = (is_player && self.p.wp > 0) || (q.cop && q.st == Ps::Chase);
        if cower {
            mb.bx(at(-0.3, 1.0, 0.1), vec3(0.14, 0.5, 0.14), yaw, q.skin);
            mb.bx(at(0.3, 1.0, 0.1), vec3(0.14, 0.5, 0.14), yaw, q.skin);
        } else {
            mb.bx(at(-0.33, 0.85 * k, -sw * 0.6), vec3(0.14, 0.62 * k, 0.14), yaw, q.shirt);
            if armed {
                mb.bx(at(0.33, 1.15 * k, 0.35), vec3(0.14, 0.14, 0.6), yaw, q.shirt);
                mb.bx(at(0.33, 1.2 * k, 0.8), vec3(0.08, 0.1, 0.35), yaw, col(30, 30, 34));
            } else {
                let punch = if is_player && self.p.punch > 0.0 { 0.5 } else { 0.0 };
                mb.bx(at(0.33, 0.85 * k, sw * 0.6 + punch), vec3(0.14, 0.62 * k, 0.14), yaw, q.shirt);
            }
        }
    }

    fn draw_scene(&mut self) {
        let mut mb = Mb::dynamic();
        // blood
        for (p, r) in &self.blood { mb.flat(p.x - r, p.y - r, p.x + r, p.y + r, 0.3, col(110, 14, 14)); }
        // beacons: job, police, hospital
        let beacon = |mb: &mut Mb, p: Vec2, c: Color, t: f32| { mb.bx(vec3(p.x, 0.0, p.y), vec3(1.2 + (t * 3.0).sin() * 0.2, 40.0, 1.2), t, alpha(c, 0.55)); };
        if let Some(j) = &self.job { if j.stage == 0 { beacon(&mut mb, j.a, col(255, 220, 40), self.time); } else { beacon(&mut mb, j.b, col(50, 230, 90), self.time); } }
        beacon(&mut mb, self.world.police, col(60, 100, 255), self.time);
        beacon(&mut mb, self.world.hospital, col(255, 70, 70), self.time);
        // traffic lights near the camera
        let cp = vec2(self.cpos.x, self.cpos.z);
        let ki = ((cp.x / BLOCK).floor() as i32 - 2).max(0)..=((cp.x / BLOCK).floor() as i32 + 3).min(N);
        for kx in ki {
            for kz in (((cp.y / BLOCK).floor() as i32 - 2).max(0))..=(((cp.y / BLOCK).floor() as i32 + 3).min(N)) {
                let cx = kx as f32 * BLOCK + ROAD / 2.0;
                let cz = kz as f32 * BLOCK + ROAD / 2.0;
                for &(sx, sz) in &[(1.0f32, 1.0f32), (-1.0, -1.0)] {
                    let (px, pz) = (cx + sx * (ROAD / 2.0 + 0.8), cz + sz * (ROAD / 2.0 + 0.8));
                    mb.bx(vec3(px, 0.0, pz), vec3(0.2, 5.0, 0.2), 0.0, col(60, 62, 66));
                    for (axis_x, ox, oz) in [(true, -sx * 0.4, 0.0f32), (false, 0.0, -sz * 0.4)] {
                        let st = light(axis_x, self.time);
                        let lc = match st { 0 => col(60, 255, 90), 1 => col(255, 210, 40), _ => col(255, 50, 40) };
                        mb.bx(vec3(px + ox, 4.9, pz + oz), vec3(0.45, 0.45, 0.45), 0.0, lc);
                    }
                }
            }
        }
        // pickups
        for k in &self.pk {
            if (k.pos - cp).length() > 150.0 { continue; }
            let bob = (self.time * 3.0 + k.t).sin() * 0.12 + 0.5;
            let (c, s) = match k.kind {
                PkK::Cash(_) => (col(60, 200, 100), vec3(0.5, 0.3, 0.35)),
                PkK::Weapon(..) => (col(200, 200, 210), vec3(0.8, 0.2, 0.25)),
                PkK::Armor => (col(70, 110, 230), vec3(0.5, 0.5, 0.3)),
                PkK::Health => (col(235, 60, 60), vec3(0.5, 0.5, 0.3)),
            };
            mb.bx(vec3(k.pos.x, self.world.ground(k.pos.x, k.pos.y) + bob, k.pos.y), s, self.time * 1.5, c);
        }
        for c in &self.cars { if c.used && (c.pos - cp).length() < 260.0 { self.draw_car(&mut mb, c); } }
        for q in &self.peds { if q.used && (q.pos - cp).length() < 200.0 { self.draw_ped(&mut mb, q, false); } }
        if self.p.car.is_none() && self.wasted <= 0.0 {
            let me = Ped { used: true, cop: false, pos: self.p.pos, y: self.p.y, yaw: self.p.yaw, hp: self.p.hp, st: Ps::Walk, t: 0.0,
                           shirt: col(230, 126, 34), pants: col(48, 62, 90), skin: col(224, 172, 130), hair: col(40, 28, 20), anim: self.p.anim * 1.0, cd: 0.0,
                           dead_t: 0.0, moving: self.p.moving, life: 0.0 };
            self.draw_ped(&mut mb, &me, true);
        }
        if self.wasted > 0.0 {
            let me = Ped { used: true, cop: false, pos: self.p.pos, y: self.p.y, yaw: self.p.yaw, hp: 0.0, st: Ps::Dead, t: 0.0, shirt: col(230, 126, 34),
                           pants: col(48, 62, 90), skin: col(224, 172, 130), hair: col(40, 28, 20), anim: 0.0, cd: 0.0, dead_t: 0.0, moving: false, life: 0.0 };
            self.draw_ped(&mut mb, &me, true);
        }
        if let Some(h) = &self.heli {
            let hy = h.ang + FRAC_PI_2;
            mb.bx(h.pos, vec3(2.2, 2.0, 5.0), -hy, col(30, 34, 44));
            let (s, c) = (-hy).sin_cos();
            mb.bx(h.pos + vec3(-s * 0.0 + 0.0, 0.5, 0.0) + vec3(s * -4.5, 0.0, c * -4.5), vec3(0.4, 0.5, 4.0), -hy, col(30, 34, 44));
            mb.bx(h.pos + vec3(0.0, 2.0, 0.0), vec3(9.0, 0.08, 0.35), h.rot, col(20, 20, 24));
            mb.bx(h.pos + vec3(0.0, 2.0, 0.0), vec3(0.35, 0.08, 9.0), h.rot, col(20, 20, 24));
        }
        for t in &self.tracers {
            let d = t.b - t.a;
            let n = ((d.length() / 1.2) as usize).clamp(1, 40);
            for k in 0..n { let p = t.a + d * (k as f32 / n as f32); mb.bx(p, vec3(0.07, 0.07, 0.07), 0.0, col(255, 240, 150)); }
        }
        for p in &self.parts {
            let f = (p.life / p.max).clamp(0.0, 1.0);
            mb.bx(p.p, vec3(p.size, p.size, p.size), 0.0, alpha(p.col, f.max(0.2)));
        }
        mb.flush();
    }

    fn draw(&mut self) {
        let sky = self.sky();
        clear_background(sky);
        let shake = vec3(rf(-1.0, 1.0), rf(-1.0, 1.0), rf(-1.0, 1.0)) * self.shake * 0.12;
        let cam = Camera3D {
            position: self.cpos + shake,
            target: self.cpos + shake + self.vd,
            up: vec3(0.0, 1.0, 0.0),
            fovy: 62f32.to_radians(),
            ..Default::default()
        };
        set_camera(&cam);
        self.draw_world();
        self.draw_scene();
        set_default_camera();
        // night tint
        let s = self.sun();
        let night = (1.0 - (s * 3.0 + 0.5).clamp(0.0, 1.0)) * 0.55;
        if night > 0.01 { draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.02, 0.04, 0.14, night)); }
        if self.hurt_t > 0.0 { draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.8, 0.0, 0.0, self.hurt_t * 0.8)); }
        self.hud();
    }

    // ------------------------------------------------------------------ HUD
    fn hud(&self) {
        let (w, h) = (screen_width(), screen_height());
        // crosshair
        if self.p.car.is_none() && self.wasted <= 0.0 {
            let a = if self.p.aim { 1.0 } else { 0.6 };
            let c = Color::new(1.0, 1.0, 1.0, a);
            draw_line(w / 2.0 - 8.0, h / 2.0, w / 2.0 - 3.0, h / 2.0, 2.0, c);
            draw_line(w / 2.0 + 3.0, h / 2.0, w / 2.0 + 8.0, h / 2.0, 2.0, c);
            draw_line(w / 2.0, h / 2.0 - 8.0, w / 2.0, h / 2.0 - 3.0, 2.0, c);
            draw_line(w / 2.0, h / 2.0 + 3.0, w / 2.0, h / 2.0 + 8.0, 2.0, c);
        }
        // status panel
        draw_rectangle(12.0, 12.0, 270.0, 128.0, Color::new(0.0, 0.0, 0.0, 0.55));
        draw_text(&format!("${}", self.cash), 24.0, 42.0, 34.0, WHITE);
        draw_rectangle(24.0, 54.0, 220.0, 12.0, col(60, 60, 60));
        draw_rectangle(24.0, 54.0, 2.2 * self.p.hp.max(0.0), 12.0, if self.p.hp > 35.0 { col(46, 204, 113) } else { col(231, 76, 60) });
        if self.p.armor > 0.0 { draw_rectangle(24.0, 69.0, 2.2 * self.p.armor, 6.0, col(70, 130, 240)); }
        let st = self.stars();
        let mut stars = String::new();
        for k in 1..=5 { stars.push_str(if k <= st { "* " } else { "- " }); }
        draw_text(&format!("WANTED {}", stars), 24.0, 100.0, 26.0, if st > 0 { col(255, 211, 77) } else { col(170, 170, 170) });
        if let Some(ci) = self.p.car {
            let c = &self.cars[ci];
            draw_text(&format!("{:.0} km/h   car {:.0}%", c.speed.abs() * 3.6, c.hp.max(0.0)), 24.0, 128.0, 24.0, WHITE);
        } else {
            let wp = self.p.wp;
            let ammo = if wp == 0 { "-".to_string() } else if self.p.rel > 0.0 { "reloading".to_string() } else { format!("{}/{}", self.p.clip[wp], self.p.res[wp]) };
            draw_text(&format!("{}  {}", WP[wp].name, ammo), 24.0, 128.0, 24.0, WHITE);
        }
        if !self.objective.is_empty() { draw_text(&self.objective, 24.0, h - 24.0, 24.0, col(255, 235, 150)); }
        if self.toast.1 > 0.0 {
            let dm = measure_text(&self.toast.0, None, 30, 1.0);
            draw_text(&self.toast.0, (w - dm.width) / 2.0, h - 90.0, 30.0, WHITE);
        }
        let hh = (self.hours as i32, ((self.hours % 1.0) * 60.0) as i32);
        draw_text(&format!("{:02}:{:02}", hh.0, hh.1), w - 90.0, h - 14.0, 22.0, col(220, 220, 220));
        if self.busted > 0.1 { draw_rectangle(w / 2.0 - 100.0, h / 2.0 + 60.0, 200.0 * (self.busted / 2.0).min(1.0), 8.0, col(70, 130, 255)); }
        if self.wasted > 0.0 { let t = "WASTED"; let dm = measure_text(t, None, 100, 1.0); draw_text(t, (w - dm.width) / 2.0, h / 2.0, 100.0, col(210, 40, 40)); }
        self.radar();
    }

    fn radar(&self) {
        let (w, h) = (screen_width(), screen_height());
        let full = self.map;
        let size = if full { w.min(h) * 0.85 } else { 170.0 };
        let (x0, y0) = if full { ((w - size) / 2.0, (h - size) / 2.0) } else { (w - size - 14.0, 14.0) };
        let (scale, origin) = if full { (size / EXT, vec2(EXT / 2.0, EXT / 2.0)) } else { (1.25, self.p.pos) };
        let ctr = vec2(x0 + size / 2.0, y0 + size / 2.0);
        // screen = ctr - (world - origin) * scale: matches the 3D view when the camera looks along +z
        let to = |p: Vec2| vec2(ctr.x - (p.x - origin.x) * scale, ctr.y - (p.y - origin.y) * scale);
        draw_rectangle(x0, y0, size, size, col(52, 54, 58));
        let clip = |a: Vec2, b: Vec2, c: Color| {
            let (sa, sb) = (to(a), to(b));
            let (lx, hx) = (sa.x.min(sb.x).max(x0), sa.x.max(sb.x).min(x0 + size));
            let (ly, hy) = (sa.y.min(sb.y).max(y0), sa.y.max(sb.y).min(y0 + size));
            if hx > lx && hy > ly { draw_rectangle(lx, ly, hx - lx, hy - ly, c); }
        };
        for b in &self.world.blk {
            if !full && (b.centre - origin).length() > 200.0 { continue; }
            let o = b.centre - vec2(BLOCK / 2.0, BLOCK / 2.0);
            clip(o + vec2(ROAD, ROAD), o + vec2(BLOCK, BLOCK), if b.park { col(46, 90, 44) } else { col(86, 88, 92) });
        }
        for k in &self.pk { if let PkK::Cash(_) = k.kind { if full || (k.pos - origin).length() < 100.0 { let p = to(k.pos); if p.x > x0 && p.x < x0 + size && p.y > y0 && p.y < y0 + size { draw_rectangle(p.x - 1.5, p.y - 1.5, 3.0, 3.0, col(60, 220, 110)); } } } }
        let dot = |p: Vec2, r: f32, c: Color| { let s = to(p); if s.x > x0 && s.x < x0 + size && s.y > y0 && s.y < y0 + size { draw_circle(s.x, s.y, r, c); } };
        for c in &self.cars { if c.used && c.ai == Ai::Cop && self.heat > 0.0 { dot(c.pos, 3.5, if (self.time * 4.0) as i32 % 2 == 0 { col(255, 60, 60) } else { col(70, 90, 255) }); } }
        for q in &self.peds { if q.used && q.cop && q.st != Ps::Dead { dot(q.pos, 2.5, col(80, 120, 255)); } }
        dot(self.world.police, 4.0, col(70, 110, 255));
        dot(self.world.hospital, 4.0, col(255, 80, 80));
        if let Some(j) = &self.job { dot(if j.stage == 0 { j.a } else { j.b }, 4.5, if j.stage == 0 { col(255, 220, 40) } else { col(50, 230, 90) }); }
        let me = to(self.p.pos);
        let f = fwd(self.p.car.map_or(self.p.yaw, |ci| self.cars[ci].yaw));
        let tip = me + vec2(-f.x, -f.y) * 8.0;
        draw_circle(me.x, me.y, 4.0, WHITE);
        draw_line(me.x, me.y, tip.x, tip.y, 2.0, WHITE);
        draw_rectangle_lines(x0, y0, size, size, 2.0, Color::new(1.0, 1.0, 1.0, 0.5));
    }
}

fn conf() -> Conf {
    Conf { window_title: "Out on Parole".to_owned(), window_width: 1280, window_height: 720, sample_count: 4, ..Default::default() }
}

#[macroquad::main(conf)]
async fn main() {
    srand(macroquad::miniquad::date::now() as u64);
    let world = World::new();
    let snd = sound::load().await;
    let mut g = Game::new(snd, world);
    loop {
        let dt = get_frame_time().min(0.05);
        if !g.started {
            clear_background(col(10, 12, 16));
            g.draw();
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.75));
            let t = "OUT ON PAROLE";
            let m = measure_text(t, None, 90, 1.0);
            draw_text(t, (screen_width() - m.width) / 2.0, screen_height() * 0.38, 90.0, WHITE);
            let lines = ["Click to start", "", "WASD move / drive   mouse aim   LMB shoot   RMB aim", "F get in/out   Shift sprint   Space jump / handbrake   C crouch",
                         "R reload   Q/E weapons   H horn   M map   V camera   Tab radio   Esc pause"];
            for (i, l) in lines.iter().enumerate() {
                let m = measure_text(l, None, 26, 1.0);
                draw_text(l, (screen_width() - m.width) / 2.0, screen_height() * 0.5 + i as f32 * 34.0, 26.0, col(220, 220, 220));
            }
            if is_mouse_button_pressed(MouseButton::Left) { g.started = true; set_cursor_grab(true); show_mouse(false); }
        } else if g.paused {
            g.draw();
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.6));
            let t = "PAUSED - click to continue";
            let m = measure_text(t, None, 50, 1.0);
            draw_text(t, (screen_width() - m.width) / 2.0, screen_height() / 2.0, 50.0, WHITE);
            if is_mouse_button_pressed(MouseButton::Left) { g.paused = false; set_cursor_grab(true); show_mouse(false); }
        } else {
            if is_key_pressed(KeyCode::Escape) { g.paused = true; set_cursor_grab(false); show_mouse(true); }
            g.update(dt);
            g.draw();
        }
        next_frame().await;
    }
}
