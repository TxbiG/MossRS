# MossRS Cheatsheet

> Quick reference for the idiomatic Rust public API. The companion [API Reference](API_Reference.md) defines the complete feature map and all exported native-to-language projections. The public calls below are the target design, not a claim that the current binding has implemented every member yet.

## Language design at a glance

The public surface is a safe Rust crate. Use snake_case functions, PascalCase types, ownership and borrowing, RAII/Drop, `Result<T, MossError>` for recoverable failures, and typed options instead of raw descriptor pointers.

## Subsystem map

| Moss subsystem | Public module / facade | Responsibility |
|---|---|---|
| Core | `moss::core` | Foundational value types, logging, memory, profiler, jobs, streams and utilities. |
| Platform | `moss::platform` | Windows/surfaces, monitors, events, input, gamepads, sensors, cameras, storage and dialogs. |
| GPU | `moss::gpu` | Devices, buffers, textures, samplers, shaders, pipelines, command recording, swapchains and synchronization. |
| Renderer | `moss::renderer` | Frame lifecycle, materials, resource sets, framebuffers, meshes, models, fonts, sprites, cameras and debug draw. |
| Audio | `moss::audio` | Audio engine, sources, streams, channels, effects, listeners, acoustics, speaker and microphone capture. |
| GUI | `moss::gui` | Contexts, windows, containers, widgets, text/scalar input and drawing. |
| Physics | `moss::physics` | Bodies, shapes, filters, collisions, constraints, characters, vehicles, soft bodies and navigation integration. |
| Network | `moss::network` | Hosts, peers, packets, transports, HTTP/TLS, WebSocket and WebRTC where enabled. |
| XR | `moss::xr` | Runtime/session lifecycle, views, actions, tracking, haptics, hand/body/face tracking and composition. |

## 1. Import Moss

```rust
use moss::{
    audio, gpu, platform, renderer,
    platform::WindowOptions,
    Result,
};
use std::time::Duration;
```

## 2. Create a window and pump events

```rust
fn main() -> moss::Result<()> {
    let mut app = moss::Application::new()?;
    let mut window = app.platform().create_window(WindowOptions {
        title: "Moss application".into(),
        width: 1280,
        height: 720,
        ..Default::default()
    })?;

    while !window.should_close() {
        app.platform_mut().poll_events()?;
    }
    // Window and Application release native resources through Drop.
    Ok(())
}
```

## 3. Create and upload a GPU buffer

```rust
let device = gpu::create_device(&gpu::GpuDeviceDesc::default())?;
let mut buffer = device.create_buffer(gpu::BufferDesc {
    size: vertices.len(),
    usage: gpu::BufferUsage::VERTEX,
    ..Default::default()
})?;
buffer.upload(&vertices, 0)?;
// `buffer` drops before `device` because it was created after it.
```

## 4. Render a frame

```rust
let mut frame = renderer.begin_frame()?;
frame.set_camera(&camera);
frame.draw_sprite(&sprite);
frame.draw_text("Hello Moss", position, &text_style);
frame.finish()?;
```

## 5. Play audio

```rust
audio::initialize()?;
let mut source = audio::load_wav("assets/theme.wav")?;
source.play()?;
audio::update(delta_time);
// Drop the source before shutting down the audio subsystem.
drop(source);
audio::shutdown();
```

## 6. Build a GUI

```rust
let mut ui = gui::Context::new()?;
ui.window("Inspector", |ui| {
    ui.text("Player");
    let enabled = ui.checkbox("Enabled", current_value);
});
```

## 7. Simulate physics

```rust
let mut physics = physics::System::new(settings)?;
let body = physics.bodies().create(physics::BodyDesc {
    shape: physics::Shape::box_shape(size),
    motion: physics::MotionType::Dynamic,
    ..Default::default()
})?;
physics.step(delta_time)?;
let contacts = physics.query().contacts(body.id())?;
```

## 8. Networking

```rust
let mut host = network::Host::bind(host_options)?;
while let Some(event) = host.service(Duration::from_millis(1))? {
    match event {
        network::Event::Connected(peer) => { /* ... */ }
        network::Event::Received { peer, packet } => { /* ... */ }
        network::Event::Disconnected(peer) => { /* ... */ }
    }
}
```

## 9. XR session

```rust
let mut session = xr::Session::new(&renderer)?;
while let Some(event) = session.poll_event()? {
    session.handle_event(event);
    if let Some(mut frame) = session.begin_frame()? {
        let views = frame.locate_views()?;
        // Render all active views.
        frame.finish()?;
    }
}
```

## 10. Common type families

| Category | Public types |
|---|---|
| Vectors | `Vec2`, `Vec3`, `Vec4`, float/double/integer variants |
| Matrices and transforms | `Mat22`, `Mat33`, `Mat44`, `DMat44`, `Quat`, `Basis` |
| Geometry | `AABB2`, `AABB3`, `OBB2`, `OBB3`, `Plane`, `Rect`, `Triangle` |
| Colour | `Color` with explicit component range and storage precision |
| Containers | language-native lists/maps/sets or typed Moss containers when allocator semantics matter |
| Results and references | target-language result/error types and explicit owned/borrowed resource wrappers |
| Jobs and synchronization | job system, thread pool, barrier, mutex, semaphore and scoped locks |

## 11. Resource-management rules

Owned handles are Rust structs with `Drop`; they are not `Copy` or `Clone` unless the native object is genuinely shareable. Borrowed APIs use `&T`/`&mut T`; avoid raw pointers in the safe module. Use `Arc<T>` only for explicitly shared native resources. Buffers accept slices, and callbacks use boxed, pinned or `Arc`-owned state retained until unregister.

## 12. Error-handling rules

Return `Result<T, MossError>` when a native operation can fail. Use `Option<T>` for an ordinary missing value and `bool` for boolean queries. Do not turn all native return values into `Result`; preserve their meaning. Callback panics must be caught at the FFI boundary and must never unwind through C.

## 13. Feature and platform checks

- A declaration in the reference does not mean every backend implements it. Check the selected GPU backend, platform, XR runtime and optional networking/codec features.
- Keep GPU resources associated with the device that created them; release child resources before the device.
- Pair renderer frame begin/end, GUI begin/end, render-pass begin/end and XR frame begin/end operations. Prefer scoped/structured helpers so early returns cannot leave a subsystem unbalanced.
- Poll platform and network events at the cadence required by the subsystem. Do not block a browser/UI thread waiting for asynchronous I/O.
- Use the API Reference's subsystem and header inventory when looking for advanced Navigation, Geometry, Physics, Resources, Skeleton or Variants APIs.

## 14. Where to look next

See [API_Reference.md](API_Reference.md) for the full native symbol crosswalk, target-language signatures, resource models, threading rules, feature coverage and the complete 270-header / 19-inline-header inventory.
