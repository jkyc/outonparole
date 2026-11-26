//! All audio is synthesised at start-up (no asset files): 22.05 kHz mono WAVs fed to macroquad.
use macroquad::audio::{load_sound_from_bytes, Sound};
use std::f32::consts::TAU;

const SR: u32 = 22050;

pub struct Sounds {
    pub pistol: Sound, pub smg: Sound, pub shotgun: Sound, pub boom: Sound, pub horn: Sound,
    pub cash: Sound, pub hit: Sound, pub crash: Sound, pub siren: Sound, pub engine: Sound,
    pub radio: Vec<Sound>,
}

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.0 >> 8) as f32 / 8388608.0) - 1.0
    }
}

fn wav(s: &[f32]) -> Vec<u8> {
    let n = s.len() as u32;
    let mut o: Vec<u8> = Vec::with_capacity(44 + s.len() * 2);
    o.extend_from_slice(b"RIFF");
    o.extend_from_slice(&(36 + n * 2).to_le_bytes());
    o.extend_from_slice(b"WAVEfmt ");
    o.extend_from_slice(&16u32.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&SR.to_le_bytes());
    o.extend_from_slice(&(SR * 2).to_le_bytes());
    o.extend_from_slice(&2u16.to_le_bytes());
    o.extend_from_slice(&16u16.to_le_bytes());
    o.extend_from_slice(b"data");
    o.extend_from_slice(&(n * 2).to_le_bytes());
    for v in s { o.extend_from_slice(&((v.clamp(-1.0, 1.0) * 30000.0) as i16).to_le_bytes()); }
    o
}

fn gen(secs: f32, f: impl Fn(f32, &mut Noise) -> f32) -> Vec<u8> {
    let n = (secs * SR as f32) as usize;
    let mut nz = Noise(12345);
    let v: Vec<f32> = (0..n).map(|i| f(i as f32 / SR as f32, &mut nz)).collect();
    wav(&v)
}

fn osc(kind: u8, ph: f32) -> f32 {
    let p = ph.fract();
    match kind { 0 => if p < 0.5 { 1.0 } else { -1.0 }, 1 => 1.0 - 4.0 * (p - 0.5).abs(), _ => p * 2.0 - 1.0 }
}

/// notes in semitones above A2, i32::MIN = rest; one note per eighth.
fn track(bpm: f32, notes: &[i32], bass: &[i32], wave: u8) -> Vec<u8> {
    let step = 60.0 / bpm / 2.0;
    let total = step * notes.len() as f32;
    gen(total, |t, _| {
        let idx = ((t / step) as usize).min(notes.len() - 1);
        let lt = t - idx as f32 * step;
        let mut s = 0.0;
        if notes[idx] != i32::MIN {
            let f = 110.0 * 2f32.powf(notes[idx] as f32 / 12.0);
            s += osc(wave, f * t) * (-lt * 7.0).exp() * 0.16;
        }
        let bi = ((t / (step * 2.0)) as usize) % bass.len();
        let bl = t - (t / (step * 2.0)).floor() * step * 2.0;
        let bf = 55.0 * 2f32.powf(bass[bi] as f32 / 12.0);
        s += osc(1, bf * t) * (-bl * 3.0).exp() * 0.22;
        s
    })
}

pub async fn load() -> Sounds {
    let l = |b: Vec<u8>| async move { load_sound_from_bytes(&b).await.unwrap() };
    let pistol = l(gen(0.25, |t, nz| nz.next() * (-t * 30.0).exp() * 0.8 + (TAU * 140.0 * t).sin() * (-t * 50.0).exp() * 0.4)).await;
    let smg = l(gen(0.12, |t, nz| nz.next() * (-t * 45.0).exp() * 0.7 + (TAU * 110.0 * t).sin() * (-t * 40.0).exp() * 0.4)).await;
    let shotgun = l(gen(0.45, |t, nz| nz.next() * (-t * 12.0).exp() * 0.9 + (TAU * 70.0 * t).sin() * (-t * 20.0).exp() * 0.5)).await;
    let boom = l(gen(1.6, |t, nz| nz.next() * (-t * 2.6).exp() * 0.7 + (TAU * 45.0 * t).sin() * (-t * 2.0).exp() * 0.6)).await;
    let horn = l(gen(0.6, |t, _| {
        let env = (t * 60.0).min(1.0) * ((0.6 - t) * 30.0).clamp(0.0, 1.0);
        ((TAU * 420.0 * t).sin().signum() * 0.12 + (TAU * 530.0 * t).sin().signum() * 0.12) * env
    })).await;
    let cash = l(gen(0.3, |t, _| {
        let a = (TAU * 1320.0 * t).sin() * (-t * 14.0).exp();
        let b = if t > 0.08 { (TAU * 1760.0 * (t - 0.08)).sin() * (-(t - 0.08) * 14.0).exp() } else { 0.0 };
        (a + b) * 0.25
    })).await;
    let hit = l(gen(0.14, |t, nz| nz.next() * (-t * 40.0).exp() * 0.5 + (TAU * 90.0 * t).sin() * (-t * 30.0).exp() * 0.6)).await;
    let crash = l(gen(0.55, |t, nz| nz.next() * (-t * 9.0).exp() * 0.8 + (TAU * 60.0 * t).sin() * (-t * 8.0).exp() * 0.4)).await;
    let w = TAU * 0.5;
    let siren = l(gen(2.0, move |t, _| (TAU * 800.0 * t - 300.0 / w * (w * t).cos() * TAU).sin() * 0.2)).await;
    let engine = l(gen(0.5, |t, _| (((t * 48.0) % 1.0) * 2.0 - 1.0) * 0.18 + (TAU * 96.0 * t).sin() * 0.12 + (TAU * 144.0 * t).sin() * 0.05)).await;
    let n = i32::MIN;
    let radio = vec![
        l(track(112.0, &[12, n, 15, 19, 24, 19, 15, n, 10, n, 14, 17, 22, 17, 14, n], &[0, -5, -2, -7], 0)).await,
        l(track(96.0, &[17, 19, 21, 24, 21, 19, 17, n, 14, 16, 17, 21, 17, 16, 14, n], &[-3, 2, -5, 0], 1)).await,
        l(track(128.0, &[24, n, 24, 27, n, 22, n, 19, 24, n, 24, 31, n, 29, n, 27], &[0, 0, 3, -2], 2)).await,
    ];
    Sounds { pistol, smg, shotgun, boom, horn, cash, hit, crash, siren, engine, radio }
}
