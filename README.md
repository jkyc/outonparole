# Out on Parole (Rust rewrite)

A Rust + macroquad rewrite of the C/raylib game. 3D third-person city, cars with traffic lights, pedestrians, wanted level
with police cars / officers / helicopter, four weapons, pickups, delivery jobs, day/night, synthesised sound and radio.
All models are placeholder boxes for now. Runs natively and in the browser (WebAssembly).

**Status: written without a compiler.** The sandbox it was written in has no Rust toolchain, so this has never been built.
Expect a handful of compile errors on the first `cargo build`; they should be small.

## Run natively
    cargo run --release

## Run in the browser (HTML5)
    rustup target add wasm32-unknown-unknown
    ./build_web.sh                       # builds dist-web/ and downloads macroquad's JS glue
    python3 -m http.server -d dist-web 8000      # open http://localhost:8000

## Controls
WASD move/drive, mouse look, LMB shoot, RMB aim, F get in/out, Shift sprint, Space jump/handbrake, C crouch,
R reload, Q/E or wheel weapons, H horn, M map, V camera distance, Tab radio, Esc pause.

## Things to check first
- Mouse look direction: if it is inverted, flip the sign of `LOOK_X` / `LOOK_Y` at the top of `src/main.rs`.
- `Vertex::new(x, y, z, u, v, color)` in `src/gfx.rs` is the one macroquad API that changed between 0.4.x releases;
  if it fails to compile, use `Vertex::new2(vec3(x, y, z), vec2(u, v), color)` instead.
- Far-away lane markings are only drawn within 110 m, to avoid depth flicker.

## Layout
- `src/gfx.rs` box-mesh builder and window textures, `src/world.rs` city and traffic lights,
  `src/sound.rs` synthesised audio, `src/main.rs` game logic, drawing and HUD.

## Not ported yet
Gamepad, character skeletons/animation clips, real car and gun models, shops and robbery, missions, ragdolls,
the original's lighting and shadows.
