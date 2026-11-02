# MossRS API Reference

> **Complete-parity public API design for Rust.** This document specifies the intended idiomatic API when the binding exposes the full Moss feature set. It is a language-facing design contract, not a statement that every projected member is implemented in the current wrapper.

## 1. API design principles

Moss C++/CMoss is the canonical feature and behaviour baseline. The language binding is a **native Rust library**, not a mechanical translation of C++ identifiers. The FFI may use C-compatible handles and exported `Moss_*` symbols internally, but ordinary users should not see those details.

The public surface is a safe Rust crate. Use snake_case functions, PascalCase types, ownership and borrowing, RAII/Drop, `Result<T, MossError>` for recoverable failures, and typed options instead of raw descriptor pointers.

The binding must preserve feature coverage and semantics, not the number or spelling of native functions. Several native calls may become one managed method, a property, an iterator, a scoped object or a language protocol. Conversely, an overloaded native function may become distinct named methods when the language does not support overloads cleanly.

### Non-negotiable rules

- Keep the complete native feature set: Core, Platform, GPU, Renderer, Audio, GUI, Physics, Networking, XR, Navigation, Geometry, Resources, Skeleton, TriangleSplitter and Variants.
- Expose resource objects rather than asking users to pass opaque pointers repeatedly.
- Hide allocator plumbing, user-data pointers, C calling conventions, struct packing and raw WASM/JNI/P/Invoke handles behind the binding layer.
- Preserve status-query meaning. `false`, `0`, an empty collection and an absent optional value are not interchangeable with an error.
- Use asynchronous operations only when the target runtime requires them. WebAssembly startup and browser fetches are asynchronous; ordinary native API calls are not made asynchronous merely for stylistic consistency.
- Document platform/backend feature gates. A declared API is not proof that a backend or optional transport is available in a particular build.

## 2. Package and module layout

```rust
use moss::{
    audio, gpu, platform, renderer,
    platform::WindowOptions,
    Result,
};
use std::time::Duration;
```

### Subsystem map

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

The public package/module root should follow the repository's established package metadata. The names in this document are the recommended public design; implementation namespaces can be adjusted to the actual package identifier without changing the member design.

## 3. Core language type system

| Native Moss concept | Rust public representation | Design requirement |
|---|---|---|
| `bool` | `bool` | Keep boolean status and query values boolean. |
| Fixed-width signed/unsigned integers | `i8/u8, i16/u16, i32/u32, i64/u64` | Preserve width and unsigned semantics at the ABI boundary. |
| `size_t` | `usize` | Must be large enough for the host architecture. |
| `float` / `double` | `f32 / f64` | Do not narrow double precision silently. |
| UTF-8 text | `&str / String` | Copy or borrow according to documented native lifetime; validate encoding. |
| Opaque native resource | `owned struct with Drop; borrow with &T/&mut T` | Never expose raw pointers as normal user-facing values. |
| Pointer + element count | `&mut [T] / &[T]` | Validate length, element type and retention duration. |
| Function pointer + user data | `safe closure with retained state` | Unregister before callback state is released. |

### Value types and descriptors

Vectors, matrices, quaternions, colours, bounds, planes, rectangles and descriptor types should be real Rust values. Use value structs/records where copy semantics are cheap and safe; use resource classes only for objects with identity or native lifetime. Descriptor objects should have named fields/options instead of long sequences of positional primitives. Validate invalid enum combinations before crossing FFI.

## 4. Public resource model

| Moss native resource family | Recommended public type | Ownership model |
|---|---|---|
| Window / monitor / gamepad / camera / capture / storage | `Window`, `Monitor`, `Gamepad`, `Camera`, `Capture`, `Storage` | The application owns platform state; each opened resource has explicit close/dispose semantics. |
| GPU device / buffer / texture / sampler / shader / pipeline / command buffer / fence / semaphore | `GpuDevice`, `GpuBuffer`, `GpuTexture`, `GpuSampler`, `GpuShader`, `GpuPipeline`, `GpuCommandBuffer`, `GpuFence`, `GpuSemaphore` | Child objects retain or borrow their creating device; destroy children before device shutdown. |
| Renderer / material / framebuffer / mesh / model / font / sprite / batch | `Renderer`, `Material`, `Framebuffer`, `Mesh`, `Model`, `Font`, `Sprite`, `SpriteBatch` | Renderer-owned resources cannot outlive the renderer unless explicitly reference-counted. |
| Audio source / stream / listener / effect / microphone | `AudioSource`, `AudioStream`, `AudioListener`, `AudioEffect`, `Microphone` | Release sources, streams and listeners before audio shutdown. |
| GUI context | `GuiContext` | Context scopes widget calls; begin/end pairs are balanced by scoped helpers or callbacks. |
| Physics system / body / shape / constraint / character / vehicle / soft body | `PhysicsSystem`, `Body`, `Shape`, `Constraint`, `Character`, `Vehicle`, `SoftBody` | System and job/allocator dependencies outlive bodies and simulation work. |
| Network host / peer / packet / HTTP request / WebSocket / WebRTC | `NetworkHost`, `Peer`, `Packet`, `HttpRequest`, `WebSocket`, `WebRtcConnection` | Explicit ownership of hosts, packets, requests and connections. |
| XR instance / session / action set / action / hand / body / face | `XrRuntime`, `XrSession`, `XrActionSet`, `XrAction`, `HandTracker`, `BodyTracker`, `FaceTracker` | Session-bound objects cannot outlive the session/runtime they depend on. |

The exact implementation may consolidate thin native objects where that is safer (for example, a renderer may own framebuffers or a device may own command pools). The public API must still make ownership unambiguous.

## 5. Lifecycle, errors, callbacks and concurrency

### Resource lifetime

Owned handles are Rust structs with `Drop`; they are not `Copy` or `Clone` unless the native object is genuinely shareable. Borrowed APIs use `&T`/`&mut T`; avoid raw pointers in the safe module. Use `Arc<T>` only for explicitly shared native resources. Buffers accept slices, and callbacks use boxed, pinned or `Arc`-owned state retained until unregister.

### Errors and results

Return `Result<T, MossError>` when a native operation can fail. Use `Option<T>` for an ordinary missing value and `bool` for boolean queries. Do not turn all native return values into `Result`; preserve their meaning. Callback panics must be caught at the FFI boundary and must never unwind through C.

### Threading and callbacks

- State which thread owns window/event pumping, renderer submission, audio callbacks, physics updates and XR frame operations.
- Do not promise thread safety unless the native subsystem guarantees it.
- Keep callback state alive until native unregistration completes; no callback may run after disposal/close returns.
- Never allow a managed exception, panic or language-runtime longjmp to unwind through a C ABI frame.
- For asynchronous APIs, define cancellation, completion and disposal behaviour. Do not return a view over memory that is freed when the Promise/callback completes.

## 6. Idiomatic quick-start patterns

### Platform and event loop

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

### GPU resource creation

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

### Renderer frame

```rust
let mut frame = renderer.begin_frame()?;
frame.set_camera(&camera);
frame.draw_sprite(&sprite);
frame.draw_text("Hello Moss", position, &text_style);
frame.finish()?;
```

### Audio playback

```rust
audio::initialize()?;
let mut source = audio::load_wav("assets/theme.wav")?;
source.play()?;
audio::update(delta_time);
// Drop the source before shutting down the audio subsystem.
drop(source);
audio::shutdown();
```

### GUI context and widgets

```rust
let mut ui = gui::Context::new()?;
ui.window("Inspector", |ui| {
    ui.text("Player");
    let enabled = ui.checkbox("Enabled", current_value);
});
```

### Physics world

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

### Networking

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

### XR session

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

The examples show the intended public shape and lifecycle rules. Names and overloads in the index below are the parity mapping target; implementation-specific initialization parameters can be added to the relevant options objects without exposing the FFI.

## 7. Complete public API projection index

The following table maps every exported callable/allocator callback in the supplied CMoss declaration catalogue to a proposed Rust public entry point. The native name is included **only for traceability**. It is not the public name that developers should call. Target signatures use idiomatic types and hide receiver/context handles where those belong to an object.

- **1063 indexed entries:** 1,058 callable/constructor declarations and five allocator callback values.
- Duplicate native overloads are retained as separate rows where their parameters differ; the public API should resolve each to a distinct overload or a disambiguated method name.
- Native pointer/out-parameter patterns should become owned resource values, spans/slices, tuples/records or callback objects as appropriate to the language.

### `Moss_Audio.h` → `moss::audio`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `Moss_Init_Audio` | `audio::initialize(...)` | `pub fn initialize() -> Result<()>` | function |
| `Moss_Terminate_Audio` | `audio::shutdown(...)` | `pub fn shutdown() -> ()` | function |
| `Moss_AudioUpdate` | `audio::update(...)` | `pub fn update(delta_time: f32) -> ()` | function |
| `Moss_AudioLoadWavFile` | `audio::load_wav_file(...)` | `audio::load_wav_file(path: impl AsRef<Path>) -> Result<AudioSource>` | function |
| `Moss_AudioLoadWav` | `audio::load_wav(...)` | `pub fn load_wav() -> Result<AudioSource>` | function |
| `Moss_AudioLoadOgg` | `audio::load_ogg(...)` | `pub fn load_ogg(filename: &str, type: AudioLoadType) -> Result<AudioSource>` | function |
| `Moss_AudioLoadMP3` | `audio::load_mp3(...)` | `pub fn load_mp3(filename: &str) -> Result<AudioSource>` | function |
| `Moss_AudioSourceDestroy` | `AudioSource::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioCaptureMicrophone` | `audio::capture_microphone(...)` | `pub fn capture_microphone(microphone: &mut Microphone) -> Result<AudioSource>` | function |
| `Moss_AudioCreateEffect` (overload 1) | `audio::create_effect(...)` | `pub fn create_effect(type: AudioEffectType) -> AudioEffect` | function |
| `Moss_AudioCreateEffect` (overload 2) | `AudioEffect::set_parameter(...)` | `pub fn set_parameter(&mut self, param_name: &str, value: f32) -> ()` | function |
| `Moss_AudioRemoveEffect` | `AudioEffect::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioCreateChannel` | `audio::create_channel(...)` | `pub fn create_channel(channel: ChannelId) -> ChannelId` | function |
| `Audio_RemoveChannel` | `audio::remove_channel(...)` | `pub fn remove_channel(channel: ChannelId) -> ()` | function |
| `Moss_AudioGetMasterChannel` | `audio::get_master_channel(...)` | `pub fn get_master_channel() -> ChannelId` | function |
| `Moss_AudioSetChannelVolume` | `audio::set_channel_volume(...)` | `pub fn set_channel_volume(channel: ChannelId, volume: f32) -> ()` | function |
| `Moss_AudioSetChannelMute` | `audio::set_channel_mute(...)` | `pub fn set_channel_mute(channel: ChannelId, mute: bool) -> ()` | function |
| `Moss_AudioAddChannelEffect` | `audio::add_effect_to_channel(...)` | `pub fn add_effect_to_channel(channel: ChannelId, effect: &mut AudioEffect) -> ()` | function |
| `Moss_AudioRemoveChannelEffect` | `audio::remove_effect_from_channel(...)` | `pub fn remove_effect_from_channel(channel: ChannelId, effect: &mut AudioEffect) -> ()` | function |
| `Moss_AudioRemoveAllChannelEffects` | `audio::clear_channel_effects(...)` | `pub fn clear_channel_effects(channel: ChannelId) -> ()` | function |
| `Moss_AudioStreamCreate` | `audio::create_stream(...)` | `pub fn create_stream() -> Result<AudioStream>` | function |
| `Moss_AudioStreamPlay` | `AudioStream::play(...)` | `pub fn play(&mut self) -> ()` | function |
| `Moss_AudioStreamStop` | `AudioStream::stop(...)` | `pub fn stop(&mut self) -> ()` | function |
| `Moss_AudioStreamSetVolume` | `AudioStream::set_volume(...)` | `pub fn set_volume(&mut self, volume: f32) -> ()` | function |
| `Moss_AudioStreamSetPitch` | `AudioStream::set_pitch(...)` | `pub fn set_pitch(&mut self, pitch: f32) -> ()` | function |
| `Moss_AudioStreamSetPlaybackRate` | `AudioStream::set_playback_rate(...)` | `pub fn set_playback_rate(&mut self, rate: f32) -> ()` | function |
| `Moss_AudioStreamSetPan` | `AudioStream::set_pan(...)` | `pub fn set_pan(&mut self, pan: f32) -> ()` | function |
| `Moss_AudioStreamSetLoop` | `AudioStream::set_loop(...)` | `pub fn set_loop(&mut self, loop: bool) -> ()` | function |
| `Moss_AudioStreamRemove` | `AudioStream::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioStream2DCreate` | `audio::create_stream_dimension2_d(...)` | `pub fn create_stream_dimension2_d() -> Result<AudioStreamDIMENSION2D>` | function |
| `Moss_AudioStream2DPlay` | `AudioStreamDIMENSION2D::play(...)` | `pub fn play(&mut self) -> ()` | function |
| `Moss_AudioStream2DStop` | `AudioStreamDIMENSION2D::stop(...)` | `pub fn stop(&mut self) -> ()` | function |
| `Moss_AudioStream2DSetVolume` | `AudioStreamDIMENSION2D::set_volume(...)` | `pub fn set_volume(&mut self, volume: f32) -> ()` | function |
| `Moss_AudioStream2DSetPitch` | `AudioStreamDIMENSION2D::set_pitch(...)` | `pub fn set_pitch(&mut self, pitch: f32) -> ()` | function |
| `Moss_AudioStream2DSetPan` | `AudioStreamDIMENSION2D::set_pan(...)` | `pub fn set_pan(&mut self, pan: f32) -> ()` | function |
| `Moss_AudioStream2DSetLoop` | `AudioStreamDIMENSION2D::set_loop(...)` | `pub fn set_loop(&mut self, loop: bool) -> ()` | function |
| `Moss_AudioStream3DSetPosition` (overload 1) | `AudioStreamDIMENSION2D::stream_dimension3_set_position(...)` | `pub fn stream_dimension3_set_position(&mut self) -> ()` | function |
| `Moss_AudioStream3DSetVelocity` (overload 1) | `AudioStreamDIMENSION2D::stream_dimension3_set_velocity(...)` | `pub fn stream_dimension3_set_velocity(&mut self) -> ()` | function |
| `Moss_AudioStream3DSetMaxDistance` (overload 1) | `AudioStreamDIMENSION2D::stream_dimension3_set_max_distance(...)` | `pub fn stream_dimension3_set_max_distance(&mut self) -> ()` | function |
| `Moss_AudioStream2DRemove` | `AudioStreamDIMENSION2D::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioStream3DCreate` | `audio::create_stream_dimension3_d(...)` | `pub fn create_stream_dimension3_d() -> Result<AudioStreamDIMENSION3D>` | function |
| `Moss_AudioStream3DPlay` | `AudioStreamDIMENSION3D::play(...)` | `pub fn play(&mut self) -> ()` | function |
| `Moss_AudioStream3DStop` | `AudioStreamDIMENSION3D::stop(...)` | `pub fn stop(&mut self) -> ()` | function |
| `Moss_AudioStream3DSetVolume` | `AudioStreamDIMENSION3D::set_volume(...)` | `pub fn set_volume(&mut self, volume: f32) -> ()` | function |
| `Moss_AudioStream3DSetPitch` | `AudioStreamDIMENSION3D::set_pitch(...)` | `pub fn set_pitch(&mut self, pitch: f32) -> ()` | function |
| `Moss_AudioStream3DSetPlaybackRate` | `AudioStreamDIMENSION3D::set_playback_rate(...)` | `pub fn set_playback_rate(&mut self, rate: f32) -> ()` | function |
| `Moss_AudioStream3DSetPan` | `AudioStreamDIMENSION3D::set_pan(...)` | `pub fn set_pan(&mut self, pan: f32) -> ()` | function |
| `Moss_AudioStream3DSetLoop` | `AudioStreamDIMENSION3D::set_loop(...)` | `pub fn set_loop(&mut self, loop: bool) -> ()` | function |
| `Moss_AudioStream3DSetPosition` (overload 2) | `AudioStreamDIMENSION3D::set_position(...)` | `pub fn set_position(&mut self) -> ()` | function |
| `Moss_AudioStream3DSetVelocity` (overload 2) | `AudioStreamDIMENSION3D::set_velocity(...)` | `pub fn set_velocity(&mut self) -> ()` | function |
| `Moss_AudioStream3DSetMaxDistance` (overload 2) | `AudioStreamDIMENSION3D::set_max_distance(...)` | `pub fn set_max_distance(&mut self) -> ()` | function |
| `Moss_AudioStream3DSetDistanceModel` | `AudioStreamDIMENSION3D::set_distance_model(...)` | `pub fn set_distance_model(&mut self, model: DistanceModel) -> ()` | function |
| `Moss_AudioStream3DRemove` | `AudioStreamDIMENSION3D::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioCreateAudioListener2D` | `audio::create_audio_listener_dimension2_d(...)` | `pub fn create_audio_listener_dimension2_d(position: Float2) -> Result<AudioListenerDIMENSION2D>` | function |
| `Moss_AudioCreateAudioListener3D` | `audio::create_audio_listener_dimension3_d(...)` | `pub fn create_audio_listener_dimension3_d(position: Float3) -> Result<AudioListenerDIMENSION3D>` | function |
| `Moss_AudioCreateRayAudioListener2D` | `audio::create_ray_audio_listener_dimension2_d(...)` | `pub fn create_ray_audio_listener_dimension2_d(position: Float2) -> Result<RayAudioListenerDIMENSION2D>` | function |
| `Moss_AudioCreateRayAudioListener3D` | `audio::create_ray_audio_listener_dimension3_d(...)` | `pub fn create_ray_audio_listener_dimension3_d(position: Float3) -> Result<RayAudioListenerDIMENSION3D>` | function |
| `Moss_AudioRemoveAudioListener2D` | `AudioListenerDIMENSION2D::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioRemoveAudioListener3D` | `AudioListenerDIMENSION3D::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioRemoveRayAudioListener2D` | `RayAudioListenerDIMENSION2D::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioRemoveRayAudioListener3D` | `RayAudioListenerDIMENSION3D::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AudioActivateAudioListener2D` | `AudioListenerDIMENSION2D::activate(...)` | `pub fn activate(&self, activate: bool) -> ()` | function |
| `Moss_AudioActivateAudioListener3D` | `AudioListenerDIMENSION3D::activate(...)` | `pub fn activate(&self, activate: bool) -> ()` | function |
| `Moss_AudioActivateRayAudioListener2D` | `RayAudioListenerDIMENSION2D::activate(...)` | `pub fn activate(&self, activate: bool) -> ()` | function |
| `Moss_AudioActivateRayAudioListener3D` | `RayAudioListenerDIMENSION3D::activate(...)` | `pub fn activate(&self, activate: bool) -> ()` | function |
| `Moss_AudioListenerSetOrientation` | `AudioListenerDIMENSION3D::listener_set_orientation(...)` | `pub fn listener_set_orientation(&mut self, forward: Vec3, up: Vec3) -> ()` | function |
| `Moss_AudioRayListener2DSetPhysics` | `RayAudioListenerDIMENSION2D::ray_set_physics(...)` | `pub fn ray_set_physics(&mut self, physics: &mut PhysicsSystem, raycast: AudioRaycastDIMENSION2DCallback) -> ()` | function |
| `Moss_AudioRayListener3DSetPhysics` | `RayAudioListenerDIMENSION3D::ray_set_physics(...)` | `pub fn ray_set_physics(&mut self, physics: &mut PhysicsSystem, raycast: AudioRaycastDIMENSION3DCallback) -> ()` | function |
| `Moss_AudioRayListener2DSetTraceSettings` | `RayAudioListenerDIMENSION2D::ray_set_trace_settings(...)` | `pub fn ray_set_trace_settings(&mut self, max_distance: f32, reflection_rays: u32) -> ()` | function |
| `Moss_AudioRayListener3DSetTraceSettings` | `RayAudioListenerDIMENSION3D::ray_set_trace_settings(...)` | `pub fn ray_set_trace_settings(&mut self, max_distance: f32, reflection_rays: u32) -> ()` | function |
| `Moss_AudioRayTrace2D` | `audio::ray_trace_dimension2_d(...)` | `pub fn ray_trace_dimension2_d(desc: &AudioRayTraceDIMENSION2DDesc, out_result: &mut AudioRayTraceResult) -> bool` | function |
| `Moss_AudioRayTrace3D` | `audio::ray_trace_dimension3_d(...)` | `pub fn ray_trace_dimension3_d(desc: &AudioRayTraceDIMENSION3DDesc, out_result: &mut AudioRayTraceResult) -> bool` | function |
| `Moss_AudioRayTraceFromListener2D` | `RayAudioListenerDIMENSION2D::ray_trace_from(...)` | `pub fn ray_trace_from(&self, source_position: &Vec2, out_result: &mut AudioRayTraceResult) -> bool` | function |
| `Moss_AudioRayTraceFromListener3D` | `RayAudioListenerDIMENSION3D::ray_trace_from(...)` | `pub fn ray_trace_from(&self, source_position: &Vec3, out_result: &mut AudioRayTraceResult) -> bool` | function |
| `Moss_AudioRayTraceComputeGain` | `AudioRayTraceResult::ray_trace_compute_gain(...)` | `pub fn ray_trace_compute_gain(&self) -> f32` | function |
| `Moss_IsSpeakerDeviceReady` | `audio::is_speaker_device_ready(...)` | `pub fn is_speaker_device_ready() -> bool` | function |
| `Moss_AudioSpeakerOpen` | `audio::speaker_open(...)` | `pub fn speaker_open() -> ()` | function |
| `Moss_AudioSpeakerPause` | `audio::speaker_pause(...)` | `pub fn speaker_pause() -> ()` | function |
| `Moss_AudioSpeakerResume` | `audio::speaker_resume(...)` | `pub fn speaker_resume() -> ()` | function |
| `Moss_AudioSpeakerIsPaused` | `audio::speaker_is_paused(...)` | `pub fn speaker_is_paused() -> bool` | function |
| `Moss_AudioSelectSpeakerDevice` | `audio::select_speaker_device(...)` | `pub fn select_speaker_device(id: i32) -> bool` | function |
| `Moss_GetCurrentSpeakerDeviceID` | `audio::get_current_speaker_device_id(...)` | `pub fn get_current_speaker_device_id() -> i32` | function |
| `Moss_GetSpeakerDeviceName` | `audio::get_speaker_device_name(...)` | `pub fn get_speaker_device_name(id: i32) -> Option<String>` | function |
| `Moss_ListSpeakerDevices` | `audio::list_speaker_devices(...)` | `pub fn list_speaker_devices() -> i32` | function |
| `Moss_MicrophoneGetDeviceCount` | `audio::microphone_get_device_count(...)` | `pub fn microphone_get_device_count() -> u32` | function |
| `Moss_MicrophoneGetDeviceName` | `audio::microphone_get_device_name(...)` | `pub fn microphone_get_device_name(device_index: u32) -> Option<String>` | function |
| `Moss_MicrophoneOpen` | `audio::microphone_open(...)` | `pub fn microphone_open(desc: &MicrophoneDesc) -> Result<Microphone>` | function |
| `Moss_MicrophoneClose` | `Microphone::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_MicrophoneStart` | `Microphone::start(...)` | `pub fn start(&self) -> bool` | function |
| `Moss_MicrophoneStop` | `Microphone::stop(...)` | `pub fn stop(&mut self) -> ()` | function |
| `Moss_MicrophoneRead` | `Microphone::read(...)` | `pub fn read(&self, out_samples: &mut &mut f32, max_frames: u32) -> u32` | function |
| `Moss_MicrophoneSetGain` | `Microphone::set_gain(...)` | `pub fn set_gain(&mut self, gain: f32) -> ()` | function |
| `Moss_MicrophoneGetSampleRate` | `Microphone::get_sample_rate(...)` | `pub fn get_sample_rate(&self) -> u32` | function |
| `Moss_MicrophoneGetChannels` | `Microphone::get_channels(...)` | `pub fn get_channels(&self) -> u32` | function |
| `Moss_MicrophoneSetCallback` | `Microphone::set_callback(...)` | `pub fn set_callback(&mut self, callback: MicrophoneCallback) -> ()` | function |
| `Moss_MicrophoneGetLevels` | `Microphone::get_levels(...)` | `pub fn get_levels(&self) -> MicrophoneLevels` | function |
| `Moss_MicrophoneGetLevelRMS` | `Microphone::get_level_rms(...)` | `pub fn get_level_rms(&self) -> f32` | function |
| `Moss_MicrophoneGetLevelPeak` | `Microphone::get_level_peak(...)` | `pub fn get_level_peak(&self) -> f32` | function |
| `Moss_MicrophoneGetSmoothedVolume` | `Microphone::get_smoothed_volume(...)` | `pub fn get_smoothed_volume(&self) -> f32` | function |
| `Moss_MicrophoneGetVoiceActivity` | `Microphone::get_voice_activity(...)` | `pub fn get_voice_activity(&self) -> f32` | function |
| `Moss_IsMicrophoneDeviceReady` | `audio::is_microphone_device_ready(...)` | `pub fn is_microphone_device_ready() -> bool` | function |
| `Moss_AudioMicrophoneOpen` | `audio::microphone_open(...)` | `pub fn microphone_open() -> i32` | function |
| `Moss_AudioMicrophoneClose` | `audio::microphone_close(...)` | `pub fn microphone_close() -> ()` | function |
| `Moss_AudioMicrophonePlay` | `audio::microphone_play(...)` | `pub fn microphone_play() -> ()` | function |
| `Moss_AudioMicrophoneStop` | `audio::microphone_stop(...)` | `pub fn microphone_stop() -> ()` | function |
| `Moss_AudioMicrophoneID` | `audio::microphone_id(...)` | `pub fn microphone_id() -> i32` | function |
| `Moss_AudioSelectMicrophoneDevice` | `audio::select_microphone_device(...)` | `pub fn select_microphone_device(id: i32) -> bool` | function |
| `Moss_GetMicrophoneDeviceName` | `audio::get_microphone_device_name(...)` | `pub fn get_microphone_device_name(index: i32) -> Option<String>` | function |
| `Moss_ListMicrophoneDevices` | `audio::list_microphone_devices(...)` | `pub fn list_microphone_devices() -> i32` | function |
| `Moss_AudioMicrophoneSetGain` | `Microphone::set_gain(...)` | `pub fn set_gain(&mut self, gain: f32) -> ()` | function |
| `Moss_AudioMicrophoneGetSampleRate` | `Microphone::get_sample_rate(...)` | `pub fn get_sample_rate(&self) -> i32` | function |
| `Moss_AudioMicrophoneGetChannels` | `Microphone::get_channels(...)` | `pub fn get_channels(&self) -> i32` | function |
| `Moss_AudioStreamSetCallback` | `AudioStream::set_callback(...)` | `pub fn set_callback(&mut self, callback: AudioStreamCallback) -> ()` | function |

### `Moss_GPU.h` → `moss::gpu`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `Moss_AcquireGPUCommandBuffer` | `GpuDevice::acquire_command_buffer(...)` | `pub fn acquire_command_buffer(&self) -> Result<GpuCommandBuffer>` | function |
| `Moss_BeginGPUComputePass` | `GpuCommandBuffer::begin_compute_pass(...)` | `pub fn begin_compute_pass(&mut self) -> ()` | function |
| `Moss_BeginGPUCopyPass` | `GpuCommandBuffer::begin_copy_pass(...)` | `pub fn begin_copy_pass(&mut self) -> ()` | function |
| `Moss_CreateGPUBuffer` | `GpuDevice::create_buffer(...)` | `pub fn create_buffer(&mut self, desc: &GpuBufferDesc) -> Result<GpuBuffer>` | function |
| `Moss_DestroyGPUDevice` | `GpuDevice::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_BeginGPURenderPass` | `GpuCommandBuffer::begin_render_pass(...)` | `pub fn begin_render_pass(&mut self, framebuffer: &mut Framebuffer) -> ()` | function |
| `Moss_BindGPUComputeSamplers` | `GpuCommandBuffer::bind_compute_samplers(...)` | `pub fn bind_compute_samplers(&self, first_slot: u32, samplers: &[GpuSampler*]) -> ()` | function |
| `Moss_BindGPUComputeTextures` | `GpuCommandBuffer::bind_compute_textures(...)` | `pub fn bind_compute_textures(&self, first_slot: u32, textures: &[GpuTexture*]) -> ()` | function |
| `Moss_BindGPUComputeUniformBuffers` | `GpuCommandBuffer::bind_compute_uniform_buffers(...)` | `pub fn bind_compute_uniform_buffers(&self, first_slot: u32, bindings: &[GpuBufferBinding]) -> ()` | function |
| `Moss_BindGPUFragmentSamplers` | `GpuCommandBuffer::bind_fragment_samplers(...)` | `pub fn bind_fragment_samplers(&self, first_slot: u32, samplers: &[GpuSampler*]) -> ()` | function |
| `Moss_BindGPUFragmentTextures` | `GpuCommandBuffer::bind_fragment_textures(...)` | `pub fn bind_fragment_textures(&self, first_slot: u32, textures: &[GpuTexture*]) -> ()` | function |
| `Moss_BindGPUFragmentUniformBuffers` | `GpuCommandBuffer::bind_fragment_uniform_buffers(...)` | `pub fn bind_fragment_uniform_buffers(&self, first_slot: u32, bindings: &[GpuBufferBinding]) -> ()` | function |
| `Moss_BindGPUIndexBuffer` | `GpuCommandBuffer::bind_index(...)` | `pub fn bind_index(&self, buffer: &mut GpuBuffer, offset: u64, index_type: GpuIndexType) -> ()` | function |
| `Moss_BindGPUPipeline` | `GpuCommandBuffer::bind_pipeline(...)` | `pub fn bind_pipeline(&self, pipeline: &mut PipelineState) -> ()` | function |
| `Moss_BindGPUResourceSet` | `GpuCommandBuffer::bind_resource_set(...)` | `pub fn bind_resource_set(&mut self, set_index: u32, set: &mut ResourceSet) -> ()` | function |
| `Moss_BindGPUVertexBuffers` | `GpuCommandBuffer::bind_vertex_buffers(...)` | `pub fn bind_vertex_buffers(&self, first_binding: u32, bindings: &[GpuBufferBinding]) -> ()` | function |
| `Moss_BindGPUVertexSamplers` | `GpuCommandBuffer::bind_vertex_samplers(...)` | `pub fn bind_vertex_samplers(&self, first_slot: u32, samplers: &[GpuSampler*]) -> ()` | function |
| `Moss_BindGPUVertexTextures` | `GpuCommandBuffer::bind_vertex_textures(...)` | `pub fn bind_vertex_textures(&self, first_slot: u32, textures: &[GpuTexture*]) -> ()` | function |
| `Moss_BindGPUVertexUniformBuffers` | `GpuCommandBuffer::bind_vertex_uniform_buffers(...)` | `pub fn bind_vertex_uniform_buffers(&self, first_slot: u32, bindings: &[GpuBufferBinding]) -> ()` | function |
| `Moss_BlitGPUTexture` | `GpuCommandBuffer::blit_texture(...)` | `pub fn blit_texture(&self, src: &mut Texture, dst: &mut Texture, region: &GpuBlitRegion) -> ()` | function |
| `Moss_CalculateGPUTextureFormatSize` | `gpu::calculate_texture_format_size(...)` | `pub fn calculate_texture_format_size(format: TextureFormat, width: u32, height: u32, depth: u32) -> u32` | function |
| `Moss_CancelGPUCommandBuffer` | `GpuCommandBuffer::cancel(...)` | `pub fn cancel(&self) -> ()` | function |
| `Moss_CopyGPUBufferToBuffer` | `GpuCommandBuffer::copy_to_buffer(...)` | `pub fn copy_to_buffer(&self, src: &mut GpuBuffer, dst: &mut GpuBuffer, src_region: &GpuBufferRegion, dst_region: &GpuBufferRegion) -> ()` | function |
| `Moss_CopyGPUBufferToTexture` | `GpuCommandBuffer::copy_to_texture(...)` | `pub fn copy_to_texture(&self, src: &mut GpuBuffer, dst: &mut GpuTexture, src_region: &GpuBufferRegion, dst_region: &GpuTextureRegion) -> ()` | function |
| `Moss_CopyGPUTextureToBuffer` | `GpuCommandBuffer::copy_texture_to(...)` | `pub fn copy_texture_to(&self, src: &mut GpuTexture, dst: &mut GpuBuffer, src_region: &GpuTextureRegion, dst_region: &GpuBufferRegion) -> ()` | function |
| `Moss_CopyGPUTextureToTexture` | `GpuCommandBuffer::copy_texture_to_texture(...)` | `pub fn copy_texture_to_texture(&self, src: &mut GpuTexture, dst: &mut GpuTexture, src_region: &GpuTextureRegion, dst_region: &GpuTextureRegion) -> ()` | function |
| `Moss_CreateGPUSampler` | `GpuDevice::create_sampler(...)` | `pub fn create_sampler(&mut self, create_info: &GpuSamplerCreateInfo) -> Result<GpuSampler>` | function |
| `Moss_CreateGPUShader` | `GpuDevice::create_shader(...)` | `pub fn create_shader(&mut self, create_info: &GpuShaderCreateInfo) -> Result<GpuShader>` | function |
| `Moss_CreateGPUComputePipeline` | `GpuDevice::create_compute_pipeline(...)` | `pub fn create_compute_pipeline(&mut self, create_info: &GpuComputePipelineCreateInfo) -> Result<ComputePipelineState>` | function |
| `Moss_CreateGPUTexture` | `GpuDevice::create_texture(...)` | `pub fn create_texture(&mut self, create_info: &GpuTextureCreateInfo) -> Result<GpuTexture>` | function |
| `Moss_CreateGPUTextureView` | `GpuDevice::create_texture_view(...)` | `pub fn create_texture_view(&mut self, create_info: &GpuTextureViewCreateInfo) -> Result<GpuTextureView>` | function |
| `Moss_CreateGPUTransferBuffer` | `GpuDevice::create_transfer_buffer(...)` | `pub fn create_transfer_buffer(&mut self, create_info: &GpuTransferBufferCreateInfo) -> Result<GpuTransferBuffer>` | function |
| `Moss_CreateGPUFramebuffer` | `GpuDevice::create_framebuffer(...)` | `pub fn create_framebuffer(&mut self, create_info: &GpuFramebufferCreateInfo) -> Result<Framebuffer>` | function |
| `Moss_CreateGPUResourceSetLayout` | `GpuDevice::create_resource_set_layout(...)` | `pub fn create_resource_set_layout(&mut self, create_info: &GpuResourceSetLayoutCreateInfo) -> Result<ResourceSetLayout>` | function |
| `Moss_CreateGPUResourceSet` | `GpuDevice::create_resource_set(...)` | `pub fn create_resource_set(&mut self, create_info: &GpuResourceSetCreateInfo) -> Result<ResourceSet>` | function |
| `Moss_CreateGPUQueryPool` | `GpuDevice::create_query_pool(...)` | `pub fn create_query_pool(&mut self, create_info: &GpuQueryPoolCreateInfo) -> Result<GpuQueryPool>` | function |
| `Moss_DispatchGPUCompute` | `GpuCommandBuffer::dispatch_compute(...)` | `pub fn dispatch_compute(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) -> ()` | function |
| `Moss_DispatchGPUComputeIndirect` | `GpuCommandBuffer::dispatch_compute_indirect(...)` | `pub fn dispatch_compute_indirect(&self, indirect_buffer: &mut GpuBuffer, offset: u64) -> ()` | function |
| `Moss_DownloadFromGPUBuffer` | `GpuBuffer::download_from(...)` | `pub fn download_from(&self, dst_data: &mut [u8], size: u64, src_offset: u64) -> ()` | function |
| `Moss_DownloadFromGPUTexture` | `GpuTexture::download_from(...)` | `pub fn download_from(&self, src_region: &GpuTextureRegion, dst_data: &mut [u8], dst_row_pitch: u32, dst_slice_pitch: u32) -> ()` | function |
| `Moss_DrawGPUIndexedPrimitives` | `GpuCommandBuffer::draw_indexed_primitives(...)` | `pub fn draw_indexed_primitives(&self, index_count: u32, instance_count: u32, first_index: u32, vertex_offset: i32, first_instance: u32) -> ()` | function |
| `Moss_DrawGPUIndexedPrimitivesIndirect` | `GpuCommandBuffer::draw_indexed_primitives_indirect(...)` | `pub fn draw_indexed_primitives_indirect(&self, indirect_buffer: &mut GpuBuffer, offset: u64, draw_count: u32, stride: u32) -> ()` | function |
| `Moss_DrawGPUPrimitives` | `GpuCommandBuffer::draw_primitives(...)` | `pub fn draw_primitives(&self, vertex_count: u32, instance_count: u32, first_vertex: u32, first_instance: u32) -> ()` | function |
| `Moss_DrawGPUPrimitivesIndirect` | `GpuCommandBuffer::draw_primitives_indirect(...)` | `pub fn draw_primitives_indirect(&self, indirect_buffer: &mut GpuBuffer, offset: u64, draw_count: u32, stride: u32) -> ()` | function |
| `Moss_EndGPUComputePass` | `GpuCommandBuffer::end_compute_pass(...)` | `pub fn end_compute_pass(&mut self) -> ()` | function |
| `Moss_EndGPUCopyPass` | `GpuCommandBuffer::end_copy_pass(...)` | `pub fn end_copy_pass(&mut self) -> ()` | function |
| `Moss_EndGPURenderPass` | `GpuCommandBuffer::end_render_pass(...)` | `pub fn end_render_pass(&mut self) -> ()` | function |
| `Moss_GDKResumeGPU` | `GpuDevice::gdk_resume(...)` | `pub fn gdk_resume(&self) -> ()` | function |
| `Moss_GDKSuspendGPU` | `GpuDevice::gdk_suspend(...)` | `pub fn gdk_suspend(&self) -> ()` | function |
| `Moss_GenerateMipmapsForGPUTexture` | `GpuCommandBuffer::generate_mipmaps_for_texture(...)` | `pub fn generate_mipmaps_for_texture(&self, texture: &mut GpuTexture) -> ()` | function |
| `Moss_GetGPUDriver` | `gpu::get_driver(...)` | `pub fn get_driver(driver_index: u32) -> Option<String>` | function |
| `Moss_GetCompiledGPUBackendType` | `gpu::get_compiled_backend_type(...)` | `pub fn get_compiled_backend_type() -> GpuBackendType` | function |
| `Moss_GetGPUDeviceBackendType` | `GpuDevice::get_backend_type(...)` | `pub fn get_backend_type(&self) -> GpuBackendType` | function |
| `Moss_GPUBackendGetName` | `gpu::backend_get_name(...)` | `pub fn backend_get_name(backend: GpuBackendType) -> Option<String>` | function |
| `Moss_GPUBackendGetShaderFormats` | `gpu::backend_get_shader_formats(...)` | `pub fn backend_get_shader_formats(backend: GpuBackendType) -> u32` | function |
| `Moss_GPUBackendIsCompiled` | `gpu::backend_is_compiled(...)` | `pub fn backend_is_compiled(backend: GpuBackendType) -> bool` | function |
| `Moss_GPUBackendIsSupportedOnPlatform` | `gpu::backend_is_supported_on_platform(...)` | `pub fn backend_is_supported_on_platform(backend: GpuBackendType) -> bool` | function |
| `Moss_GetGPUBackendInfo` | `gpu::get_backend_info(...)` | `pub fn get_backend_info(backend: GpuBackendType, out_info: &mut GpuBackendInfo) -> bool` | function |
| `Moss_RegisterNativeGPUBackend` | `gpu::register_native_backend(...)` | `pub fn register_native_backend(backend: GpuBackendType, native_table: &[u8]) -> bool` | function |
| `Moss_HasNativeGPUBackend` | `gpu::has_native_backend(...)` | `pub fn has_native_backend(backend: GpuBackendType) -> bool` | function |
| `Moss_GetGPUShaderFormats` | `GpuDevice::get_shader_formats(...)` | `pub fn get_shader_formats(&self) -> u32` | function |
| `Moss_GetGPUTextureFormatFromPixelFormat` | `gpu::get_texture_format_from_pixel_format(...)` | `pub fn get_texture_format_from_pixel_format(pixel_format: PixelFormat) -> TextureFormat` | function |
| `Moss_GetNumGPUDrivers` | `gpu::get_num_drivers(...)` | `pub fn get_num_drivers() -> u32` | function |
| `Moss_GetPixelFormatFromGPUTextureFormat` | `gpu::get_pixel_format_from_texture_format(...)` | `pub fn get_pixel_format_from_texture_format(texture_format: TextureFormat) -> PixelFormat` | function |
| `Moss_GPUSupportsProperties` | `GpuDevice::supports_properties(...)` | `pub fn supports_properties(&self) -> bool` | function |
| `Moss_GPUSupportsShaderFormats` | `GpuDevice::supports_shader_formats(...)` | `pub fn supports_shader_formats(&self, shader_format_mask: u32) -> bool` | function |
| `Moss_GPUTextureFormatTexelBlockSize` | `gpu::texture_format_texel_block_size(...)` | `pub fn texture_format_texel_block_size(format: TextureFormat) -> u32` | function |
| `Moss_GPUTextureSupportsFormat` | `GpuDevice::texture_supports_format(...)` | `pub fn texture_supports_format(&self, format: TextureFormat, usage: TextureUsage) -> bool` | function |
| `Moss_GPUTextureSupportsSampleCount` | `GpuDevice::texture_supports_sample_count(...)` | `pub fn texture_supports_sample_count(&self, format: TextureFormat, sample_count: u32) -> bool` | function |
| `Moss_InsertGPUDebugLabel` | `GpuCommandBuffer::insert_debug_label(...)` | `pub fn insert_debug_label(&self, label: &str) -> ()` | function |
| `Moss_MapGPUTransferBuffer` | `GpuTransferBuffer::map(...)` | `pub fn map(&self) -> &mut [u8]` | function |
| `Moss_PopGPUDebugGroup` | `GpuCommandBuffer::pop_debug_group(...)` | `pub fn pop_debug_group(&self) -> ()` | function |
| `Moss_PushGPUComputeUniformData` | `GpuCommandBuffer::push_compute_uniform_data(...)` | `pub fn push_compute_uniform_data(&self, slot: u32, data: &[u8], size: u32) -> ()` | function |
| `Moss_PushGPUDebugGroup` | `GpuCommandBuffer::push_debug_group(...)` | `pub fn push_debug_group(&self, label: &str) -> ()` | function |
| `Moss_PushGPUFragmentUniformData` | `GpuCommandBuffer::push_fragment_uniform_data(...)` | `pub fn push_fragment_uniform_data(&self, slot: u32, data: &[u8], size: u32) -> ()` | function |
| `Moss_PushGPUVertexUniformData` | `GpuCommandBuffer::push_vertex_uniform_data(...)` | `pub fn push_vertex_uniform_data(&self, slot: u32, data: &[u8], size: u32) -> ()` | function |
| `Moss_QueryGPUFence` | `GpuFence::query(...)` | `pub fn query(&self) -> bool` | function |
| `Moss_ReleaseGPUBuffer` | `GpuBuffer::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_ReleaseGPUFence` | `GpuFence::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_ReleaseGPUSampler` | `GpuSampler::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_ReleaseGPUShader` | `GpuShader::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_ReleaseGPUComputePipeline` | `GpuDevice::release_compute_pipeline(...)` | `pub fn release_compute_pipeline(&self, pipeline: &mut ComputePipelineState) -> ()` | function |
| `Moss_ReleaseGPUTexture` | `GpuTexture::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_ReleaseGPUTextureView` | `GpuTextureView::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_ReleaseGPUTransferBuffer` | `GpuTransferBuffer::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_ReleaseGPUFramebuffer` | `Framebuffer::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_ReleaseGPUResourceSetLayout` | `ResourceSetLayout::release(...)` | `pub fn release(&mut self) -> ()` | function |
| `Moss_ReleaseGPUResourceSet` | `ResourceSet::release(...)` | `pub fn release(&mut self) -> ()` | function |
| `Moss_ReleaseGPUQueryPool` | `GpuQueryPool::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_SetGPUAllowedFramesInFlight` | `GpuDevice::set_allowed_frames_in_flight(...)` | `pub fn set_allowed_frames_in_flight(&mut self, frames_in_flight: u32) -> ()` | function |
| `Moss_SetGPUBlendConstants` | `GpuCommandBuffer::set_blend_constants(...)` | `pub fn set_blend_constants(&mut self, r: f32, g: f32, b: f32, a: f32) -> ()` | function |
| `Moss_SetGPUBufferName` | `GpuBuffer::set_name(...)` | `pub fn set_name(&mut self, name: &str) -> ()` | function |
| `Moss_SetGPUScissor` (overload 1) | `GpuCommandBuffer::set_scissor(...)` | `pub fn set_scissor(&mut self, scissor: &Rect) -> ()` | function |
| `Moss_SetGPUStencilReference` | `GpuCommandBuffer::set_stencil_reference(...)` | `pub fn set_stencil_reference(&mut self, reference: u32) -> ()` | function |
| `Moss_SetGPUTextureName` | `GpuTexture::set_name(...)` | `pub fn set_name(&mut self, name: &str) -> ()` | function |
| `Moss_SetGPUViewport` | `GpuCommandBuffer::set_viewport(...)` | `pub fn set_viewport(&mut self, viewport: &GpuViewport) -> ()` | function |
| `Moss_SetGPUScissor` (overload 2) | `GpuCommandBuffer::set_scissor(...)` | `pub fn set_scissor(&mut self, scissors: &Rect) -> ()` | function |
| `Moss_SubmitGPUCommandBuffer` | `GpuCommandBuffer::submit(...)` | `pub fn submit(&mut self) -> ()` | function |
| `Moss_SubmitGPUCommandBufferAndAcquireFence` | `GpuCommandBuffer::submit_and_acquire_fence(...)` | `pub fn submit_and_acquire_fence(&mut self) -> Result<GpuFence>` | function |
| `Moss_BindGPUComputePipeline` | `GpuCommandBuffer::bind_compute_pipeline(...)` | `pub fn bind_compute_pipeline(&self, pipeline: &mut ComputePipelineState) -> ()` | function |
| `Moss_BarrierGPUResources` | `GpuCommandBuffer::barrier_resources(...)` | `pub fn barrier_resources(&self, barrier_info: &GpuBarrierInfo) -> ()` | function |
| `Moss_ResetGPUQueryPool` | `GpuCommandBuffer::reset_query_pool(...)` | `pub fn reset_query_pool(&self, query_pool: &mut GpuQueryPool, first_query: u32, query_count: u32) -> ()` | function |
| `Moss_WriteGPUTimestamp` | `GpuCommandBuffer::write_timestamp(...)` | `pub fn write_timestamp(&self, query_pool: &mut GpuQueryPool, query_index: u32) -> ()` | function |
| `Moss_BeginGPUQuery` | `GpuCommandBuffer::begin_query(...)` | `pub fn begin_query(&mut self, query_pool: &mut GpuQueryPool, query_index: u32) -> ()` | function |
| `Moss_EndGPUQuery` | `GpuCommandBuffer::end_query(...)` | `pub fn end_query(&mut self, query_pool: &mut GpuQueryPool, query_index: u32) -> ()` | function |
| `Moss_GetGPUQueryResults` | `GpuQueryPool::get_query_results(...)` | `pub fn get_query_results(&self, first_query: u32, query_count: u32, data: &mut [u8], data_size: u64, stride: u64, flags: GpuQueryResultFlags) -> bool` | function |
| `Moss_TransitionGPUBuffer` | `GpuCommandBuffer::transition(...)` | `pub fn transition(&self, buffer: &mut GpuBuffer, old_state: ResourceState, new_state: ResourceState) -> ()` | function |
| `Moss_TransitionGPUTexture` | `GpuCommandBuffer::transition_texture(...)` | `pub fn transition_texture(&self, texture: &mut GpuTexture, old_state: ResourceState, new_state: ResourceState) -> ()` | function |
| `Moss_UnmapGPUTransferBuffer` | `GpuTransferBuffer::unmap(...)` | `pub fn unmap(&self) -> ()` | function |
| `Moss_UploadToGPUBuffer` | `GpuBuffer::upload_to(...)` | `pub fn upload_to(&mut self, src_data: &[u8], size: u64, dst_offset: u64) -> ()` | function |
| `Moss_UploadToGPUTexture` | `GpuTexture::upload_to(...)` | `pub fn upload_to(&mut self, transfer_info: &GpuTextureTransferInfo) -> ()` | function |
| `Moss_WaitForGPUFences` | `GpuDevice::wait_for_fences(...)` | `pub fn wait_for_fences(&self, fences: &[GpuFence*], wait_all: bool, timeout_ns: u64) -> ()` | function |
| `Moss_WaitForGPUIdle` | `GpuDevice::wait_for_idle(...)` | `pub fn wait_for_idle(&self) -> ()` | function |
| `Moss_WindowSupportsGPUPresentMode` | `Window::supports_present_mode(...)` | `pub fn supports_present_mode(&self, present_mode: GpuPresentMode) -> bool` | function |
| `Moss_GPUBlitInfo` | `GpuBlitInfo::default()` | `GpuBlitInfo::default() -> GpuBlitInfo` | constructor |
| `Moss_GPUBlitRegion` | `GpuBlitRegion::default()` | `GpuBlitRegion::default() -> GpuBlitRegion` | constructor |
| `Moss_GPUBufferBinding` | `GpuBufferBinding::default()` | `GpuBufferBinding::default() -> GpuBufferBinding` | constructor |
| `Moss_GPUBufferCreateInfo` | `GpuBufferCreateInfo::default()` | `GpuBufferCreateInfo::default() -> GpuBufferCreateInfo` | constructor |
| `Moss_GPUBufferLocation` | `GpuBufferLocation::default()` | `GpuBufferLocation::default() -> GpuBufferLocation` | constructor |
| `Moss_GPUBufferRegion` | `GpuBufferRegion::default()` | `GpuBufferRegion::default() -> GpuBufferRegion` | constructor |
| `Moss_GPUColorTargetBlendState` | `GpuColorTargetBlendState::default()` | `GpuColorTargetBlendState::default() -> GpuColorTargetBlendState` | constructor |
| `Moss_GPUColorTargetDescription` | `GpuColorTargetDescription::default()` | `GpuColorTargetDescription::default() -> GpuColorTargetDescription` | constructor |
| `Moss_GPUColorTargetInfo` | `GpuColorTargetInfo::default()` | `GpuColorTargetInfo::default() -> GpuColorTargetInfo` | constructor |
| `Moss_GPUDepthStencilState` | `GpuDepthStencilState::default()` | `GpuDepthStencilState::default() -> GpuDepthStencilState` | constructor |
| `Moss_GPUDepthStencilTargetInfo` | `GpuDepthStencilTargetInfo::default()` | `GpuDepthStencilTargetInfo::default() -> GpuDepthStencilTargetInfo` | constructor |
| `Moss_GPUIndexedIndirectDrawCommand` | `GpuIndexedIndirectDrawCommand::default()` | `GpuIndexedIndirectDrawCommand::default() -> GpuIndexedIndirectDrawCommand` | constructor |
| `Moss_GPUIndirectDispatchCommand` | `GpuIndirectDispatchCommand::default()` | `GpuIndirectDispatchCommand::default() -> GpuIndirectDispatchCommand` | constructor |
| `Moss_GPUIndirectDrawCommand` | `GpuIndirectDrawCommand::default()` | `GpuIndirectDrawCommand::default() -> GpuIndirectDrawCommand` | constructor |
| `Moss_GPUMultisampleState` | `GpuMultisampleState::default()` | `GpuMultisampleState::default() -> GpuMultisampleState` | constructor |
| `Moss_GPURasterizerState` | `GpuRasterizerState::default()` | `GpuRasterizerState::default() -> GpuRasterizerState` | constructor |
| `Moss_GPUSamplerCreateInfo` | `GpuSamplerCreateInfo::default()` | `GpuSamplerCreateInfo::default() -> GpuSamplerCreateInfo` | constructor |
| `Moss_GPUShaderCreateInfo` | `GpuShaderCreateInfo::default()` | `GpuShaderCreateInfo::default() -> GpuShaderCreateInfo` | constructor |
| `Moss_GPUStencilOpState` | `GpuStencilOpState::default()` | `GpuStencilOpState::default() -> GpuStencilOpState` | constructor |
| `Moss_GPUTextureCreateInfo` | `GpuTextureCreateInfo::default()` | `GpuTextureCreateInfo::default() -> GpuTextureCreateInfo` | constructor |
| `Moss_GPUTextureLocation` | `GpuTextureLocation::default()` | `GpuTextureLocation::default() -> GpuTextureLocation` | constructor |
| `Moss_GPUTextureRegion` | `GpuTextureRegion::default()` | `GpuTextureRegion::default() -> GpuTextureRegion` | constructor |
| `Moss_GPUTextureSamplerBinding` | `GpuTextureSamplerBinding::default()` | `GpuTextureSamplerBinding::default() -> GpuTextureSamplerBinding` | constructor |
| `Moss_GPUTextureTransferInfo` | `GpuTextureTransferInfo::default()` | `GpuTextureTransferInfo::default() -> GpuTextureTransferInfo` | constructor |
| `Moss_GPUTransferBufferCreateInfo` | `GpuTransferBufferCreateInfo::default()` | `GpuTransferBufferCreateInfo::default() -> GpuTransferBufferCreateInfo` | constructor |
| `Moss_GPUTransferBufferLocation` | `GpuTransferBufferLocation::default()` | `GpuTransferBufferLocation::default() -> GpuTransferBufferLocation` | constructor |
| `Moss_GPUVertexAttribute` | `GpuVertexAttribute::default()` | `GpuVertexAttribute::default() -> GpuVertexAttribute` | constructor |
| `Moss_GPUVertexBufferDescription` | `GpuVertexBufferDescription::default()` | `GpuVertexBufferDescription::default() -> GpuVertexBufferDescription` | constructor |
| `Moss_GPUVertexInputState` | `GpuVertexInputState::default()` | `GpuVertexInputState::default() -> GpuVertexInputState` | constructor |
| `Moss_GPUViewport` | `GpuViewport::default()` | `GpuViewport::default() -> GpuViewport` | constructor |
| `Moss_BindGPUComputeStorageBuffers` | `GpuCommandBuffer::bind_compute_storage_buffers(...)` | `pub fn bind_compute_storage_buffers(&mut self, first_slot: u32, bindings: &[GpuStorageBufferReadWriteBinding]) -> Result<()>` | function |
| `Moss_BindGPUComputeStorageTextures` | `GpuCommandBuffer::bind_compute_storage_textures(...)` | `pub fn bind_compute_storage_textures(&self, first_slot: u32, bindings: &[GpuStorageTextureReadWriteBinding]) -> ()` | function |
| `Moss_BindGPUFragmentStorageBuffers` | `GpuCommandBuffer::bind_fragment_storage_buffers(...)` | `pub fn bind_fragment_storage_buffers(&self, first_slot: u32, bindings: &[GpuStorageBufferReadWriteBinding]) -> ()` | function |
| `Moss_BindGPUFragmentStorageTextures` | `GpuCommandBuffer::bind_fragment_storage_textures(...)` | `pub fn bind_fragment_storage_textures(&self, first_slot: u32, bindings: &[GpuStorageTextureReadWriteBinding]) -> ()` | function |
| `Moss_BindGPUVertexStorageBuffers` | `GpuCommandBuffer::bind_vertex_storage_buffers(...)` | `pub fn bind_vertex_storage_buffers(&self, first_slot: u32, bindings: &[GpuStorageBufferReadWriteBinding]) -> ()` | function |
| `Moss_BindGPUVertexStorageTextures` | `GpuCommandBuffer::bind_vertex_storage_textures(...)` | `pub fn bind_vertex_storage_textures(&self, first_slot: u32, bindings: &[GpuStorageTextureReadWriteBinding]) -> ()` | function |
| `Moss_GetGPUDeviceDriver` | `GpuDevice::get_driver(...)` | `pub fn get_driver(&self) -> Option<String>` | function |
| `Moss_GetGPUDeviceProperties` | `GpuDevice::get_properties(...)` | `pub fn get_properties(&self) -> Result<GpuDeviceProperties>` | function |
| `Moss_ReleaseWindowFromGPUDevice` | `GpuDevice::release_window_from(...)` | `pub fn release_window_from(&self, window: &mut Window) -> ()` | function |
| `Moss_ClaimWindowForGPUDevice` | `GpuDevice::claim_window_for(...)` | `pub fn claim_window_for(&self, window: &mut Window) -> bool` | function |
| `Moss_UpdateGPUResourceSet` | `ResourceSet::update(...)` | `pub fn update(&mut self, bindings: &[GpuResourceBinding]) -> ()` | function |
| `Moss_CreateGPUDevice` | `gpu::create_device(...)` | `pub fn create_device(descriptor: &GpuDeviceDesc) -> Result<GpuDevice>` | function |
| `Moss_CreateGPUDeviceWithProperties` | `gpu::create_device_with_properties(...)` | `pub fn create_device_with_properties(descriptor: &GpuDeviceDesc, requested: &GpuDeviceProperties) -> Result<GpuDevice>` | function |
| `Moss_GPUBufferCreate` | `GpuDevice::create_buffer(...)` | `pub fn create_buffer(&self, descriptor: GpuBufferDescriptor) -> Result<GpuBuffer>` | function |
| `Moss_GPUBufferUpload` | `GpuBuffer::upload(...)` | `pub fn upload(&mut self, data: &[u8], offset: u64) -> Result<()>` | function |
| `Moss_GPUBufferMap` | `GpuBuffer::map(...)` | `pub fn map(&self) -> &mut [u8]` | function |
| `Moss_GPUBufferUnmap` | `GpuBuffer::unmap(...)` | `pub fn unmap(&self) -> ()` | function |
| `Moss_GPUBufferDestroy` | `GpuBuffer::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_AcquireGPUSwapchainTexture` | `GpuTexture::acquire_swapchain(...)` | `pub fn acquire_swapchain(&self, window: &mut Window) -> bool` | function |
| `Moss_AcquireGPUSwapchainTextureStatus` | `GpuTexture::acquire_swapchain_status(...)` | `pub fn acquire_swapchain_status(&self, window: &mut Window) -> GpuSwapchainResult` | function |
| `Moss_GetGPUSwapchainTextureFormat` | `GpuDevice::get_swapchain_texture_format(...)` | `pub fn get_swapchain_texture_format(&self, window: &mut Window) -> TextureFormat` | function |
| `Moss_SetGPUSwapchainParameters` | `GpuDevice::set_swapchain_parameters(...)` | `pub fn set_swapchain_parameters(&mut self, window: &mut Window, width: u32, height: u32, present_mode: GpuPresentMode) -> bool` | function |
| `Moss_ResizeGPUSwapchain` | `GpuDevice::resize_swapchain(...)` | `pub fn resize_swapchain(&self, window: &mut Window, width: u32, height: u32) -> GpuSwapchainResult` | function |
| `Moss_WaitAndAcquireGPUSwapchainTexture` | `GpuTexture::wait_and_acquire_swapchain(...)` | `pub fn wait_and_acquire_swapchain(&self, window: &mut Window, timeout_ns: u64) -> bool` | function |
| `Moss_WaitAndAcquireGPUSwapchainTextureStatus` | `GpuTexture::wait_and_acquire_swapchain_status(...)` | `pub fn wait_and_acquire_swapchain_status(&self, window: &mut Window, timeout_ns: u64) -> GpuSwapchainResult` | function |
| `Moss_WaitForGPUSwapchain` | `GpuDevice::wait_for_swapchain(...)` | `pub fn wait_for_swapchain(&self, window: &mut Window, timeout_ns: u64) -> bool` | function |
| `Moss_WindowSupportsGPUSwapchainComposition` | `Window::supports_swapchain_composition(...)` | `pub fn supports_swapchain_composition(&self) -> bool` | function |
| `Moss_PresentGPUSwapchain` | `GpuDevice::present_swapchain(...)` | `pub fn present_swapchain(&self, window: &mut Window, texture: &mut GpuTexture) -> bool` | function |
| `Moss_PresentGPUSwapchainStatus` | `GpuDevice::present_swapchain_status(...)` | `pub fn present_swapchain_status(&self, window: &mut Window, texture: &mut GpuTexture) -> GpuSwapchainResult` | function |
| `Moss_GPUClear` | `GpuCommandBuffer::clear(...)` | `pub fn clear(&self, r: f32, g: f32, b: f32, a: f32) -> ()` | function |
| `Moss_GPUSamplerCreate` | `GpuDevice::create_sampler(...)` | `pub fn create_sampler(&mut self, desc: &GpuSamplerDesc) -> Result<GpuSampler>` | function |
| `Moss_GPUSamplerDestroy` | `GpuSampler::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_TextureCreate` | `GpuDevice::create_texture(...)` | `pub fn create_texture(&mut self, desc: &TextureDesc) -> Result<Texture>` | function |
| `Moss_TextureDestroy` | `Texture::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_TextureUpload` | `Texture::upload(...)` | `pub fn upload(&mut self, desc: &TextureUploadDesc) -> ()` | function |
| `Moss_TextureGenerateMips` | `Texture::generate_mips(...)` | `pub fn generate_mips(&self) -> ()` | function |
| `Moss_GPUGetExternalFeatureMask` | `gpu::get_external_feature_mask(...)` | `pub fn get_external_feature_mask() -> u32` | function |
| `Moss_GPUExternalFeatureAvailable` | `gpu::external_feature_available(...)` | `pub fn external_feature_available(feature: GpuExternalFeature) -> bool` | function |
| `Moss_GPUGetExternalFeatures` | `gpu::get_external_features(...)` | `pub fn get_external_features(out_features: &mut GpuExternalFeatures) -> ()` | function |
| `Moss_GPUExternalFeatureName` | `gpu::external_feature_name(...)` | `pub fn external_feature_name(feature: GpuExternalFeature) -> Option<String>` | function |
| `Moss_FSR1BuildEasuConstants` | `gpu::fsr1_build_easu_constants(...)` | `pub fn fsr1_build_easu_constants(out_constants16: u32, input_viewport_width: f32, input_viewport_height: f32, input_width: f32, input_height: f32, output_width: f32, output_height: f32) -> ()` | function |
| `Moss_FSR2GetUpscaleRatio` | `gpu::fsr2_get_upscale_ratio(...)` | `pub fn fsr2_get_upscale_ratio(quality: FSR2QualityMode) -> f32` | function |
| `Moss_FSR2GetRenderResolution` | `gpu::fsr2_get_render_resolution(...)` | `pub fn fsr2_get_render_resolution(display_width: u32, display_height: u32, quality: FSR2QualityMode, out_resolution: &mut FSR2RenderResolution) -> bool` | function |
| `Moss_FSR2GetJitterPhaseCount` | `gpu::fsr2_get_jitter_phase_count(...)` | `pub fn fsr2_get_jitter_phase_count(render_width: i32, display_width: i32) -> i32` | function |
| `Moss_FSR2GetJitterOffset` | `gpu::fsr2_get_jitter_offset(...)` | `pub fn fsr2_get_jitter_offset(out_x: &mut &mut f32, out_y: &mut &mut f32, index: i32, phase_count: i32) -> bool` | function |
| `Moss_MeshGenerateVertexRemap` | `gpu::mesh_generate_vertex_remap(...)` | `pub fn mesh_generate_vertex_remap(destination: &mut &mut u32, indices: &[Uint32T], vertices: &[u8], vertex_count: usize, vertex_size: usize) -> usize` | function |
| `Moss_MeshOptimizeVertexCache` | `gpu::mesh_optimize_vertex_cache(...)` | `pub fn mesh_optimize_vertex_cache(destination: &mut &mut u32, indices: &[Uint32T], vertex_count: usize) -> bool` | function |
| `Moss_MeshOptimizeOverdraw` | `gpu::mesh_optimize_overdraw(...)` | `pub fn mesh_optimize_overdraw(destination: &mut &mut u32, indices: &[Uint32T], vertex_positions: &[Float], vertex_positions_stride: usize, threshold: f32) -> bool` | function |
| `Moss_MeshOptimizeVertexFetch` | `gpu::mesh_optimize_vertex_fetch(...)` | `pub fn mesh_optimize_vertex_fetch(destination: &mut [u8], indices: &mut &mut u32, index_count: usize, vertices: &[u8], vertex_count: usize, vertex_size: usize) -> usize` | function |
| `Moss_SMOLVEncode` | `gpu::smolv_encode(...)` | `pub fn smolv_encode(spirv_data: &[u8], spirv_size: usize, out_smolv_data: &mut [u8], inout_smolv_size: &mut usize, flags: u32) -> bool` | function |
| `Moss_SMOLVGetDecodedSize` | `gpu::smolv_get_decoded_size(...)` | `pub fn smolv_get_decoded_size(smolv_data: &[u8], smolv_size: usize) -> usize` | function |
| `Moss_SMOLVDecode` | `gpu::smolv_decode(...)` | `pub fn smolv_decode(smolv_data: &[u8], smolv_size: usize, out_spirv_data: &mut [u8], spirv_size: usize, flags: u32) -> bool` | function |
| `Moss_GPUVulkanMemoryAllocatorAvailable` | `gpu::memory_allocator_available(...)` | `pub fn memory_allocator_available() -> bool` | function |
| `Moss_GPUClothSolverCreate` | `GpuDevice::create_cloth_solver(...)` | `pub fn create_cloth_solver(&mut self, desc: &GpuClothSolverDesc) -> Result<GpuClothSolver>` | function |
| `Moss_GPUClothSolverDestroy` | `GpuClothSolver::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_GPUClothSolverDispatch` | `GpuCommandBuffer::cloth_solver_dispatch(...)` | `pub fn cloth_solver_dispatch(&self, solver: &mut GpuClothSolver, desc: &GpuClothDispatchDesc) -> bool` | function |
| `Moss_GPUCommandBufferBegin` | `GpuDevice::create_command_buffer(...)` | `pub fn create_command_buffer(&self, queue: CommandQueue) -> Result<GpuCommandBuffer>` | function |
| `Moss_GPUCommandBufferEnd` | `GpuCommandBuffer::end(...)` | `pub fn end(&mut self) -> ()` | function |
| `Moss_GPUCommandBufferSubmit` | `GpuCommandBuffer::submit(...)` | `pub fn submit(&mut self) -> ()` | function |
| `Moss_CmdBeginRenderPass` | `GpuCommandBuffer::cmd_begin_render_pass(...)` | `pub fn cmd_begin_render_pass(&mut self, fb: &mut Framebuffer) -> ()` | function |
| `Moss_CmdEndRenderPass` | `GpuCommandBuffer::cmd_end_render_pass(...)` | `pub fn cmd_end_render_pass(&mut self) -> ()` | function |
| `Moss_CmdBindPipeline` | `GpuCommandBuffer::cmd_bind_pipeline(...)` | `pub fn cmd_bind_pipeline(&self, pipeline: &mut PipelineState) -> ()` | function |
| `Moss_CmdBindResourceSet` | `GpuCommandBuffer::cmd_bind_resource_set(...)` | `pub fn cmd_bind_resource_set(&mut self, set_index: u32, set: &mut ResourceSet) -> ()` | function |
| `Moss_CmdBindVertexBuffer` | `GpuCommandBuffer::cmd_bind_vertex(...)` | `pub fn cmd_bind_vertex(&self, buffer: &mut GpuBuffer, offset: u64) -> ()` | function |
| `Moss_CmdBindIndexBuffer` | `GpuCommandBuffer::cmd_bind_index(...)` | `pub fn cmd_bind_index(&self, buffer: &mut GpuBuffer, offset: u64) -> ()` | function |
| `Moss_CmdDraw` | `GpuCommandBuffer::cmd_draw(...)` | `pub fn cmd_draw(&self, vertex_count: u32, first_vertex: u32) -> ()` | function |
| `Moss_CmdDrawIndexed` | `GpuCommandBuffer::cmd_draw_indexed(...)` | `pub fn cmd_draw_indexed(&self, index_count: u32, first_index: u32, vertex_offset: i32) -> ()` | function |
| `Moss_CmdDispatch` | `GpuCommandBuffer::cmd_dispatch(...)` | `pub fn cmd_dispatch(&self, x: u32, y: u32, z: u32) -> ()` | function |
| `Moss_RGPassReadTexture` | `RGPass::read_texture(...)` | `pub fn read_texture(&self, texture: &mut RGTexture, state: ResourceState) -> Result<RGTexture>` | function |
| `Moss_RGPassWriteTexture` | `RGPass::write_texture(...)` | `pub fn write_texture(&self, texture: &mut RGTexture, state: ResourceState) -> Result<RGTexture>` | function |
| `Moss_GPUDeviceSupportsAsyncCompute` | `GpuDevice::supports_async_compute(...)` | `pub fn supports_async_compute(&self) -> i32` | function |
| `Moss_TextureSetResidency` | `Texture::set_residency(...)` | `pub fn set_residency(&mut self, state: ResidencyState) -> ()` | function |
| `Moss_ShaderCreate` | `GpuDevice::create_shader(...)` | `pub fn create_shader(&mut self, desc: &ShaderDesc) -> Result<Shader>` | function |
| `Moss_ShaderDestroy` | `Shader::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_BindlessRegisterTexture` | `GpuDevice::bindless_register_texture(...)` | `pub fn bindless_register_texture(&self, texture: &mut [u8]) -> BindlessHandle` | function |
| `Moss_BindlessRegisterBuffer` | `GpuBuffer::bindless_register(...)` | `pub fn bindless_register(&self) -> BindlessHandle` | function |
| `Moss_BindlessUnregister` | `GpuDevice::bindless_unregister(...)` | `pub fn bindless_unregister(&self, handle: BindlessHandle) -> ()` | function |
| `Moss_PipelineCreate` | `GpuDevice::create_pipeline(...)` | `pub fn create_pipeline(&mut self, desc: &PipelineDesc) -> Result<PipelineState>` | function |
| `Moss_PipelineDestroy` | `GpuDevice::pipeline_destroy(...)` | `pub fn pipeline_destroy(&mut self, pipeline: &mut PipelineState) -> ()` | function |
| `Moss_WindowResizeVK` | `gpu::window_resize_vk(...)` | `pub fn window_resize_vk(physical_device: VkPhysicalDevice, device: VkDevice, swapchain: VkSwapchainKHR, swapchain_extent: VkExtentDIMENSION2D, swapchain_framebuffers: StdVector<VkFramebuffer>, swapchain_image_views: StdVector<VkImageView>, depth_image: VkImage, depth_image_memory: VkDeviceMemory, depth_image_view: VkImageView) -> ()` | function |
| `Moss_GPUFatalError` | `gpu::fatal_error(...)` | `pub fn fatal_error(result: MossResult) -> ()` | function |
| `Moss_SetGPUViewports` | `GpuCommandBuffer::set_viewports(...)` | `pub fn set_viewports(&mut self, viewports: &[GpuViewport]) -> ()` | function |
| `Moss_SetGPUScissors` | `GpuCommandBuffer::set_scissors(...)` | `pub fn set_scissors(&mut self, scissors: &[Rect]) -> ()` | function |
| `Moss_ClearGPUTexture` | `GpuCommandBuffer::clear_texture(...)` | `pub fn clear_texture(&self, texture: &mut GpuTexture, color: f32) -> ()` | function |
| `Moss_ClearGPUDepthStencilTexture` | `GpuCommandBuffer::clear_depth_stencil_texture(...)` | `pub fn clear_depth_stencil_texture(&self, texture: &mut GpuTexture, depth: f32, stencil: u8) -> ()` | function |
| `Moss_DrawGPUIndexedPrimitivesIndirectCount` | `GpuCommandBuffer::draw_indexed_primitives_indirect_count(...)` | `pub fn draw_indexed_primitives_indirect_count(&self, indirect_buffer: &mut GpuBuffer, offset: u64, count_buffer: &mut GpuBuffer, count_offset: u64, max_draw_count: u32, stride: u32) -> ()` | function |
| `Moss_DrawGPUPrimitivesIndirectCount` | `GpuCommandBuffer::draw_primitives_indirect_count(...)` | `pub fn draw_primitives_indirect_count(&self, indirect_buffer: &mut GpuBuffer, offset: u64, count_buffer: &mut GpuBuffer, count_offset: u64, max_draw_count: u32, stride: u32) -> ()` | function |
| `Moss_SetGPUDepthBias` | `GpuCommandBuffer::set_depth_bias(...)` | `pub fn set_depth_bias(&mut self, constant_factor: f32, clamp: f32, slope_factor: f32) -> ()` | function |
| `Moss_CreateGPUSemaphore` | `GpuDevice::create_semaphore(...)` | `pub fn create_semaphore(&mut self) -> Result<GpuSemaphore>` | function |
| `Moss_ReleaseGPUSemaphore` | `GpuSemaphore::release(...)` | `pub fn release(&self) -> ()` | function |
| `Moss_SignalGPUSemaphoreValue` | `GpuSemaphore::signal_value(...)` | `pub fn signal_value(&self, value: u64) -> ()` | function |
| `Moss_WaitGPUSemaphoreValue` | `GpuSemaphore::wait_value(...)` | `pub fn wait_value(&self, value: u64, timeout_ns: u64) -> bool` | function |
| `Moss_SubmitGPUCommandBufferWithSemaphores` | `GpuCommandBuffer::submit_with_semaphores(...)` | `pub fn submit_with_semaphores(&mut self, wait_semaphores: &GpuSemaphore, wait_values: &[Uint64T], signal_semaphores: &GpuSemaphore, signal_values: &[Uint64T]) -> ()` | function |
| `Moss_GetGPUBindlessCapacity` | `GpuDevice::get_bindless_capacity(...)` | `pub fn get_bindless_capacity(&self, type: ShaderResourceType) -> u32` | function |
| `Moss_CreateGPUPipelineCache` | `GpuDevice::create_pipeline_cache(...)` | `pub fn create_pipeline_cache(&mut self, initial_data: &[u8], initial_size: usize) -> Result<GpuPipelineCache>` | function |
| `Moss_GetGPUPipelineCacheData` | `GpuPipelineCache::get_data(...)` | `pub fn get_data(&self, out_data: &mut [u8], out_size: usize) -> usize` | function |
| `Moss_ReleaseGPUPipelineCache` | `GpuPipelineCache::release(...)` | `pub fn release(&self) -> ()` | function |

### `Moss_GUI.h` → `moss::gui`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `CreateContext` | `gui::create_context(...)` | `pub fn create_context(shared_font_atlas: &mut FontAtlas) -> Result<GuiContext>` | function |
| `DestroyContext` | `GuiContext::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `GetCurrentContext` | `gui::get_current_context(...)` | `pub fn get_current_context() -> Result<GuiContext>` | function |
| `SetCurrentContext` | `GuiContext::set_current(...)` | `pub fn set_current(&mut self) -> ()` | function |
| `BeginWindow` | `GuiContext::begin_window(...)` | `pub fn begin_window(&mut self, name: &str, p_open: &mut bool, flags: GuiWindowFlags) -> bool` | function |
| `EndWindow` | `GuiContext::end_window(...)` | `pub fn end_window(&mut self) -> ()` | function |
| `BeginWindowChild` (overload 1) | `GuiContext::begin_window_child(...)` | `pub fn begin_window_child(&mut self, str_id: &str, size: Float2, child_flags: GuiChildFlags, window_flags: GuiWindowFlags) -> bool` | function |
| `BeginWindowChild` (overload 2) | `GuiContext::begin_window_child(...)` | `pub fn begin_window_child(&mut self, id: u32, size: Float2, child_flags: GuiChildFlags, window_flags: GuiWindowFlags) -> bool` | function |
| `EndWindowChild` | `GuiContext::end_window_child(...)` | `pub fn end_window_child(&mut self) -> ()` | function |
| `BeginContainer` | `GuiContext::begin_container(...)` | `pub fn begin_container(&mut self, rect: Rect) -> ()` | function |
| `EndContainer` | `GuiContext::end_container(...)` | `pub fn end_container(&mut self) -> ()` | function |
| `BeginVContainer` | `GuiContext::begin_v_container(...)` | `pub fn begin_v_container(&mut self, rect: Rect, spacing: f32) -> ()` | function |
| `EndVContainer` | `GuiContext::end_v_container(...)` | `pub fn end_v_container(&mut self) -> ()` | function |
| `BeginHContainer` | `GuiContext::begin_h_container(...)` | `pub fn begin_h_container(&mut self, rect: Rect, spacing: f32) -> ()` | function |
| `EndHContainer` | `GuiContext::end_h_container(...)` | `pub fn end_h_container(&mut self) -> ()` | function |
| `BeginCenterContainer` | `GuiContext::begin_center_container(...)` | `pub fn begin_center_container(&mut self, rect: Rect) -> ()` | function |
| `EndCenterContainer` | `GuiContext::end_center_container(...)` | `pub fn end_center_container(&mut self) -> ()` | function |
| `BeginMarginContainer` | `GuiContext::begin_margin_container(...)` | `pub fn begin_margin_container(&mut self, left_margin: f32, right_margin: f32, top_margin: f32, bottom_margin: f32) -> ()` | function |
| `EndMarginContainer` | `GuiContext::end_margin_container(...)` | `pub fn end_margin_container(&mut self) -> ()` | function |
| `BeginVScrollContainer` | `GuiContext::begin_v_scroll_container(...)` | `pub fn begin_v_scroll_container(&mut self) -> ()` | function |
| `EndVScrollContainer` | `GuiContext::end_v_scroll_container(...)` | `pub fn end_v_scroll_container(&mut self) -> ()` | function |
| `BeginHScrollContainer` | `GuiContext::begin_h_scroll_container(...)` | `pub fn begin_h_scroll_container(&mut self) -> ()` | function |
| `EndHScrollContainer` | `GuiContext::end_h_scroll_container(...)` | `pub fn end_h_scroll_container(&mut self) -> ()` | function |
| `BeginGridContainer` | `GuiContext::begin_grid_container(...)` | `pub fn begin_grid_container(&mut self) -> ()` | function |
| `EndGridContainer` | `GuiContext::end_grid_container(...)` | `pub fn end_grid_container(&mut self) -> ()` | function |
| `ColorRect` | `GuiContext::color_rect(...)` | `pub fn color_rect(&self, color: Color, size: Float2) -> ()` | function |
| `Text` | `GuiContext::text(...)` | `pub fn text(&self, fmt: &str, args: ...) -> ()` | function |
| `RichText` | `GuiContext::rich_text(...)` | `pub fn rich_text(&self, label: &str) -> ()` | function |
| `ProgressBar` | `GuiContext::progress_bar(...)` | `pub fn progress_bar(&self) -> ()` | function |
| `TextLink` | `GuiContext::text_link(...)` | `pub fn text_link(&self, label: &str) -> bool` | function |
| `TextLinkOpenURL` | `GuiContext::text_link_open_url(...)` | `pub fn text_link_open_url(&self, label: &str, url: &str) -> ()` | function |
| `Button` | `GuiContext::button(...)` | `pub fn button(&self, label: &str, rect_ptr: &Rect, vp_matrix: Mat44, default_width: f32, default_height: f32) -> bool` | function |
| `ToggleSwitch` | `GuiContext::toggle_switch(...)` | `pub fn toggle_switch(&self, active: bool) -> bool` | function |
| `RadioButton` | `GuiContext::radio_button(...)` | `pub fn radio_button(&self, label: &str, active: bool) -> bool` | function |
| `Checkbox` | `GuiContext::checkbox(...)` | `pub fn checkbox(&self, label: &str, value: &mut bool, rect_ptr: &Rect, vp_matrix: Mat44, default_size: f32) -> bool` | function |
| `SliderFloat` | `GuiContext::slider_float(...)` | `pub fn slider_float(&self, min_value: f32, max_value: f32, value: f32) -> ()` | function |
| `SliderDouble` | `GuiContext::slider_double(...)` | `pub fn slider_double(&self, min_value: f64, max_value: f64, value: f64) -> ()` | function |
| `SliderInt` | `GuiContext::slider_int(...)` | `pub fn slider_int(&self, min_value: i32, max_value: i32, value: i32) -> ()` | function |
| `SliderFloat2` | `GuiContext::slider_float2(...)` | `pub fn slider_float2(&self, min_value: Float2, max_value: Float2, value: Float2) -> ()` | function |
| `SliderDouble2` | `GuiContext::slider_double2(...)` | `pub fn slider_double2(&self, min_value: Double2, max_value: Double2, value: Double2) -> ()` | function |
| `SliderInt2` | `GuiContext::slider_int2(...)` | `pub fn slider_int2(&self, min_value: Int2, max_value: Int2, value: Int2) -> ()` | function |
| `SliderFloat3` | `GuiContext::slider_float3(...)` | `pub fn slider_float3(&self, min_value: Float3, max_value: Float3, value: Float3) -> ()` | function |
| `SliderDouble3` | `GuiContext::slider_double3(...)` | `pub fn slider_double3(&self, min_value: Double3, max_value: Double3, value: Double3) -> ()` | function |
| `SliderInt3` | `GuiContext::slider_int3(...)` | `pub fn slider_int3(&self, min_value: Int3, max_value: Int3, value: Int3) -> ()` | function |
| `SliderFloat4` | `GuiContext::slider_float4(...)` | `pub fn slider_float4(&self, min_value: Float4, max_value: Float4, value: Float4) -> ()` | function |
| `SliderDouble4` | `GuiContext::slider_double4(...)` | `pub fn slider_double4(&self, min_value: Double4, max_value: Double4, value: Double4) -> ()` | function |
| `SliderInt4` | `GuiContext::slider_int4(...)` | `pub fn slider_int4(&self, min_value: Int4, max_value: Int4, value: Int4) -> ()` | function |
| `DragFloat` | `GuiContext::drag_float(...)` | `pub fn drag_float(&self, label: &str, value: &mut &mut f32, v_speed: f32, v_min: f32, v_max: f32, format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragFloat2` | `GuiContext::drag_float2(...)` | `pub fn drag_float2(&self, label: &str, value: Float2, v_speed: f32, v_min: f32, v_max: f32, format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragFloat3` | `GuiContext::drag_float3(...)` | `pub fn drag_float3(&self, label: &str, value: Float3, v_speed: f32, v_min: f32, v_max: f32, format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragFloat4` | `GuiContext::drag_float4(...)` | `pub fn drag_float4(&self, label: &str, value: Float4, v_speed: f32, v_min: f32, v_max: f32, format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragFloatRange2` | `GuiContext::drag_float_range2(...)` | `pub fn drag_float_range2(&self, label: &str, v_current_min: &mut &mut f32, v_current_max: &mut &mut f32, v_speed: f32, v_min: f32, v_max: f32, format: &str, format_max: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragInt` | `GuiContext::drag_int(...)` | `pub fn drag_int(&self, label: &str, v: &mut &mut i32, v_speed: f32, v_min: i32, v_max: i32, format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragInt2` | `GuiContext::drag_int2(...)` | `pub fn drag_int2(&self, label: &str, v: i32, v_speed: f32, v_min: i32, v_max: i32, format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragInt3` | `GuiContext::drag_int3(...)` | `pub fn drag_int3(&self, label: &str, v: i32, v_speed: f32, v_min: i32, v_max: i32, format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragInt4` | `GuiContext::drag_int4(...)` | `pub fn drag_int4(&self, label: &str, v: i32, v_speed: f32, v_min: i32, v_max: i32, format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragIntRange2` | `GuiContext::drag_int_range2(...)` | `pub fn drag_int_range2(&self, label: &str, v_current_min: &mut &mut i32, v_current_max: &mut &mut i32, v_speed: f32, v_min: i32, v_max: i32, format: &str, format_max: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragScalar` | `GuiContext::drag_scalar(...)` | `pub fn drag_scalar(&self, label: &str, data_type: GuiDataType, p_data: &mut [u8], v_speed: f32, p_min: &[u8], p_max: &[u8], format: &str, flags: GuiSliderFlags) -> bool` | function |
| `DragScalarN` | `GuiContext::drag_scalar_n(...)` | `pub fn drag_scalar_n(&self, label: &str, data_type: GuiDataType, p_data: &mut [u8], components: i32, v_speed: f32, p_min: &[u8], p_max: &[u8], format: &str, flags: GuiSliderFlags) -> bool` | function |
| `InputText` | `GuiContext::input_text(...)` | `pub fn input_text(&self, label: &str, buf: &mut [u8], buf_size: usize, flags: GuiInputTextFlags, callback: GuiInputTextCallback) -> bool` | function |
| `InputTextMultiline` | `GuiContext::input_text_multiline(...)` | `pub fn input_text_multiline(&self, label: &str, buf: &mut [u8], buf_size: usize, size: Float2, flags: GuiInputTextFlags, callback: GuiInputTextCallback) -> bool` | function |
| `InputTextWithHint` | `GuiContext::input_text_with_hint(...)` | `pub fn input_text_with_hint(&self, label: &str, hint: &str, buf: &mut [u8], buf_size: usize, flags: GuiInputTextFlags, callback: GuiInputTextCallback) -> bool` | function |
| `InputFloat` | `GuiContext::input_float(...)` | `pub fn input_float(&self, label: &str, v: &mut &mut f32, step: f32, step_fast: f32, format: &str, flags: GuiInputTextFlags) -> bool` | function |
| `InputFloat2` | `GuiContext::input_float2(...)` | `pub fn input_float2(&self, label: &str, v: f32, format: &str, flags: GuiInputTextFlags) -> bool` | function |
| `InputFloat3` | `GuiContext::input_float3(...)` | `pub fn input_float3(&self, label: &str, v: f32, format: &str, flags: GuiInputTextFlags) -> bool` | function |
| `InputFloat4` | `GuiContext::input_float4(...)` | `pub fn input_float4(&self, label: &str, v: f32, format: &str, flags: GuiInputTextFlags) -> bool` | function |
| `InputInt` | `GuiContext::input_int(...)` | `pub fn input_int(&self, label: &str, v: &mut &mut i32, step: i32, step_fast: i32, flags: GuiInputTextFlags) -> bool` | function |
| `InputInt2` | `GuiContext::input_int2(...)` | `pub fn input_int2(&self, label: &str, v: i32, flags: GuiInputTextFlags) -> bool` | function |
| `InputInt3` | `GuiContext::input_int3(...)` | `pub fn input_int3(&self, label: &str, v: i32, flags: GuiInputTextFlags) -> bool` | function |
| `InputInt4` | `GuiContext::input_int4(...)` | `pub fn input_int4(&self, label: &str, v: i32, flags: GuiInputTextFlags) -> bool` | function |
| `InputDouble` | `GuiContext::input_double(...)` | `pub fn input_double(&self, label: &str, v: &mut &mut f64, step: f64, step_fast: f64, format: &str, flags: GuiInputTextFlags) -> bool` | function |
| `InputScalar` | `GuiContext::input_scalar(...)` | `pub fn input_scalar(&self, label: &str, data_type: GuiDataType, p_data: &mut [u8], p_step: &[u8], p_step_fast: &[u8], format: &str, flags: GuiInputTextFlags) -> bool` | function |
| `InputScalarN` | `GuiContext::input_scalar_n(...)` | `pub fn input_scalar_n(&self, label: &str, data_type: GuiDataType, p_data: &mut [u8], components: i32, p_step: &[u8], p_step_fast: &[u8], format: &str, flags: GuiInputTextFlags) -> bool` | function |

### `Moss_Network.h` → `moss::network`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `Moss_Init_Network` | `network::initialize(...)` | `pub fn initialize() -> Result<()>` | function |
| `enet_initialize_with_callbacks` | `NetworkCallbacks::initialize_with(...)` | `pub fn initialize_with(&self) -> i32` | function |
| `Moss_TerminateNetwork` | `network::shutdown(...)` | `pub fn shutdown() -> ()` | function |
| `enet_list_clear` | `NetworkList::clear(...)` | `pub fn clear(&self) -> ()` | function |
| `enet_list_insert` | `network::list_insert(...)` | `pub fn list_insert(net_list_iterator: E, value: &mut [u8]) -> NetworkListIterator` | function |
| `enet_list_remove` | `network::list_remove(...)` | `pub fn list_remove(net_list_iterator: E) -> &mut [u8]` | function |
| `enet_list_move` | `network::list_move(...)` | `pub fn list_move(net_list_iterator: E, value: &mut [u8], value: &mut [u8]) -> NetworkListIterator` | function |
| `enet_list_size` | `NetworkList::size(...)` | `pub fn size(&self) -> usize` | function |
| `enet_time_get` | `network::time_get(...)` | `pub fn time_get() -> u32` | function |
| `enet_time_set` | `network::time_set(...)` | `pub fn time_set(int32_t: U) -> ()` | function |
| `enet_socket_create` | `network::create_socket(...)` | `pub fn create_socket(adress_type: NetworkAddressType, socket_type: NetworkSocketType) -> NetworkSocket` | function |
| `enet_socket_bind` | `NetworkAddress::socket_bind(...)` | `pub fn socket_bind(&self, net_socket: E) -> i32` | function |
| `enet_socket_get_address` | `NetworkAddress::socket_get(...)` | `pub fn socket_get(&self, net_socket: E) -> i32` | function |
| `enet_socket_listen` | `network::socket_listen(...)` | `pub fn socket_listen(net_socket: E, nt: I) -> i32` | function |
| `enet_socket_accept` | `NetworkAddress::socket_accept(...)` | `pub fn socket_accept(&self, net_socket: E) -> NetworkSocket` | function |
| `enet_socket_connect` | `NetworkAddress::socket_connect(...)` | `pub fn socket_connect(&self, net_socket: E) -> i32` | function |
| `enet_socket_send` | `NetworkAddress::socket_send(...)` | `pub fn socket_send(&self, net_socket: E, value: &NetworkBuffer, ize_t: S) -> i32` | function |
| `enet_socket_receive` | `NetworkAddress::socket_receive(...)` | `pub fn socket_receive(&self, net_socket: E, value: &mut NetworkBuffer, ize_t: S) -> i32` | function |
| `enet_socket_wait` | `network::socket_wait(...)` | `pub fn socket_wait(net_socket: E, value: &mut &mut u32, int32_t: U) -> i32` | function |
| `enet_socket_set_option` | `network::socket_set_option(...)` | `pub fn socket_set_option(net_socket: E, net_socket_option: E, nt: I) -> i32` | function |
| `enet_socket_get_option` | `network::socket_get_option(...)` | `pub fn socket_get_option(net_socket: E, net_socket_option: E, value: &mut &mut i32) -> i32` | function |
| `enet_socket_shutdown` | `network::socket_shutdown(...)` | `pub fn socket_shutdown(net_socket: E, net_socket_shutdown: E) -> i32` | function |
| `enet_socket_destroy` | `network::socket_destroy(...)` | `pub fn socket_destroy(net_socket: E) -> ()` | function |
| `enet_socketset_select` | `NetworkSocketSet::socketset_select(...)` | `pub fn socketset_select(&self, net_socket: E, value: &mut NetworkSocketSet, int32_t: U) -> i32` | function |
| `enet_address_set_host_ip` | `NetworkAddress::set_host_ip(...)` | `pub fn set_host_ip(&mut self, host_name: &str) -> i32` | function |
| `enet_address_set_host` | `NetworkAddress::set_host(...)` | `pub fn set_host(&mut self, type: NetworkAddressType, host_name: &str) -> i32` | function |
| `enet_address_get_host_ip` | `NetworkAddress::get_host_ip(...)` | `pub fn get_host_ip(&self, host_name: &mut [u8], name_length: usize) -> i32` | function |
| `enet_address_get_host` | `NetworkAddress::get_host(...)` | `pub fn get_host(&self, host_name: &mut [u8], name_length: usize) -> i32` | function |
| `enet_address_equal_host` | `NetworkAddress::equal_host(...)` | `pub fn equal_host(&self, second_address: &NetworkAddress) -> i32` | function |
| `enet_address_is_broadcast` | `NetworkAddress::is_broadcast(...)` | `pub fn is_broadcast(&self) -> i32` | function |
| `enet_address_build_any` | `NetworkAddress::build_any(...)` | `pub fn build_any(&self, type: NetworkAddressType) -> ()` | function |
| `enet_address_build_loopback` | `NetworkAddress::build_loopback(...)` | `pub fn build_loopback(&self, type: NetworkAddressType) -> ()` | function |
| `enet_address_convert_ipv6` | `NetworkAddress::convert_ipv6(...)` | `pub fn convert_ipv6(&self) -> ()` | function |
| `enet_crc32` | `NetworkBuffer::crc32(...)` | `pub fn crc32(&self, ize_t: S) -> u32` | function |
| `enet_host_create` | `NetworkAddress::create_host(...)` | `pub fn create_host(&mut self, type: NetworkAddressType, ize_t: S, ize_t: S, int32_t: U, int32_t: U) -> Result<NetworkHost>` | function |
| `enet_host_destroy` | `NetworkHost::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `enet_host_check_events` | `NetworkHost::check_events(...)` | `pub fn check_events(&self, value: &mut NetworkEvent) -> i32` | function |
| `enet_host_service` | `NetworkHost::service(...)` | `pub fn service(&self, value: &mut NetworkEvent, int32_t: U) -> i32` | function |
| `enet_host_flush` (overload 1) | `NetworkHost::flush(...)` | `pub fn flush(&self) -> ()` | function |
| `enet_host_broadcast` (overload 1) | `NetworkHost::broadcast(...)` | `pub fn broadcast(&self, int8_t: U, value: &mut NetworkPacket) -> ()` | function |
| `enet_host_compress` | `NetworkHost::compress(...)` | `pub fn compress(&self, value: &NetworkCompressor) -> ()` | function |
| `enet_host_compress_with_range_coder` | `NetworkHost::compress_with_range_coder(...)` | `pub fn compress_with_range_coder(&self) -> i32` | function |
| `enet_host_channel_limit` | `NetworkHost::channel_limit(...)` | `pub fn channel_limit(&self, ize_t: S) -> ()` | function |
| `enet_host_bandwidth_limit` | `NetworkHost::bandwidth_limit(...)` | `pub fn bandwidth_limit(&self, int32_t: U, int32_t: U) -> ()` | function |
| `enet_host_bandwidth_throttle` | `NetworkHost::bandwidth_throttle(...)` | `pub fn bandwidth_throttle(&self) -> ()` | function |
| `enet_host_random_seed` | `network::host_random_seed(...)` | `pub fn host_random_seed() -> u32` | function |
| `enet_host_random` | `NetworkHost::random(...)` | `pub fn random(&self) -> u32` | function |
| `enet_host_connect` (overload 1) | `NetworkHost::connect(...)` | `pub fn connect(&self, value: &NetworkAddress, ize_t: S, int32_t: U) -> Result<NetworkPeer>` | function |
| `enet_peer_send` | `NetworkPeer::send(...)` | `pub fn send(&self, int8_t: U, value: &mut NetworkPacket) -> i32` | function |
| `enet_peer_ping` (overload 1) | `NetworkPeer::ping(...)` | `pub fn ping(&self) -> ()` | function |
| `enet_peer_ping_interval` | `NetworkPeer::ping_interval(...)` | `pub fn ping_interval(&self, int32_t: U) -> ()` | function |
| `enet_peer_timeout` (overload 1) | `NetworkPeer::timeout(...)` | `pub fn timeout(&self, int32_t: U, int32_t: U, int32_t: U) -> ()` | function |
| `enet_peer_reset` (overload 1) | `NetworkPeer::reset(...)` | `pub fn reset(&self) -> ()` | function |
| `enet_peer_disconnect` (overload 1) | `NetworkPeer::disconnect(...)` | `pub fn disconnect(&self, int32_t: U) -> ()` | function |
| `enet_peer_disconnect_now` (overload 1) | `NetworkPeer::disconnect_now(...)` | `pub fn disconnect_now(&self, int32_t: U) -> ()` | function |
| `enet_peer_disconnect_later` | `NetworkPeer::disconnect_later(...)` | `pub fn disconnect_later(&self, int32_t: U) -> ()` | function |
| `enet_peer_throttle_configure` | `NetworkPeer::throttle_configure(...)` | `pub fn throttle_configure(&self, int32_t: U, int32_t: U, int32_t: U) -> ()` | function |
| `enet_peer_throttle` | `NetworkPeer::throttle(...)` | `pub fn throttle(&self, int32_t: U) -> i32` | function |
| `enet_peer_reset_queues` | `NetworkPeer::reset_queues(...)` | `pub fn reset_queues(&self) -> ()` | function |
| `enet_peer_has_outgoing_commands` | `NetworkPeer::has_outgoing_commands(...)` | `pub fn has_outgoing_commands(&self) -> i32` | function |
| `enet_peer_setup_outgoing_command` | `NetworkOutgoingCommand::peer_setup(...)` | `pub fn peer_setup(&self, value: &mut NetworkPeer) -> ()` | function |
| `enet_peer_queue_outgoing_command` | `NetworkPeer::queue_outgoing_command(...)` | `pub fn queue_outgoing_command(&self, value: &NetworkProtocol, value: &mut NetworkPacket, int32_t: U, int16_t: U) -> Result<NetworkOutgoingCommand>` | function |
| `enet_peer_queue_incoming_command` | `NetworkPeer::queue_incoming_command(...)` | `pub fn queue_incoming_command(&self, value: &NetworkProtocol, value: &[u8], ize_t: S, int32_t: U, int32_t: U) -> Result<NetworkIncomingCommand>` | function |
| `enet_peer_queue_acknowledgement` | `NetworkPeer::queue_acknowledgement(...)` | `pub fn queue_acknowledgement(&self, value: &NetworkProtocol, int16_t: U) -> Result<NetworkAcknowledgement>` | function |
| `enet_peer_dispatch_incoming_unreliable_commands` | `NetworkIncomingCommand::peer_dispatch_incoming_unreliable_commands(...)` | `pub fn peer_dispatch_incoming_unreliable_commands(&self, value: &mut NetworkPeer, value: &mut NetworkChannel) -> ()` | function |
| `enet_peer_dispatch_incoming_reliable_commands` | `NetworkIncomingCommand::peer_dispatch_incoming_reliable_commands(...)` | `pub fn peer_dispatch_incoming_reliable_commands(&self, value: &mut NetworkPeer, value: &mut NetworkChannel) -> ()` | function |
| `enet_peer_on_connect` | `NetworkPeer::on_connect(...)` | `pub fn on_connect(&self) -> ()` | function |
| `enet_peer_on_disconnect` | `NetworkPeer::on_disconnect(...)` | `pub fn on_disconnect(&self) -> ()` | function |
| `enet_packet_create` | `network::create_packet(...)` | `pub fn create_packet(value: &[u8], ize_t: S, int32_t: U) -> Result<NetworkPacket>` | function |
| `enet_packet_destroy` | `NetworkPacket::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `enet_packet_resize` | `NetworkPacket::resize(...)` | `pub fn resize(&self, ize_t: S) -> i32` | function |
| `enet_peer_receive` | `NetworkPeer::receive(...)` | `pub fn receive(&self, channel_id: &mut [u8]) -> Result<NetworkPacket>` | function |
| `enet_range_coder_create` | `network::create_range_coder(...)` | `pub fn create_range_coder() -> &mut [u8]` | function |
| `enet_range_coder_destroy` | `network::range_coder_destroy(...)` | `pub fn range_coder_destroy(value: &mut [u8]) -> ()` | function |
| `enet_range_coder_compress` | `NetworkBuffer::range_coder_compress(...)` | `pub fn range_coder_compress(&self, value: &mut [u8], ize_t: S, ize_t: S, value: &mut [u8], ize_t: S) -> usize` | function |
| `enet_range_coder_decompress` | `network::range_coder_decompress(...)` | `pub fn range_coder_decompress(value: &mut [u8], value: &[u8], ize_t: S, value: &mut [u8], ize_t: S) -> usize` | function |
| `enet_protocol_command_size` | `network::protocol_command_size(...)` | `pub fn protocol_command_size(int8_t: U) -> usize` | function |
| `enet_packet_get_data` | `NetworkPacket::get_data(...)` | `pub fn get_data(&self) -> &mut [u8]` | function |
| `enet_packet_get_user_data` | `NetworkPacket::get_user_data(...)` | `pub fn get_user_data(&self) -> &mut [u8]` | function |
| `enet_packet_set_user_data` | `NetworkPacket::set_user_data(...)` | `pub fn set_user_data(&mut self) -> ()` | function |
| `enet_packet_get_length` | `NetworkPacket::get_length(...)` | `pub fn get_length(&self) -> i32` | function |
| `enet_packet_set_acknowledge_callback` | `NetworkPacket::set_acknowledge_callback(...)` | `pub fn set_acknowledge_callback(&mut self, net_packet_acknowledged_callback: E) -> ()` | function |
| `enet_packet_set_free_callback` | `NetworkPacket::set_free_callback(...)` | `pub fn set_free_callback(&mut self, net_packet_free_callback: E) -> ()` | function |
| `enet_packet_check_references` | `NetworkPacket::check_references(...)` | `pub fn check_references(&self) -> i32` | function |
| `enet_packet_dispose` | `NetworkPacket::dispose(...)` | `pub fn dispose(&self) -> ()` | function |
| `enet_host_get_peers_count` | `NetworkHost::get_peers_count(...)` | `pub fn get_peers_count(&self) -> u32` | function |
| `enet_host_get_packets_sent` | `NetworkHost::get_packets_sent(...)` | `pub fn get_packets_sent(&self) -> u32` | function |
| `enet_host_get_packets_received` | `NetworkHost::get_packets_received(...)` | `pub fn get_packets_received(&self) -> u32` | function |
| `enet_host_get_bytes_sent` | `NetworkHost::get_bytes_sent(...)` | `pub fn get_bytes_sent(&self) -> u32` | function |
| `enet_host_get_bytes_received` | `NetworkHost::get_bytes_received(...)` | `pub fn get_bytes_received(&self) -> u32` | function |
| `enet_host_set_max_duplicate_peers` | `NetworkHost::set_max_duplicate_peers(...)` | `pub fn set_max_duplicate_peers(&mut self, int16_t: U) -> ()` | function |
| `enet_host_set_intercept_callback` | `NetworkHost::set_intercept_callback(...)` | `pub fn set_intercept_callback(&mut self, net_intercept_callback: E) -> ()` | function |
| `enet_host_set_checksum_callback` | `NetworkHost::set_checksum_callback(...)` | `pub fn set_checksum_callback(&mut self, net_checksum_callback: E) -> ()` | function |
| `enet_peer_get_id` | `NetworkPeer::get_id(...)` | `pub fn get_id(&self) -> u32` | function |
| `enet_peer_get_ip` | `NetworkPeer::get_ip(...)` | `pub fn get_ip(&self, value: &mut [u8], ize_t: S) -> i32` | function |
| `enet_peer_get_port` | `NetworkPeer::get_port(...)` | `pub fn get_port(&self) -> u16` | function |
| `enet_peer_get_mtu` | `NetworkPeer::get_mtu(...)` | `pub fn get_mtu(&self) -> u32` | function |
| `enet_peer_get_state` | `NetworkPeer::get_state(...)` | `pub fn get_state(&self) -> NetworkPeerState` | function |
| `enet_peer_get_rtt` | `NetworkPeer::get_rtt(...)` | `pub fn get_rtt(&self) -> u32` | function |
| `enet_peer_get_last_rtt` | `NetworkPeer::get_last_rtt(...)` | `pub fn get_last_rtt(&self) -> u32` | function |
| `enet_peer_get_lastsendtime` | `NetworkPeer::get_lastsendtime(...)` | `pub fn get_lastsendtime(&self) -> u32` | function |
| `enet_peer_get_lastreceivetime` | `NetworkPeer::get_lastreceivetime(...)` | `pub fn get_lastreceivetime(&self) -> u32` | function |
| `enet_peer_get_packets_throttle` | `NetworkPeer::get_packets_throttle(...)` | `pub fn get_packets_throttle(&self) -> f32` | function |
| `enet_peer_get_data` | `NetworkPeer::get_data(...)` | `pub fn get_data(&self) -> &mut [u8]` | function |
| `enet_peer_set_data` | `NetworkPeer::set_data(...)` | `pub fn set_data(&mut self, value: &[u8]) -> ()` | function |
| `enet_host_create_with_transport` (overload 1) | `NetworkAddress::host_create_with_transport(...)` | `pub fn host_create_with_transport(&mut self, peer_count: usize, channel_limit: usize, incoming_bandwidth: u32, outgoing_bandwidth: u32, transport: NetworkTransportType) -> Result<NetworkHost>` | function |
| `enet_host_connect` (overload 2) | `NetworkHost::connect(...)` | `pub fn connect(&self, address: &NetworkAddress, channel_count: usize, data: u32) -> Result<NetworkPeer>` | function |
| `enet_host_connect_ex` (overload 1) | `NetworkHost::connect_ex(...)` | `pub fn connect_ex(&self, endpoint: &str, channel_count: usize, data: u32) -> Result<NetworkPeer>` | function |
| `enet_host_transport_caps` (overload 1) | `NetworkHost::transport_caps(...)` | `pub fn transport_caps(&self) -> u32` | function |
| `enet_transport_handle_event` | `NetworkTransportEvent::transport_handle(...)` | `pub fn transport_handle(&self, host: &mut NetworkHost) -> ()` | function |
| `enet_peer_reset` (overload 2) | `NetworkPeer::reset(...)` | `pub fn reset(&self) -> ()` | function |
| `enet_peer_timeout` (overload 2) | `NetworkPeer::timeout(...)` | `pub fn timeout(&self) -> ()` | function |
| `enet_peer_ping` (overload 2) | `NetworkPeer::ping(...)` | `pub fn ping(&self) -> ()` | function |
| `enet_packet_encode` | `NetPacket::encode(...)` | `pub fn encode(&self, out_buffer: &mut Uint8, buffer_size: usize) -> usize` | function |
| `enet_packet_decode` | `Uint8::packet_decode(...)` | `pub fn packet_decode(&self, out_packet: &mut NetPacket, buffer_size: usize) -> bool` | function |
| `enet_fragment_send` | `NetworkPeer::fragment_send(...)` | `pub fn fragment_send(&self, channel_id: Uint8, data: &[Uint8]) -> ()` | function |
| `enet_fragment_receive` | `NetworkPeer::fragment_receive(...)` | `pub fn fragment_receive(&self, fragment: &NetPacket, out_packet: &mut NetPacket) -> bool` | function |
| `enet_bandwidth_update` | `NetworkHost::bandwidth_update(...)` | `pub fn bandwidth_update(&mut self, delta_time: f32) -> ()` | function |
| `enet_peer_send_window` | `NetworkPeer::send_window(...)` | `pub fn send_window(&self) -> usize` | function |
| `enet_peer_create_channel` | `NetworkPeer::create_channel(...)` | `pub fn create_channel(&mut self, reliable: bool, ordered: bool) -> i32` | function |
| `enet_peer_reset_channels` | `NetworkPeer::reset_channels(...)` | `pub fn reset_channels(&self) -> ()` | function |
| `enet_peer_disconnect` (overload 2) | `NetworkPeer::disconnect(...)` | `pub fn disconnect(&self, reason: Uint32TT) -> ()` | function |
| `enet_peer_disconnect_now` (overload 2) | `NetworkPeer::disconnect_now(...)` | `pub fn disconnect_now(&self) -> ()` | function |
| `enet_peer_get_stats` | `NetworkPeer::get_stats(...)` | `pub fn get_stats(&self, rtt: &mut &mut f32, packet_loss: &mut &mut f32, sent: &mut Uint32TT, received: &mut Uint32TT) -> ()` | function |
| `enet_host_flush` (overload 2) | `NetworkHost::flush(...)` | `pub fn flush(&self) -> ()` | function |
| `enet_host_broadcast` (overload 2) | `NetworkHost::broadcast(...)` | `pub fn broadcast(&self, channel_id: Uint8, data: &[Uint8]) -> ()` | function |
| `enet_host_set_protocol_id` | `NetworkHost::set_protocol_id(...)` | `pub fn set_protocol_id(&mut self, protocol_id: Uint16TT) -> ()` | function |
| `enet_host_get_protocol_id` | `NetworkHost::get_protocol_id(...)` | `pub fn get_protocol_id(&self) -> Uint16TT` | function |
| `enet_host_create_with_transport` (overload 2) | `NetworkAddress::host_create_with_transport(...)` | `pub fn host_create_with_transport(&mut self, peer_count: usize, channel_limit: usize, incoming_bandwidth: u32, outgoing_bandwidth: u32, transport: NetworkTransportType) -> Result<NetworkHost>` | function |
| `enet_host_connect` (overload 3) | `NetworkHost::connect(...)` | `pub fn connect(&self, value: &NetworkAddress, ize_t: S, int32_t: U) -> Result<NetworkPeer>` | function |
| `enet_host_connect_ex` (overload 2) | `NetworkHost::connect_ex(...)` | `pub fn connect_ex(&self, endpoint: &str, ize_t: S, int32_t: U) -> Result<NetworkPeer>` | function |
| `enet_host_transport_caps` (overload 2) | `NetworkHost::transport_caps(...)` | `pub fn transport_caps(&self) -> u32` | function |
| `enet_http_server_start` | `network::http_server_start(...)` | `pub fn http_server_start(bind_address: &str, port: NetworkUint16T) -> Result<NetworkHttpServer>` | function |
| `enet_http_server_stop` | `NetworkHttpServer::stop(...)` | `pub fn stop(&mut self) -> ()` | function |
| `enet_http_register_endpoint` | `NetworkHttpServer::http_register_endpoint(...)` | `pub fn http_register_endpoint(&self, path: &str, handler: NetworkHttpEndpointHandler) -> ()` | function |
| `enet_host_create_secure` | `NetworkAddress::host_create_secure(...)` | `pub fn host_create_secure(&mut self, peer_count: usize, channel_limit: usize, incoming_bandwidth: u32, outgoing_bandwidth: u32, transport: NetworkTransportType, security: &NetworkTransportSecurity) -> Result<NetworkHost>` | function |
| `enet_http_request_async` | `network::http_request_async(...)` | `pub fn http_request_async(desc: &NetworkHttpRequestDesc, cb: NetworkHttpResponseCallback) -> Result<NetworkHttpRequest>` | function |
| `enet_http_request_cancel` | `NetworkHttpRequest::cancel(...)` | `pub fn cancel(&self) -> ()` | function |
| `enet_http_request_release` | `NetworkHttpRequest::release(...)` | `pub fn release(&self) -> ()` | function |
| `enet_host_set_connect_validator` | `NetworkHost::set_connect_validator(...)` | `pub fn set_connect_validator(&mut self, validator: NetworkConnectValidator) -> ()` | function |
| `enet_transport_send_stream` | `network::transport_send_stream(...)` | `pub fn transport_send_stream(args: ...) -> i32` | function |
| `enet_transport_receive_stream` | `network::transport_receive_stream(...)` | `pub fn transport_receive_stream(args: ...) -> i32` | function |
| `enet_transport_ws_close` | `NetworkTransportPeer::transport_ws_close(...)` | `pub fn transport_ws_close(&mut self, code: i32) -> ()` | function |
| `enet_transport_webrtc_signal` | `NetworkWebRTCSignal::transport_webrtc(...)` | `pub fn transport_webrtc(&self, value: &mut NetworkTransport, value: &mut NetworkTransportPeer) -> ()` | function |
| `enet_host_set_webrtc_signal_callback` | `NetworkHost::set_webrtc_signal_callback(...)` | `pub fn set_webrtc_signal_callback(&mut self, net_web_rtc_signal_callback: E, value: &mut [u8]) -> ()` | function |
| `enet_webrtc_handle_signal` | `NetworkWebRTCSignal::webrtc_handle(...)` | `pub fn webrtc_handle(&self, value: &mut NetworkHost, value: &mut NetworkPeer) -> i32` | function |

### `Moss_Physics.h` → `moss::physics`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `CollideShapeResult_FreeMembers` | `CollideShapeResult::free_members(...)` | `pub fn free_members(&self) -> ()` | function |
| `CollisionEstimationResult_FreeMembers` | `CollisionEstimationResult::free_members(...)` | `pub fn free_members(&self) -> ()` | function |
| `BroadPhaseLayerInterfaceMask_Create` | `physics::create_broad_phase_layer_interface_mask(...)` | `pub fn create_broad_phase_layer_interface_mask(num_broad_phase_layers: u32) -> Result<BroadPhaseLayerInterface>` | function |
| `BroadPhaseLayerInterfaceMask_ConfigureLayer` | `BroadPhaseLayerInterface::mask_configure_layer(...)` | `pub fn mask_configure_layer(&self, broad_phase_layer: BroadPhaseLayer, groups_to_include: u32, groups_to_exclude: u32) -> ()` | function |
| `BroadPhaseLayerInterfaceTable_Create` | `physics::create_broad_phase_layer_interface_table(...)` | `pub fn create_broad_phase_layer_interface_table(num_object_layers: u32, num_broad_phase_layers: u32) -> Result<BroadPhaseLayerInterface>` | function |
| `BroadPhaseLayerInterfaceTable_MapObjectToBroadPhaseLayer` | `BroadPhaseLayerInterface::table_map_object_to_broad_phase_layer(...)` | `pub fn table_map_object_to_broad_phase_layer(&self, object_layer: ObjectLayer, broad_phase_layer: BroadPhaseLayer) -> ()` | function |
| `ObjectLayerPairFilterMask_Create` | `physics::create_object_layer_pair_filter_mask(...)` | `pub fn create_object_layer_pair_filter_mask() -> Result<ObjectLayerPairFilter>` | function |
| `ObjectLayerPairFilterMask_GetObjectLayer` | `physics::object_layer_pair_filter_mask_get_object_layer(...)` | `pub fn object_layer_pair_filter_mask_get_object_layer(group: u32, mask: u32) -> ObjectLayer` | function |
| `ObjectLayerPairFilterMask_GetGroup` | `physics::object_layer_pair_filter_mask_get_group(...)` | `pub fn object_layer_pair_filter_mask_get_group(layer: ObjectLayer) -> u32` | function |
| `ObjectLayerPairFilterMask_GetMask` | `physics::object_layer_pair_filter_mask_get_mask(...)` | `pub fn object_layer_pair_filter_mask_get_mask(layer: ObjectLayer) -> u32` | function |
| `ObjectLayerPairFilterTable_Create` | `physics::create_object_layer_pair_filter_table(...)` | `pub fn create_object_layer_pair_filter_table(num_object_layers: u32) -> Result<ObjectLayerPairFilter>` | function |
| `ObjectLayerPairFilterTable_DisableCollision` | `ObjectLayerPairFilter::table_disable_collision(...)` | `pub fn table_disable_collision(&self, layer1: ObjectLayer, layer2: ObjectLayer) -> ()` | function |
| `ObjectLayerPairFilterTable_EnableCollision` | `ObjectLayerPairFilter::table_enable_collision(...)` | `pub fn table_enable_collision(&self, layer1: ObjectLayer, layer2: ObjectLayer) -> ()` | function |
| `ObjectLayerPairFilterTable_ShouldCollide` | `ObjectLayerPairFilter::table_should_collide(...)` | `pub fn table_should_collide(&self, layer1: ObjectLayer, layer2: ObjectLayer) -> bool` | function |
| `ObjectVsBroadPhaseLayerFilterMask_Create` | `BroadPhaseLayerInterface::create_object_vs_broad_phase_layer_filter_mask(...)` | `pub fn create_object_vs_broad_phase_layer_filter_mask(&mut self) -> Result<ObjectVsBroadPhaseLayerFilter>` | function |
| `ObjectVsBroadPhaseLayerFilterTable_Create` | `BroadPhaseLayerInterface::create_object_vs_broad_phase_layer_filter_table(...)` | `pub fn create_object_vs_broad_phase_layer_filter_table(&mut self, num_broad_phase_layers: u32, object_layer_pair_filter: &mut ObjectLayerPairFilter, num_object_layers: u32) -> Result<ObjectVsBroadPhaseLayerFilter>` | function |
| `DrawSettings_InitDefault` | `DrawSettings::init_default(...)` | `pub fn init_default(&self) -> ()` | function |
| `ContactManifold_GetWorldSpaceNormal` | `ContactManifold::get_world_space_normal(...)` | `pub fn get_world_space_normal(&self, result: &mut Vec3) -> ()` | function |
| `ContactManifold_GetPenetrationDepth` | `ContactManifold::get_penetration_depth(...)` | `pub fn get_penetration_depth(&self) -> f32` | function |
| `ContactManifold_GetSubShapeID1` | `ContactManifold::get_sub_shape_id1(...)` | `pub fn get_sub_shape_id1(&self) -> SubShapeId` | function |
| `ContactManifold_GetSubShapeID2` | `ContactManifold::get_sub_shape_id2(...)` | `pub fn get_sub_shape_id2(&self) -> SubShapeId` | function |
| `ContactManifold_GetPointCount` | `ContactManifold::get_point_count(...)` | `pub fn get_point_count(&self) -> u32` | function |
| `ContactManifold_GetWorldSpaceContactPointOn1` | `ContactManifold::get_world_space_contact_point_on1(...)` | `pub fn get_world_space_contact_point_on1(&self, index: u32, result: &mut Vec3) -> ()` | function |
| `ContactManifold_GetWorldSpaceContactPointOn2` | `ContactManifold::get_world_space_contact_point_on2(...)` | `pub fn get_world_space_contact_point_on2(&self, index: u32, result: &mut Vec3) -> ()` | function |
| `CollisionDispatch_CollideShapeVsShape` | `CollideShapeCollectorCallback::collision_dispatch_collide_shape_vs_shape(...)` | `pub fn collision_dispatch_collide_shape_vs_shape(&self, shape1: &Shape, shape2: &Shape, scale1: &Vec3, scale2: &Vec3, center_of_mass_transform1: &Mat44, center_of_mass_transform2: &Mat44, collide_shape_settings: &CollideShapeSettings, shape_filter: &ShapeFilter) -> bool` | function |
| `CollisionDispatch_CastShapeVsShapeLocalSpace` | `CastShapeCollectorCallback::collision_dispatch_cast_shape_vs_shape_local_space(...)` | `pub fn collision_dispatch_cast_shape_vs_shape_local_space(&self, direction: &Vec3, shape1: &Shape, shape2: &Shape, scale1_in_shape2_local_space: &Vec3, scale2: &Vec3, center_of_mass_transform1_in_shape2_local_space: &mut Mat44, center_of_mass_world_transform2: &mut Mat44, shape_cast_settings: &ShapeCastSettings, shape_filter: &ShapeFilter) -> bool` | function |
| `CollisionDispatch_CastShapeVsShapeWorldSpace` | `CastShapeCollectorCallback::collision_dispatch_cast_shape_vs_shape_world_space(...)` | `pub fn collision_dispatch_cast_shape_vs_shape_world_space(&self, direction: &Vec3, shape1: &Shape, shape2: &Shape, scale1: &Vec3, in_scale2: &Vec3, center_of_mass_world_transform1: &Mat44, center_of_mass_world_transform2: &Mat44, shape_cast_settings: &ShapeCastSettings, shape_filter: &ShapeFilter) -> bool` | function |
| `Skeleton_Create` | `physics::create_skeleton(...)` | `pub fn create_skeleton() -> Result<Skeleton>` | function |
| `Skeleton_Destroy` | `Skeleton::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Skeleton_AddJoint` | `Skeleton::add_joint(...)` | `pub fn add_joint(&self, name: &str) -> u32` | function |
| `Skeleton_AddJoint2` | `Skeleton::add_joint2(...)` | `pub fn add_joint2(&self, name: &str, parent_index: i32) -> u32` | function |
| `Skeleton_AddJoint3` | `Skeleton::add_joint3(...)` | `pub fn add_joint3(&self, name: &str, parent_name: &str) -> u32` | function |
| `Skeleton_GetJointCount` | `Skeleton::get_joint_count(...)` | `pub fn get_joint_count(&self) -> i32` | function |
| `Skeleton_GetJoint` | `SkeletonJoint::skeleton_get(...)` | `pub fn skeleton_get(&self, skeleton: &Skeleton, index: i32) -> ()` | function |
| `Skeleton_GetJointIndex` | `Skeleton::get_joint_index(...)` | `pub fn get_joint_index(&self, name: &str) -> i32` | function |
| `Skeleton_CalculateParentJointIndices` | `Skeleton::calculate_parent_joint_indices(...)` | `pub fn calculate_parent_joint_indices(&self) -> ()` | function |
| `Skeleton_AreJointsCorrectlyOrdered` | `Skeleton::are_joints_correctly_ordered(...)` | `pub fn are_joints_correctly_ordered(&self) -> bool` | function |
| `Indexify` | `physics::indexify(...)` | `pub fn indexify(in_triangles: TriangleList, out_vertices: VertexList, out_triangles: IndexedTriangleList, in_vertex_weld_distance: f32) -> ()` | function |
| `Deindexify` | `physics::deindexify(...)` | `pub fn deindexify(in_vertices: VertexList, in_triangles: IndexedTriangleList, out_triangles: TriangleList) -> ()` | function |

### `Moss_Platform.h` → `moss::platform`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `Moss_CreateWindow` | `platform::create_window(...)` | `pub fn create_window(options: WindowOptions) -> Result<Window>` | function |
| `Moss_TerminateWindow` | `Window::close(...)` | `pub fn close(&self) -> ()` | function |
| `Moss_CreateMessageBox` | `Window::show_message_box(...)` | `pub fn show_message_box(&mut self, title: &str, message: &str, flags: MessageBoxFlags) -> bool` | function |
| `Moss_ShouldWindowClose` | `Window::should_close` | `pub fn should_close(&self) -> bool` | function |
| `Moss_PollEvents` | `platform::poll_events(...)` | `pub fn poll_events() -> ()` | function |
| `Moss_GetWindowWidth` | `platform::get_window_width(...)` | `pub fn get_window_width() -> i32` | function |
| `Moss_GetWindowHeight` | `platform::get_window_height(...)` | `pub fn get_window_height() -> i32` | function |
| `Moss_SetWindowTitle` | `Window::title / Window::set_title` | `pub fn title(&self) -> &str; pub fn set_title(&mut self, title: &str)` | function |
| `Moss_SetWindowIcon` | `Window::set_icon(...)` | `pub fn set_icon(&mut self, image: Image) -> ()` | function |
| `Moss_CloseWindow` | `Window::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_MonitorGetPrimary` | `platform::monitors::monitor_get_primary(...)` | `pub fn monitor_get_primary() -> Result<Monitor>` | function |
| `Moss_MonitorGetSecondary` | `platform::monitors::monitor_get_secondary(...)` | `pub fn monitor_get_secondary() -> Result<Monitor>` | function |
| `Moss_MonitorGetPhysicalSize` | `Monitor::get_physical_size(...)` | `pub fn get_physical_size(&self, width_mm: &mut &mut i32, height_mm: &mut &mut i32) -> ()` | function |
| `Moss_MonitorGetContentScale` | `Monitor::get_content_scale(...)` | `pub fn get_content_scale(&self, xscale: &mut &mut f32, yscale: &mut &mut f32) -> ()` | function |
| `Moss_MonitorGetPosition` | `Monitor::get_position(...)` | `pub fn get_position(&self, x: &mut &mut i32, y: &mut &mut i32) -> ()` | function |
| `Moss_MonitorGetName` | `Monitor::get_name(...)` | `pub fn get_name(&self) -> Option<String>` | function |
| `Moss_MonitorSetGammaRamp` | `GammaRamp::monitor_set(...)` | `pub fn monitor_set(&mut self, monitor: &mut Monitor) -> ()` | function |
| `Moss_MonitorGetGammaRamp` | `Monitor::get_gamma_ramp(...)` | `pub fn get_gamma_ramp(&self) -> Result<GammaRamp>` | function |
| `Moss_MonitorSetGamma` | `Monitor::set_gamma(...)` | `pub fn set_gamma(&mut self, gamma: f32) -> ()` | function |
| `Moss_IsKeyPressed` | `platform::input::is_key_pressed(...)` | `pub fn is_key_pressed(key: Keyboard) -> bool` | function |
| `Moss_IsReleased` | `platform::is_released(...)` | `pub fn is_released(key: Keyboard) -> bool` | function |
| `Moss_IsKeyJustPressed` | `platform::input::is_key_just_pressed(...)` | `pub fn is_key_just_pressed(key: Keyboard) -> bool` | function |
| `Moss_IsKeyJustReleased` | `platform::input::is_key_just_released(...)` | `pub fn is_key_just_released(key: Keyboard) -> bool` | function |
| `Moss_InputGetKey` | `platform::input::input_get_key(...)` | `pub fn input_get_key() -> Keyboard` | function |
| `Moss_IsMousePressed` | `platform::input::is_mouse_pressed(...)` | `pub fn is_mouse_pressed(button: Mouse) -> bool` | function |
| `Moss_IsMouseReleased` | `platform::input::is_mouse_released(...)` | `pub fn is_mouse_released(button: Mouse) -> bool` | function |
| `Moss_IsMouseJustPressed` | `platform::input::is_mouse_just_pressed(...)` | `pub fn is_mouse_just_pressed(button: Mouse) -> bool` | function |
| `Moss_IsMouseJustReleased` | `platform::input::is_mouse_just_released(...)` | `pub fn is_mouse_just_released(button: Mouse) -> bool` | function |
| `Moss_InputGetMouseButton` | `platform::input::input_get_mouse_button(...)` | `pub fn input_get_mouse_button() -> Mouse` | function |
| `Moss_GetMousePosition` | `platform::input::mouse_position(...)` | `pub fn mouse_position() -> (i32, i32)` | function |
| `Moss_SetMousePosition` | `platform::input::set_mouse_position(...)` | `pub fn set_mouse_position(x: i32, y: i32) -> ()` | function |
| `Moss_SetMouseVisible` | `platform::input::set_mouse_visible(...)` | `pub fn set_mouse_visible(visible: bool) -> ()` | function |
| `Moss_GetNumGamepads` | `platform::get_num_gamepads(...)` | `pub fn get_num_gamepads() -> i32` | function |
| `Moss_OpenGamepad` | `platform::gamepads::open_gamepad(...)` | `pub fn open_gamepad(id: GamepadId) -> Result<Gamepad>` | function |
| `Moss_CloseGamepad` | `Gamepad::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_GamepadConnected` | `Gamepad::connected(...)` | `pub fn connected(&self) -> bool` | function |
| `Moss_UpdateGamepads` | `platform::update_gamepads(...)` | `pub fn update_gamepads() -> ()` | function |
| `Moss_IsGamepadButtonPressed` | `Gamepad::is_button_pressed(...)` | `pub fn is_button_pressed(&self, button: GamepadButton) -> bool` | function |
| `Moss_IsGamepadButtonJustPressed` | `Gamepad::is_button_just_pressed(...)` | `pub fn is_button_just_pressed(&self, button: GamepadButton) -> bool` | function |
| `Moss_IsGamepadButtonJustReleased` | `Gamepad::is_button_just_released(...)` | `pub fn is_button_just_released(&self, button: GamepadButton) -> bool` | function |
| `Moss_GetGamepadAxis` | `Gamepad::get_axis(...)` | `pub fn get_axis(&self, axis: GamepadAxis) -> f32` | function |
| `Moss_GetGamepadOrientation` | `Gamepad::get_orientation(...)` | `pub fn get_orientation(&self, w: &mut &mut f32, x: &mut &mut f32, y: &mut &mut f32, z: &mut &mut f32) -> bool` | function |
| `Moss_GetGamepadPlayerSpaceGyro` | `Gamepad::get_player_space_gyro(...)` | `pub fn get_player_space_gyro(&self, out_x: &mut &mut f32, out_y: &mut &mut f32, yaw_relax_factor: f32) -> ()` | function |
| `Moss_GetGamepadWorldSpaceGyro` | `Gamepad::get_world_space_gyro(...)` | `pub fn get_world_space_gyro(&self, out_x: &mut &mut f32, out_y: &mut &mut f32, side_reduction_threshold: f32) -> ()` | function |
| `Moss_ResetGamepadCalibration` | `Gamepad::reset_calibration(...)` | `pub fn reset_calibration(&self) -> ()` | function |
| `Moss_RumbleGamepad` | `Gamepad::rumble(...)` | `pub fn rumble(&self, low: u16, high: u16, duration_ms: u32) -> bool` | function |
| `Moss_RumbleGamepadTriggers` | `Gamepad::rumble_triggers(...)` | `pub fn rumble_triggers(&self, left: u16, right: u16, duration_ms: u32) -> bool` | function |
| `Moss_SetGamepadLED` | `Gamepad::set_led(...)` | `pub fn set_led(&mut self, r: u8, g: u8, b: u8) -> bool` | function |
| `Moss_GetGamepadName` | `Gamepad::get_name(...)` | `pub fn get_name(&self) -> Option<String>` | function |
| `Moss_GetGamepadID` | `Gamepad::get_id(...)` | `pub fn get_id(&self) -> GamepadId` | function |
| `Moss_GetGamepadPlayerIndex` | `Gamepad::get_player_index(...)` | `pub fn get_player_index(&self) -> i32` | function |
| `Moss_GetGamepadPowerInfo` | `Gamepad::get_power_info(...)` | `pub fn get_power_info(&self, percent: &mut &mut i32) -> PowerState` | function |
| `Moss_GetNumGamepadTouchpads` | `Gamepad::get_num_touchpads(...)` | `pub fn get_num_touchpads(&self) -> i32` | function |
| `Moss_GetNumGamepadTouchpadFingers` | `Gamepad::get_num_touchpad_fingers(...)` | `pub fn get_num_touchpad_fingers(&self) -> i32` | function |
| `Moss_GetGamepadTouchpadFinger` | `Gamepad::get_touchpad_finger(...)` | `pub fn get_touchpad_finger(&self, pad: i32, finger: i32, down: &mut bool, x: &mut &mut f32, y: &mut &mut f32, pressure: &mut &mut f32) -> bool` | function |
| `Moss_GetGamepadMapping` | `Gamepad::get_mapping(...)` | `pub fn get_mapping(&self) -> Option<String>` | function |
| `Moss_SetGamepadMapping` | `Gamepad::set_mapping(...)` | `pub fn set_mapping(&mut self, mapping: &str) -> bool` | function |
| `Moss_ReloadGamepadMappings` | `platform::gamepads::reload_gamepad_mappings(...)` | `pub fn reload_gamepad_mappings() -> ()` | function |
| `Moss_InputGetGamepadButton` | `platform::gamepads::input_get_gamepad_button(...)` | `pub fn input_get_gamepad_button() -> GamepadButton` | function |
| `Moss_InputGetGamepadAxis` | `platform::gamepads::input_get_gamepad_axis(...)` | `pub fn input_get_gamepad_axis() -> GamepadAxis` | function |
| `Moss_GetPenDeviceType` | `platform::input::get_pen_device_type(...)` | `pub fn get_pen_device_type(instance_id: PenId) -> PenDeviceType` | function |
| `Moss_GetTouchDeviceName` | `platform::input::get_touch_device_name(...)` | `pub fn get_touch_device_name(touch_id: TouchId) -> Option<String>` | function |
| `Moss_GetTouchDevices` | `platform::input::get_touch_devices(...)` | `pub fn get_touch_devices(count: &mut &mut i32) -> Result<TouchId>` | function |
| `Moss_GetTouchDeviceType` | `platform::input::get_touch_device_type(...)` | `pub fn get_touch_device_type(touch_id: TouchId) -> TouchDeviceType` | function |
| `Moss_GetTouchFingers` | `platform::input::get_touch_fingers(...)` | `pub fn get_touch_fingers(touch_id: TouchId, count: &mut &mut i32) -> Result<Finger>` | function |
| `Moss_GamepadHasSensor` | `Gamepad::has_sensor(...)` | `pub fn has_sensor(&self, sensor: GamepadSensorType) -> bool` | function |
| `Moss_SetGamepadSensorEnabled` | `Gamepad::set_sensor_enabled(...)` | `pub fn set_sensor_enabled(&mut self, sensor: GamepadSensorType, enabled: bool) -> bool` | function |
| `Moss_GetGamepadSensorData` | `GamepadSensorData::get(...)` | `pub fn get(&self, gp: &mut Gamepad, sensor: GamepadSensorType) -> bool` | function |
| `Moss_OpenHaptic` | `platform::haptics::open_haptic(...)` | `pub fn open_haptic(id: HapticId) -> Result<Haptic>` | function |
| `Moss_CloseHaptic` | `Haptic::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_CreateHapticEffect` | `Haptic::create_effect(...)` | `pub fn create_effect(&mut self) -> HapticId` | function |
| `Moss_DestroyHapticEffect` | `Haptic::destroy_effect(...)` | `pub fn destroy_effect(&mut self) -> ()` | function |
| `Moss_GetHapticEffectStatus` | `Haptic::get_effect_status(...)` | `pub fn get_effect_status(&self) -> bool` | function |
| `Moss_GetHapticFeatures` | `Haptic::get_features(...)` | `pub fn get_features(&self) -> u32` | function |
| `Moss_GetHapticFromID` | `Haptic::get_from_id(...)` | `pub fn get_from_id(&self) -> Result<Haptic>` | function |
| `Moss_GetHapticID` | `Haptic::get_id(...)` | `pub fn get_id(&self) -> Result<HapticId>` | function |
| `Moss_GetHapticName` | `Haptic::get_name(...)` | `pub fn get_name(&self) -> Option<String>` | function |
| `Moss_GetHapticNameForID` | `Haptic::get_name_for_id(...)` | `pub fn get_name_for_id(&self) -> Option<String>` | function |
| `Moss_GetHaptics` | `Haptic::get_haptics(...)` | `pub fn get_haptics(&self) -> Result<HapticId>` | function |
| `Moss_GetMaxHapticEffects` | `Haptic::get_max_effects(...)` | `pub fn get_max_effects(&self) -> i32` | function |
| `Moss_GetMaxHapticEffectsPlaying` | `Haptic::get_max_effects_playing(...)` | `pub fn get_max_effects_playing(&self) -> i32` | function |
| `Moss_GetNumHapticAxes` | `Haptic::get_num_axes(...)` | `pub fn get_num_axes(&self) -> i32` | function |
| `Moss_HapticEffectSupported` | `Haptic::effect_supported(...)` | `pub fn effect_supported(&self) -> bool` | function |
| `Moss_HapticRumbleSupported` | `Haptic::rumble_supported(...)` | `pub fn rumble_supported(&self) -> bool` | function |
| `Moss_InitHapticRumble` | `Haptic::init_rumble(...)` | `pub fn init_rumble(&self) -> bool` | function |
| `Moss_IsJoystickHaptic` | `Gamepad::is_joystick_haptic(...)` | `pub fn is_joystick_haptic(&self) -> bool` | function |
| `Moss_IsMouseHaptic` | `platform::haptics::is_mouse_haptic(...)` | `pub fn is_mouse_haptic() -> bool` | function |
| `Moss_OpenHapticFromJoystick` | `Gamepad::open_haptic_from_joystick(...)` | `pub fn open_haptic_from_joystick(&self) -> Result<Haptic>` | function |
| `Moss_OpenHapticFromMouse` | `platform::haptics::open_haptic_from_mouse(...)` | `pub fn open_haptic_from_mouse() -> Result<Haptic>` | function |
| `Moss_PauseHaptic` | `Haptic::pause(...)` | `pub fn pause(&self) -> bool` | function |
| `Moss_PlayHapticRumble` | `Haptic::play_rumble(...)` | `pub fn play_rumble(&mut self, strength: f32, length: u32) -> bool` | function |
| `Moss_ResumeHaptic` | `Haptic::resume(...)` | `pub fn resume(&self) -> bool` | function |
| `Moss_RunHapticEffect` | `Haptic::run_effect(...)` | `pub fn run_effect(&self, iterations: u32) -> bool` | function |
| `Moss_SetHapticAutocenter` | `Haptic::set_autocenter(...)` | `pub fn set_autocenter(&mut self, center: i32) -> bool` | function |
| `Moss_SetHapticGain` | `Haptic::set_gain(...)` | `pub fn set_gain(&mut self, gain: i32) -> bool` | function |
| `Moss_StopHapticEffect` | `Haptic::stop_effect(...)` | `pub fn stop_effect(&mut self, effect: HapticEffectId) -> bool` | function |
| `Moss_StopHapticEffects` | `Haptic::stop_effects(...)` | `pub fn stop_effects(&mut self) -> bool` | function |
| `Moss_StopHapticRumble` | `Haptic::stop_rumble(...)` | `pub fn stop_rumble(&mut self) -> bool` | function |
| `Moss_UpdateHapticEffect` | `HapticEffect::update(...)` | `pub fn update(&mut self, haptic: &mut Haptic, effect: HapticEffectId) -> bool` | function |
| `Moss_GetAvailableCPUCores` | `platform::get_available_cpu_cores(...)` | `pub fn get_available_cpu_cores() -> i32` | function |
| `Moss_GetCPUCacheLineSize` | `platform::get_cpu_cache_line_size(...)` | `pub fn get_cpu_cache_line_size() -> i32` | function |
| `Moss_GetSystemRAM` | `platform::get_system_ram(...)` | `pub fn get_system_ram() -> i32` | function |
| `Moss_OpenURL` | `platform::open_url(...)` | `pub fn open_url(url: &str) -> bool` | function |
| `Moss_GetLocale` | `platform::get_locale(...)` | `pub fn get_locale() -> Result<Locale>` | function |
| `Moss_GetPowerInfo` | `platform::get_power_info(...)` | `pub fn get_power_info(seconds: &mut &mut i32, percent: &mut &mut i32) -> PowerState` | function |
| `Moss_IsProcessRunningByName` | `platform::is_process_running_by_name(...)` | `pub fn is_process_running_by_name(executable_path: &str) -> bool` | function |
| `Moss_LoadDynamicLibrary` | `platform::load_dynamic_library(...)` | `pub fn load_dynamic_library(lib_path: &str) -> &mut [u8]` | function |
| `Moss_GetLibrarySymbol` | `platform::get_library_symbol(...)` | `pub fn get_library_symbol(handle: &mut [u8], symbol_name: &str) -> &mut [u8]` | function |
| `Moss_UnloadDynamicLibrary` | `platform::unload_dynamic_library(...)` | `pub fn unload_dynamic_library(handle: &mut [u8]) -> ()` | function |
| `Moss_SetFramebufferResizeCallback` | `platform::set_framebuffer_resize_callback(...)` | `pub fn set_framebuffer_resize_callback(callback: FramebufferResizeCallback) -> ()` | function |
| `Moss_SetWindowSizeCallback` | `platform::set_window_size_callback(...)` | `pub fn set_window_size_callback(callback: WindowSizeCallback) -> ()` | function |
| `Moss_SetWindowResizeCallback` | `platform::set_window_resize_callback(...)` | `pub fn set_window_resize_callback(callback: WindowResizeCallback) -> ()` | function |
| `Moss_SetWindowPositionCallback` | `platform::set_window_position_callback(...)` | `pub fn set_window_position_callback(callback: WindowPositionCallback) -> ()` | function |
| `Moss_SetWindowFocusCallback` | `platform::set_window_focus_callback(...)` | `pub fn set_window_focus_callback(callback: WindowFocusCallback) -> ()` | function |
| `Moss_SetWindowContentScaleCallback` | `platform::set_window_content_scale_callback(...)` | `pub fn set_window_content_scale_callback(callback: WindowContentScaleCallback) -> ()` | function |
| `Moss_SetMonitorCallback` | `platform::monitors::set_monitor_callback(...)` | `pub fn set_monitor_callback(callback: MonitorCallback) -> ()` | function |
| `Moss_GetCameras` | `platform::get_cameras(...)` | `pub fn get_cameras(count: &mut &mut i32) -> Result<CameraId>` | function |
| `Moss_GetCameraName` | `platform::cameras::get_camera_name(...)` | `pub fn get_camera_name(camera_id: CameraId) -> Option<String>` | function |
| `Moss_GetCameraPosition` | `platform::cameras::get_camera_position(...)` | `pub fn get_camera_position(camera_id: CameraId) -> CameraPosition` | function |
| `Moss_GetCurrentCameraDriver` | `platform::cameras::get_current_camera_driver(...)` | `pub fn get_current_camera_driver() -> Option<String>` | function |
| `Moss_GetNumCameraDrivers` | `platform::cameras::get_num_camera_drivers(...)` | `pub fn get_num_camera_drivers() -> i32` | function |
| `Moss_GetCameraSupportedFormats` | `platform::cameras::get_camera_supported_formats(...)` | `pub fn get_camera_supported_formats(camera_id: CameraId, count: &mut &mut i32) -> Result<CameraSpec>` | function |
| `Moss_AcquireCameraFrame` | `Capture::acquire_camera_frame(...)` | `pub fn acquire_camera_frame(&self, timestamp_ns: &mut u64) -> Result<Surface>` | function |
| `Moss_ReleaseCameraFrame` | `Capture::release_camera_frame(...)` | `pub fn release_camera_frame(&self, frame: &mut Surface) -> ()` | function |
| `Moss_GetCameraFormat` | `Capture::get_camera_format(...)` | `pub fn get_camera_format(&self, out_spec: &mut CameraSpec) -> bool` | function |
| `Moss_GetCameraPermissionState` | `Capture::get_camera_permission_state(...)` | `pub fn get_camera_permission_state(&self) -> CameraPermissionState` | function |
| `Moss_GetCameraProperties` (overload 1) | `Capture::get_camera_properties(...)` | `pub fn get_camera_properties(&self) -> PropertiesId` | function |
| `Moss_CloseCamera` | `Capture::close_camera(...)` | `pub fn close_camera(&mut self) -> ()` | function |
| `Moss_GetCameraID` | `Capture::get_camera_id(...)` | `pub fn get_camera_id(&self) -> CameraId` | function |
| `Moss_GetCameraProperties` (overload 2) | `Capture::get_camera_properties(...)` | `pub fn get_camera_properties(&self) -> PropertiesId` | function |
| `Moss_OpenCapture` | `CameraSpec::open_capture(...)` | `pub fn open_capture(&self, id: CameraId) -> Result<Capture>` | function |
| `Moss_CopyFile` | `platform::storage::copy_file(...)` | `pub fn copy_file(src_path: &str, dst_path: &str, overwrite: bool) -> bool` | function |
| `Moss_CreateDirectory` | `platform::storage::create_directory(...)` | `pub fn create_directory(path: &str, recursive: bool) -> bool` | function |
| `Moss_RemovePath` | `platform::storage::remove_path(...)` | `pub fn remove_path(path: &str, recursive: bool) -> bool` | function |
| `Moss_RenamePath` | `platform::storage::rename_path(...)` | `pub fn rename_path(old_path: &str, new_path: &str, overwrite: bool) -> bool` | function |
| `Moss_GetPathInfo` | `platform::storage::get_path_info(...)` | `pub fn get_path_info(path: &str, out_info: &mut PathInfo) -> bool` | function |
| `Moss_GetCurrentDirectory` | `platform::storage::get_current_directory(...)` | `pub fn get_current_directory(out_path: &mut [u8], max_len: i32) -> bool` | function |
| `Moss_GetBasePath` | `platform::storage::get_base_path(...)` | `pub fn get_base_path(out_path: &mut [u8], max_len: i32) -> bool` | function |
| `Moss_GetUserFolder` | `platform::get_user_folder(...)` | `pub fn get_user_folder(folder: UserFolder, out_path: &mut [u8], max_len: i32) -> bool` | function |
| `Moss_GetPrefPath` | `platform::storage::get_pref_path(...)` | `pub fn get_pref_path(org_name: &str, app_name: &str, out_path: &mut [u8], max_len: i32) -> bool` | function |
| `Moss_EnumerateDirectory` | `platform::storage::enumerate_directory(...)` | `pub fn enumerate_directory(path: &str, recursive: bool, callback: DirectoryIterateFn) -> bool` | function |
| `Moss_GlobDirectory` | `platform::storage::glob_directory(...)` | `pub fn glob_directory(pattern: &str, callback: DirectoryIterateFn) -> bool` | function |
| `Moss_ShowFileDialogWithProperties` | `DialogFileFilter::show_file_dialog_with_properties(...)` | `pub fn show_file_dialog_with_properties(&self, callback: DialogFileCallback, window: &mut Window, nfilters: i32, default_location: &str) -> ()` | function |
| `Moss_ShowOpenFolderDialog` | `DialogFileFilter::show_open_folder_dialog(...)` | `pub fn show_open_folder_dialog(&self, callback: DialogFileCallback, window: &mut Window, nfilters: i32, default_location: &str, allow_many: bool) -> ()` | function |
| `Moss_ShowOpenFileDialog` | `Window::show_open_file_dialog(...)` | `pub fn show_open_file_dialog(&self, callback: DialogFileCallback, default_location: &str, allow_many: bool) -> ()` | function |
| `Moss_ShowSaveFileDialog` | `platform::dialogs::show_save_file_dialog(...)` | `pub fn show_save_file_dialog(type: FileDialogType, callback: DialogFileCallback, props: PropertiesId) -> ()` | function |
| `Moss_CloseStorage` | `Storage::close(...)` | `pub fn close(&mut self) -> bool` | function |
| `Moss_CopyStorageFile` | `Storage::copy_file(...)` | `pub fn copy_file(&self, oldpath: &str, newpath: &str) -> bool` | function |
| `Moss_CreateStorageDirectory` | `Storage::create_directory(...)` | `pub fn create_directory(&mut self, path: &str) -> bool` | function |
| `Moss_EnumerateStorageDirectory` | `Storage::enumerate_directory(...)` | `pub fn enumerate_directory(&self, path: &str, callback: EnumerateDirectoryCallback) -> bool` | function |
| `Moss_GetStorageFileSize` | `Storage::get_file_size(...)` | `pub fn get_file_size(&self, path: &str, length: &mut Uint64) -> bool` | function |
| `Moss_GetStorageSpaceRemaining` | `Storage::get_space_remaining(...)` | `pub fn get_space_remaining(&self) -> u64` | function |
| `Moss_GetStoragePathInfo` | `Storage::get_path_info(...)` | `pub fn get_path_info(&self, path: &str, info: &mut PathInfo) -> bool` | function |
| `Moss_GlobStorageDirectory` | `Storage::glob_directory(...)` | `pub fn glob_directory(&self, path: &str, pattern: &str, flags: GlobFlags, count: &mut &mut i32) -> Option<String>` | function |
| `Moss_OpenFileStorage` | `platform::storage::open_file_storage(...)` | `pub fn open_file_storage(path: &str) -> Result<Storage>` | function |
| `Moss_OpenStorage` | `Storage::open(...)` | `pub fn open(&self) -> Result<Storage>` | function |
| `Moss_OpenTitleStorage` | `platform::storage::open_title_storage(...)` | `pub fn open_title_storage(override: &str, props: PropertiesId) -> Result<Storage>` | function |
| `Moss_OpenUserStorage` | `platform::storage::open_user_storage(...)` | `pub fn open_user_storage(org: &str, app: &str, props: PropertiesId) -> Result<Storage>` | function |
| `Moss_ReadStorageFile` | `Storage::read_file(...)` | `pub fn read_file(&self, path: &str, destination: &mut [u8], length: Uint64) -> bool` | function |
| `Moss_RemoveStoragePath` | `Storage::remove_path(...)` | `pub fn remove_path(&mut self, path: &str) -> bool` | function |
| `Moss_RenameStoragePath` | `Storage::rename_path(...)` | `pub fn rename_path(&self, oldpath: &str, newpath: &str) -> bool` | function |
| `Moss_StorageReady` | `Storage::ready(...)` | `pub fn ready(&self) -> bool` | function |
| `Moss_WriteStorageFile` | `Storage::write_file(...)` | `pub fn write_file(&self, path: &str, source: &[u8], length: Uint64) -> bool` | function |
| `Moss_CreateEmbeddedWindow` | `platform::create_embedded_window(...)` | `pub fn create_embedded_window() -> Result<Window>` | function |
| `Moss_Terminate_EmbeddedWindow` | `platform::terminate_embedded_window(...)` | `pub fn terminate_embedded_window() -> ()` | function |
| `Moss_EmbeddedWindow_OnOrientationChanged` | `platform::embedded_window_on_orientation_changed(...)` | `pub fn embedded_window_on_orientation_changed(focused: bool) -> ()` | function |
| `Moss_EmbeddedWindow_OnResume` | `platform::embedded_window_on_resume(...)` | `pub fn embedded_window_on_resume() -> ()` | function |
| `Moss_EmbeddedWindow_OnPause` | `platform::embedded_window_on_pause(...)` | `pub fn embedded_window_on_pause() -> ()` | function |
| `Moss_MakeContextCurrent` | `Window::make_context_current(...)` | `pub fn make_context_current(&self) -> ()` | function |
| `Moss_SwapBuffers` | `platform::swap_buffers(...)` | `pub fn swap_buffers() -> ()` | function |
| `Moss_SwapBuffersInterval` | `platform::swap_buffers_interval(...)` | `pub fn swap_buffers_interval(interval: i32) -> ()` | function |
| `Moss_GetProcAddress` | `platform::get_proc_address(...)` | `pub fn get_proc_address(procname: &str) -> &mut [u8]` | function |
| `Moss_CreateWindowSurface` | `Window::create_vulkan_surface(...)` | `pub fn create_vulkan_surface(&self, instance: VkInstance, allocator: Option<&VkAllocationCallbacks>) -> Result<VkSurfaceKHR>` | function |
| `Moss_VulkanSupported` | `platform::supported(...)` | `pub fn supported() -> i32` | function |
| `Moss_GetRequiredInstanceExtensions` | `platform::get_required_instance_extensions(...)` | `pub fn get_required_instance_extensions(count: &mut &mut u32) -> Option<String>` | function |
| `Moss_GetInstanceProcAddress` | `platform::get_instance_proc_address(...)` | `pub fn get_instance_proc_address(instance: VkInstance, procname: &str) -> &mut [u8]` | function |
| `Moss_GetPhysicalDevicePresentationSupport` | `Window::get_physical_device_presentation_support(...)` | `pub fn get_physical_device_presentation_support(&self, device: VkPhysicalDevice, queuefamily: u32) -> i32` | function |
| `Moss_Metal_CreateView` | `Window::create_view(...)` | `pub fn create_view(&mut self) -> MetalView` | function |
| `Moss_Metal_DestroyView` | `platform::destroy_view(...)` | `pub fn destroy_view(view: MetalView) -> ()` | function |
| `Moss_Metal_GetLayer` | `platform::get_layer(...)` | `pub fn get_layer(view: MetalView) -> &mut [u8]` | function |
| `Moss_Metal_Resize` | `platform::resize(...)` | `pub fn resize(handle: MetalView, width: u32, height: u32) -> ()` | function |

### `Moss_Renderer.h` → `moss::renderer`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `Moss_RendererCreateMaterial` | `Renderer::create_material(...)` | `pub fn create_material(&mut self, desc: &MaterialDesc) -> Result<Material>` | function |
| `Moss_RendererDestroyMaterial` | `Renderer::destroy_material(...)` | `pub fn destroy_material(&mut self, material: &mut Material) -> ()` | function |
| `Moss_MaterialSetUniform` | `Material::set_uniform(...)` | `pub fn set_uniform(&mut self, name: &str, data: &[u8], size: u32) -> i32` | function |
| `Moss_MaterialSetTexture` | `Material::set_texture(...)` | `pub fn set_texture(&mut self, type: MaterialTextureType, texture: &mut Texture, sampler: &mut GpuSampler) -> i32` | function |
| `Moss_RendererCreatePipeline` | `Renderer::create_pipeline(...)` | `pub fn create_pipeline(&mut self, desc: &PipelineDesc) -> Result<PipelineState>` | function |
| `Moss_PipelineBind` | `renderer::pipeline_bind(...)` | `pub fn pipeline_bind(pipeline: &mut PipelineState) -> ()` | function |
| `Moss_PipelineUnbind` | `renderer::pipeline_unbind(...)` | `pub fn pipeline_unbind(pipeline: &mut PipelineState) -> ()` | function |
| `Moss_PipelineSetUniform` | `renderer::pipeline_set_uniform(...)` | `pub fn pipeline_set_uniform(pipeline: &mut PipelineState, name: &str, data: &[u8], size: u32) -> ()` | function |
| `Moss_PipelineFlush` | `renderer::pipeline_flush(...)` | `pub fn pipeline_flush(pipeline: &mut PipelineState) -> ()` | function |
| `Moss_PipelineValidate` | `renderer::pipeline_validate(...)` | `pub fn pipeline_validate(pipeline: &mut PipelineState) -> i32` | function |
| `Moss_RendererCreateResourceSet` | `Renderer::create_resource_set(...)` | `pub fn create_resource_set(&mut self, desc: &ResourceSetLayoutDesc) -> Result<ResourceSet>` | function |
| `Moss_RendererDestroyResourceSet` | `Renderer::destroy_resource_set(...)` | `pub fn destroy_resource_set(&mut self, set: &mut ResourceSet) -> ()` | function |
| `Moss_ResourceSetBindUniformBuffer` | `ResourceSet::bind_uniform_buffer(...)` | `pub fn bind_uniform_buffer(&mut self, binding: u32, buffer: &mut GpuBuffer) -> ()` | function |
| `Moss_ResourceSetBindUniformBufferRange` | `ResourceSet::bind_uniform_buffer_range(...)` | `pub fn bind_uniform_buffer_range(&mut self, binding: u32, buffer: &mut GpuBuffer, offset: usize, size: usize) -> ()` | function |
| `Moss_ResourceSetBindStorageBuffer` | `ResourceSet::bind_storage_buffer(...)` | `pub fn bind_storage_buffer(&mut self, binding: u32, buffer: &mut GpuBuffer) -> ()` | function |
| `Moss_ResourceSetBindTexture` | `ResourceSet::bind_texture(...)` | `pub fn bind_texture(&mut self, binding: u32, texture: &mut Texture) -> ()` | function |
| `Moss_ResourceSetBindSampler` | `ResourceSet::bind_sampler(...)` | `pub fn bind_sampler(&mut self, binding: u32, sampler: &mut GpuSampler) -> ()` | function |
| `Moss_ResourceSetBind` | `Renderer::resource_set_bind(...)` | `pub fn resource_set_bind(&mut self, pipeline: &mut PipelineState, set_index: u32, set: &mut ResourceSet) -> ()` | function |
| `Moss_RendererCreateFramebuffer` | `Renderer::create_framebuffer(...)` | `pub fn create_framebuffer(&mut self, desc: &FramebufferDesc) -> Result<Framebuffer>` | function |
| `Moss_RendererDestroyFramebuffer` | `Renderer::destroy_framebuffer(...)` | `pub fn destroy_framebuffer(&mut self, framebuffer: &mut Framebuffer) -> ()` | function |
| `Moss_FramebufferResize` | `Framebuffer::resize(...)` | `pub fn resize(&self, width: u32, height: u32) -> ()` | function |
| `Moss_PostProcessExecute` | `Renderer::post_process_execute(...)` | `pub fn post_process_execute(&self, pass: &PostProcessPass) -> ()` | function |
| `Moss_FramebufferBegin` | `Renderer::framebuffer_begin(...)` | `pub fn framebuffer_begin(&mut self, framebuffer: &mut Framebuffer) -> ()` | function |
| `Moss_FramebufferEnd` | `Renderer::framebuffer_end(...)` | `pub fn framebuffer_end(&mut self, framebuffer: &mut Framebuffer) -> ()` | function |
| `Moss_TextureBarrier` | `Renderer::texture_barrier(...)` | `pub fn texture_barrier(&self, texture: &mut Texture, old_state: ResourceState, new_state: ResourceState) -> ()` | function |
| `Moss_RendererGetBackbuffer` | `Renderer::get_backbuffer(...)` | `pub fn get_backbuffer(&self) -> Result<Framebuffer>` | function |
| `Moss_RenderSubViewport` | `SubViewport::render(...)` | `pub fn render(&self, cmd: &mut GpuCommandBuffer, record: SubViewportRecordFn) -> ()` | function |
| `Moss_RendererCreate` | `renderer::create(...)` | `pub fn create(desc: &RendererDesc) -> Result<Renderer>` | function |
| `Moss_CreateRenderer` | `renderer::create_renderer(...)` | `pub fn create_renderer(window: &mut Window) -> Result<Renderer>` | function |
| `Moss_RendererDestroy` | `Renderer::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_TerminateRenderer` | `Renderer::close(...)` | `pub fn close(&self) -> ()` | function |
| `Moss_RendererBeginFrame` | `Renderer::begin_frame(...)` | `pub fn begin_frame(&mut self) -> ()` | function |
| `Moss_RendererBeginFrameEx` | `Renderer::begin_frame_ex(...)` | `pub fn begin_frame_ex(&mut self, desc: &RenderFrameDesc) -> ()` | function |
| `Moss_RendererEndFrame` | `Renderer::end_frame(...)` | `pub fn end_frame(&mut self) -> ()` | function |
| `Moss_RendererGetGPUDevice` | `Renderer::get_gpu_device(...)` | `pub fn get_gpu_device(&self) -> Result<GpuDevice>` | function |
| `Moss_MeshLoadFBX` | `renderer::mesh_load_fbx(...)` | `pub fn mesh_load_fbx(file_path: &str) -> Result<Mesh>` | function |
| `Moss_MeshLoadOBJ` | `renderer::mesh_load_obj(...)` | `pub fn mesh_load_obj(file_path: &str) -> Result<Mesh>` | function |
| `Moss_MeshLoadGLB` | `renderer::mesh_load_glb(...)` | `pub fn mesh_load_glb(file_path: &str) -> Result<Mesh>` | function |
| `Moss_ModelLoadFBX` | `renderer::model_load_fbx(...)` | `pub fn model_load_fbx(file_path: &str) -> Result<Model>` | function |
| `Moss_ModelLoadOBJ` | `renderer::model_load_obj(...)` | `pub fn model_load_obj(file_path: &str) -> Result<Model>` | function |
| `Moss_ModelLoadGLB` | `renderer::model_load_glb(...)` | `pub fn model_load_glb(file_path: &str) -> Result<Model>` | function |
| `Moss_ModelLoad` | `renderer::model_load(...)` | `pub fn model_load(file_path: &str) -> Result<Model>` | function |
| `Moss_ModelGetLastLoadError` | `renderer::model_get_last_load_error(...)` | `pub fn model_get_last_load_error() -> ModelLoadError` | function |
| `Moss_ModelGetLastLoadErrorMessage` | `renderer::model_get_last_load_error_message(...)` | `pub fn model_get_last_load_error_message() -> Option<String>` | function |
| `Moss_ModelFindBone` | `Model::find_bone(...)` | `pub fn find_bone(&self, name: &str) -> i32` | function |
| `Moss_ModelFindMorphTarget` | `Model::find_morph_target(...)` | `pub fn find_morph_target(&self, name: &str) -> i32` | function |
| `Moss_ModelSetBoneTransform` | `Model::set_bone_transform(...)` | `pub fn set_bone_transform(&mut self, bone_index: u32, transform16: &f32) -> bool` | function |
| `Moss_ModelSetMorphWeight` | `Model::set_morph_weight(...)` | `pub fn set_morph_weight(&mut self, morph_index: u32, weight: f32) -> bool` | function |
| `Moss_RendererDrawMesh` | `Renderer::draw_mesh(...)` | `pub fn draw_mesh(&self, desc: &DrawMeshDesc) -> ()` | function |
| `Moss_RendererDrawModelDesc` | `Renderer::draw_model_desc(...)` | `pub fn draw_model_desc(&self, desc: &DrawModelDesc) -> ()` | function |
| `Moss_RendererDrawModel` | `Renderer::draw_model(...)` | `pub fn draw_model(&self, model: &Model, transform: &f32) -> ()` | function |
| `Moss_RendererCreateFont` | `Renderer::create_font(...)` | `pub fn create_font(&mut self, desc: &FontDesc) -> Result<Font>` | function |
| `Moss_RendererDestroyFont` | `Renderer::destroy_font(...)` | `pub fn destroy_font(&mut self, font: &mut Font) -> ()` | function |
| `Moss_FontLoad` | `renderer::font_load(...)` | `pub fn font_load(path: &str, pixel_size: f32) -> Result<Font>` | function |
| `Moss_FontCreateFromMemory` | `renderer::font_create_from_memory(...)` | `pub fn font_create_from_memory(data: &[u8], data_size: usize, pixel_size: f32) -> Result<Font>` | function |
| `Moss_FontDestroy` | `Font::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_FontGetMetrics` | `Font::get_metrics(...)` | `pub fn get_metrics(&self) -> Result<FontMetrics>` | function |
| `Moss_FontGetGlyph` | `Font::get_glyph(...)` | `pub fn get_glyph(&self, codepoint: u32) -> Result<FontGlyph>` | function |
| `Moss_FontGetAtlasPixels` | `Font::get_atlas_pixels(...)` | `pub fn get_atlas_pixels(&self, width: &mut &mut u32, height: &mut &mut u32, stride: &mut &mut u32) -> Result<UnsignedChar>` | function |
| `Moss_FontGetAtlasTexture` | `Font::get_atlas_texture(...)` | `pub fn get_atlas_texture(&self) -> Result<Texture>` | function |
| `Moss_FontUploadAtlas` | `Renderer::font_upload_atlas(...)` | `pub fn font_upload_atlas(&mut self, font: &mut Font) -> bool` | function |
| `Moss_FontMeasureText` | `Font::measure_text(...)` | `pub fn measure_text(&self, text: &str, scale: f32) -> TextMeasure` | function |
| `Moss_FontBuildTextQuads` | `renderer::font_build_text_quads(...)` | `pub fn font_build_text_quads(desc: &DrawTextDesc, out_vertices: &mut FontVertex, vertex_capacity: usize) -> usize` | function |
| `Moss_RendererDrawText` | `Renderer::draw_text(...)` | `pub fn draw_text(&self, desc: &DrawTextDesc) -> ()` | function |
| `Moss_RendererGetFontBackendMask` | `renderer::get_font_backend_mask(...)` | `pub fn get_font_backend_mask() -> u32` | function |
| `Moss_RendererBackendSupportsFonts` | `renderer::backend_supports_fonts(...)` | `pub fn backend_supports_fonts(backend: GpuBackendType) -> bool` | function |
| `Moss_RendererDrawSprite` | `Renderer::draw_sprite(...)` | `pub fn draw_sprite(&self, desc: &SpriteDesc) -> ()` | function |
| `Moss_RendererCreateSpriteBatch` | `Renderer::create_sprite_batch(...)` | `pub fn create_sprite_batch(&mut self, capacity: u32) -> Result<SpriteBatch>` | function |
| `Moss_RendererDestroySpriteBatch` | `Renderer::destroy_sprite_batch(...)` | `pub fn destroy_sprite_batch(&mut self, batch: &mut SpriteBatch) -> ()` | function |
| `Moss_SpriteBatchClear` | `SpriteBatch::clear(...)` | `pub fn clear(&self) -> ()` | function |
| `Moss_SpriteBatchAdd` | `SpriteBatch::add(...)` | `pub fn add(&self, desc: &SpriteDesc) -> i32` | function |
| `Moss_RendererDrawSpriteBatch` | `Renderer::draw_sprite_batch(...)` | `pub fn draw_sprite_batch(&self, batch: &SpriteBatch) -> ()` | function |
| `Moss_RendererCreateDebugDrawList` | `Renderer::create_debug_draw_list(...)` | `pub fn create_debug_draw_list(&mut self, capacity: u32) -> Result<DebugDrawList>` | function |
| `Moss_RendererDestroyDebugDrawList` | `Renderer::destroy_debug_draw_list(...)` | `pub fn destroy_debug_draw_list(&mut self, list: &mut DebugDrawList) -> ()` | function |
| `Moss_DebugDrawListClear` | `DebugDrawList::clear(...)` | `pub fn clear(&self) -> ()` | function |
| `Moss_DebugDrawLine` | `DebugDrawList::debug_draw_line(...)` | `pub fn debug_draw_line(&self, desc: &DebugLineDesc) -> i32` | function |
| `Moss_DebugDrawBox` | `DebugDrawList::debug_draw_box(...)` | `pub fn debug_draw_box(&self, desc: &DebugBoxDesc) -> i32` | function |
| `Moss_DebugDrawSphere` | `DebugDrawList::debug_draw_sphere(...)` | `pub fn debug_draw_sphere(&self, desc: &DebugSphereDesc) -> i32` | function |
| `Moss_DebugDrawNavmesh` | `DebugDrawList::debug_draw_navmesh(...)` | `pub fn debug_draw_navmesh(&self, desc: &DebugNavmeshDesc) -> i32` | function |
| `Moss_DebugDrawPhysicsContact` | `DebugDrawList::debug_draw_physics_contact(...)` | `pub fn debug_draw_physics_contact(&self, desc: &DebugPhysicsContactDesc) -> i32` | function |
| `Moss_RendererDrawDebugList` | `Renderer::draw_debug_list(...)` | `pub fn draw_debug_list(&self, list: &DebugDrawList) -> ()` | function |
| `Moss_CameraCreate2D` | `renderer::camera_create_dimension2_d(...)` | `pub fn camera_create_dimension2_d(desc: &CameraDIMENSION2DDesc) -> Result<Camera>` | function |
| `Moss_CameraCreate3D` | `renderer::camera_create_dimension3_d(...)` | `pub fn camera_create_dimension3_d(desc: &CameraDIMENSION3DDesc) -> Result<Camera>` | function |
| `Moss_CameraCreateEditor` | `renderer::camera_create_editor(...)` | `pub fn camera_create_editor(desc: &EditorCameraDesc) -> Result<Camera>` | function |
| `Moss_CameraDestroy` | `Camera::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_RendererSetCamera` | `Renderer::set_camera(...)` | `pub fn set_camera(&mut self, camera: &mut Camera) -> ()` | function |
| `Moss_RendererCreateShadowMap` | `Renderer::create_shadow_map(...)` | `pub fn create_shadow_map(&mut self, desc: &ShadowMapDesc) -> Result<ShadowMap>` | function |
| `Moss_RendererDestroyShadowMap` | `Renderer::destroy_shadow_map(...)` | `pub fn destroy_shadow_map(&mut self, shadow_map: &mut ShadowMap) -> ()` | function |
| `Moss_RendererBeginShadowPass` | `Renderer::begin_shadow_pass(...)` | `pub fn begin_shadow_pass(&mut self, shadow_map: &mut ShadowMap) -> ()` | function |
| `Moss_RendererEndShadowPass` | `Renderer::end_shadow_pass(...)` | `pub fn end_shadow_pass(&mut self, shadow_map: &mut ShadowMap) -> ()` | function |
| `Moss_MeshDraw` | `Mesh::draw(...)` | `pub fn draw(&self) -> ()` | function |
| `Moss_ModelDraw` | `Model::draw(...)` | `pub fn draw(&self) -> ()` | function |
| `Moss_MeshRemove` | `Mesh::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_ModelRemove` | `Model::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_ResourceSetCreate` | `Renderer::create_resource_set(...)` | `pub fn create_resource_set(&mut self, desc: &ResourceSetLayoutDesc) -> Result<ResourceSet>` | function |
| `Moss_ResourceSetDestroy` | `ResourceSet::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_FramebufferCreate` | `Renderer::create_framebuffer(...)` | `pub fn create_framebuffer(&mut self, desc: &FramebufferDesc) -> Result<Framebuffer>` | function |
| `Moss_FramebufferDestroy` | `Framebuffer::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_RendererSetUpscaler` | `Renderer::set_upscaler(...)` | `pub fn set_upscaler(&mut self, upscaler: Upscaler) -> ()` | function |
| `Moss_RendererSetUpscalerDesc` | `Renderer::set_upscaler_desc(...)` | `pub fn set_upscaler_desc(&mut self, desc: &RendererUpscalerDesc) -> ()` | function |
| `Moss_GPUGetPhysicalDeviceName` | `renderer::get_physical_device_name(...)` | `pub fn get_physical_device_name() -> Option<String>` | function |
| `Moss_GPUAPIGetName` | `renderer::gpuapi_get_name(...)` | `pub fn gpuapi_get_name() -> Option<String>` | function |
| `Moss_GPUAPIGetVersion` | `renderer::gpuapi_get_version(...)` | `pub fn gpuapi_get_version() -> Option<String>` | function |
| `Moss_RendererDrawLine2D` | `Renderer::draw_line_dimension2_d(...)` | `pub fn draw_line_dimension2_d(&self, from: &Vec2, to: &Vec2, color: Color, thickness: f32) -> ()` | function |
| `Moss_RendererDrawLines2D` | `Renderer::draw_lines_dimension2_d(...)` | `pub fn draw_lines_dimension2_d(&self, positions: &TArray<Vec2>, color: Color, thickness: f32) -> ()` | function |
| `Moss_RendererDrawBox2D` | `Renderer::draw_box_dimension2_d(...)` | `pub fn draw_box_dimension2_d(&self, center: &[Vec2], rotation: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawCircle2D` | `Renderer::draw_circle_dimension2_d(...)` | `pub fn draw_circle_dimension2_d(&self, center: &Vec2, radius: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawCylinder2D` | `Renderer::draw_cylinder_dimension2_d(...)` | `pub fn draw_cylinder_dimension2_d(&self, matrix: &mut RMat44, half_height: f32, radius: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawTriangle2D` | `Renderer::draw_triangle_dimension2_d(...)` | `pub fn draw_triangle_dimension2_d(&self, p0: &Vec2, p1: &Vec2, p2: &Vec2, color: Color) -> ()` | function |
| `Moss_RendererDrawArrow2D` | `Renderer::draw_arrow_dimension2_d(...)` | `pub fn draw_arrow_dimension2_d(&self, from: &Vec2, to: &Vec2, color: Color, head_size: f32) -> ()` | function |
| `Moss_RendererDrawMarker2D` | `Renderer::draw_marker_dimension2_d(...)` | `pub fn draw_marker_dimension2_d(&self, position: &Vec2, color: Color, size: f32) -> ()` | function |
| `Moss_RendererDrawBone2D` | `Renderer::draw_bone_dimension2_d(...)` | `pub fn draw_bone_dimension2_d(&self, transform: &mut RMat44, length: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawCoordinateSystem2D` | `Renderer::draw_coordinate_system_dimension2_d(...)` | `pub fn draw_coordinate_system_dimension2_d(&self, matrix: &mut RMat44, size: f32) -> ()` | function |
| `Moss_RendererDrawCapsule2D` | `Renderer::draw_capsule_dimension2_d(...)` | `pub fn draw_capsule_dimension2_d(&self, matrix: &mut RMat44, half_height_of_cylinder: f32, radius: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawConvex2D` | `Renderer::draw_convex_dimension2_d(...)` | `pub fn draw_convex_dimension2_d(&self, points: &[Vec2], color: Color) -> ()` | function |
| `Moss_RendererDrawConvexHull2D` | `Renderer::draw_convex_hull_dimension2_d(...)` | `pub fn draw_convex_hull_dimension2_d(&self, points: &[Vec2], color: Color) -> ()` | function |
| `Moss_RendererDrawPolygonShape2D` | `Renderer::draw_polygon_shape_dimension2_d(...)` | `pub fn draw_polygon_shape_dimension2_d(&self, points: &[Vec2], color: Color) -> ()` | function |
| `Moss_RendererDrawGizmo2D` | `Renderer::draw_gizmo_dimension2_d(...)` | `pub fn draw_gizmo_dimension2_d(&self, transform: &mut RMat44, size: f32) -> ()` | function |
| `Moss_RendererDrawLineBatch2D` | `Renderer::draw_line_batch_dimension2_d(...)` | `pub fn draw_line_batch_dimension2_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawBoxBatch2D` | `Renderer::draw_box_batch_dimension2_d(...)` | `pub fn draw_box_batch_dimension2_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawCircleBatch2D` | `Renderer::draw_circle_batch_dimension2_d(...)` | `pub fn draw_circle_batch_dimension2_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawArrowBatch2D` | `Renderer::draw_arrow_batch_dimension2_d(...)` | `pub fn draw_arrow_batch_dimension2_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawMarkerBatch2D` | `Renderer::draw_marker_batch_dimension2_d(...)` | `pub fn draw_marker_batch_dimension2_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawBoxInstanced2D` | `Renderer::draw_box_instanced_dimension2_d(...)` | `pub fn draw_box_instanced_dimension2_d(&self, size: &Vec2, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawCircleInstanced2D` | `Renderer::draw_circle_instanced_dimension2_d(...)` | `pub fn draw_circle_instanced_dimension2_d(&self, radius: f32, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawCapsuleInstanced2D` | `Renderer::draw_capsule_instanced_dimension2_d(...)` | `pub fn draw_capsule_instanced_dimension2_d(&self, half_height_of_cylinder: f32, radius: f32, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawMarkerInstanced2D` | `Renderer::draw_marker_instanced_dimension2_d(...)` | `pub fn draw_marker_instanced_dimension2_d(&self, size: f32, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawGizmoInstanced2D` | `Renderer::draw_gizmo_instanced_dimension2_d(...)` | `pub fn draw_gizmo_instanced_dimension2_d(&self, size: f32, transform: &mut RMat44, count: u32) -> ()` | function |
| `Moss_RendererDrawLine3D` | `Renderer::draw_line_dimension3_d(...)` | `pub fn draw_line_dimension3_d(&self, from: &Vec3, to: &Vec3, color: Color, thickness: f32) -> ()` | function |
| `Moss_RendererDrawLines3D` | `Renderer::draw_lines_dimension3_d(...)` | `pub fn draw_lines_dimension3_d(&self, positions: &TArray<Vec3>, color: Color, thickness: f32) -> ()` | function |
| `Moss_RendererDrawBox3D` | `Renderer::draw_box_dimension3_d(...)` | `pub fn draw_box_dimension3_d(&self, center: &[Vec3], rotation: &mut RMat44, color: Color) -> ()` | function |
| `Moss_RendererDrawSphere3D` | `Renderer::draw_sphere_dimension3_d(...)` | `pub fn draw_sphere_dimension3_d(&self, center: &Vec3, radius: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawCylinder3D` | `Renderer::draw_cylinder_dimension3_d(...)` | `pub fn draw_cylinder_dimension3_d(&self, base: &Vec3, top: &Vec3, radius: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawCone3D` | `Renderer::draw_cone_dimension3_d(...)` | `pub fn draw_cone_dimension3_d(&self, base: &Vec3, tip: &Vec3, radius: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawTriangle3D` | `Renderer::draw_triangle_dimension3_d(...)` | `pub fn draw_triangle_dimension3_d(&self, p0: &Vec3, p1: &Vec3, p2: &Vec3, color: Color) -> ()` | function |
| `Moss_RendererDrawArrow3D` | `Renderer::draw_arrow_dimension3_d(...)` | `pub fn draw_arrow_dimension3_d(&self, from: &Vec3, to: &Vec3, color: Color, head_size: f32) -> ()` | function |
| `Moss_RendererDrawMarker3D` | `Renderer::draw_marker_dimension3_d(...)` | `pub fn draw_marker_dimension3_d(&self, position: &Vec3, color: Color, size: f32) -> ()` | function |
| `Moss_RendererDrawPlane3D` | `Renderer::draw_plane_dimension3_d(...)` | `pub fn draw_plane_dimension3_d(&self, center: &Vec3, normal: &[Vec3], color: Color) -> ()` | function |
| `Moss_RendererDrawBone3D` | `Renderer::draw_bone_dimension3_d(...)` | `pub fn draw_bone_dimension3_d(&self, transform: &mut RMat44, length: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawCoordinateSystem3D` | `Renderer::draw_coordinate_system_dimension3_d(...)` | `pub fn draw_coordinate_system_dimension3_d(&self, matrix: &mut RMat44, size: f32) -> ()` | function |
| `Moss_RendererDrawCapsule3D` | `Renderer::draw_capsule_dimension3_d(...)` | `pub fn draw_capsule_dimension3_d(&self, matrix: &mut RMat44, half_height_of_cylinder: f32, radius: f32, color: Color) -> ()` | function |
| `Moss_RendererDrawConvex3D` | `Renderer::draw_convex_dimension3_d(...)` | `pub fn draw_convex_dimension3_d(&self, points: &[Vec3], color: Color) -> ()` | function |
| `Moss_RendererDrawConvexHull3D` | `Renderer::draw_convex_hull_dimension3_d(...)` | `pub fn draw_convex_hull_dimension3_d(&self, points: &[Vec3], color: Color) -> ()` | function |
| `Moss_RendererDrawPolygonShape3D` | `Renderer::draw_polygon_shape_dimension3_d(...)` | `pub fn draw_polygon_shape_dimension3_d(&self, points: &[Vec3], color: Color) -> ()` | function |
| `Moss_RendererDrawGizmo3D` | `Renderer::draw_gizmo_dimension3_d(...)` | `pub fn draw_gizmo_dimension3_d(&self, transform: &mut RMat44, size: f32) -> ()` | function |
| `Moss_RendererDrawLineBatch3D` | `Renderer::draw_line_batch_dimension3_d(...)` | `pub fn draw_line_batch_dimension3_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawBoxBatch3D` | `Renderer::draw_box_batch_dimension3_d(...)` | `pub fn draw_box_batch_dimension3_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawSphereBatch3D` | `Renderer::draw_sphere_batch_dimension3_d(...)` | `pub fn draw_sphere_batch_dimension3_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawArrowBatch3D` | `Renderer::draw_arrow_batch_dimension3_d(...)` | `pub fn draw_arrow_batch_dimension3_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawMarkerBatch3D` | `Renderer::draw_marker_batch_dimension3_d(...)` | `pub fn draw_marker_batch_dimension3_d(&self, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawBoxInstanced3D` | `Renderer::draw_box_instanced_dimension3_d(...)` | `pub fn draw_box_instanced_dimension3_d(&self, size: &Vec3, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawSphereInstanced3D` | `Renderer::draw_sphere_instanced_dimension3_d(...)` | `pub fn draw_sphere_instanced_dimension3_d(&self, radius: f32, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawCapsuleInstanced3D` | `Renderer::draw_capsule_instanced_dimension3_d(...)` | `pub fn draw_capsule_instanced_dimension3_d(&self, half_height_of_cylinder: f32, radius: f32, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawCylinderInstanced3D` | `Renderer::draw_cylinder_instanced_dimension3_d(...)` | `pub fn draw_cylinder_instanced_dimension3_d(&self, half_height: f32, radius: f32, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawConeInstanced3D` | `Renderer::draw_cone_instanced_dimension3_d(...)` | `pub fn draw_cone_instanced_dimension3_d(&self, height: f32, radius: f32, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawMarkerInstanced3D` | `Renderer::draw_marker_instanced_dimension3_d(...)` | `pub fn draw_marker_instanced_dimension3_d(&self, size: f32, transform: &mut RMat44, colour: Color, count: u32) -> ()` | function |
| `Moss_RendererDrawGizmoInstanced3D` | `Renderer::draw_gizmo_instanced_dimension3_d(...)` | `pub fn draw_gizmo_instanced_dimension3_d(&self, size: f32, transform: &mut RMat44, count: u32) -> ()` | function |
| `Moss_VulkanGetDevice` | `Renderer::get_device(...)` | `pub fn get_device(&self) -> VkDevice` | function |
| `Moss_VulkanGetDescriptorPool` | `Renderer::get_descriptor_pool(...)` | `pub fn get_descriptor_pool(&self) -> VkDescriptorPool` | function |
| `Moss_VulkanGetDescriptorSetLayoutTexture` | `Renderer::get_descriptor_set_layout_texture(...)` | `pub fn get_descriptor_set_layout_texture(&mut self) -> VkDescriptorSetLayout` | function |
| `Moss_VulkanGetTextureSamplerRepeat` | `Renderer::get_texture_sampler_repeat(...)` | `pub fn get_texture_sampler_repeat(&self) -> VkSampler` | function |
| `Moss_VulkanGetTextureSamplerShadow` | `Renderer::get_texture_sampler_shadow(...)` | `pub fn get_texture_sampler_shadow(&self) -> VkSampler` | function |
| `Moss_VulkanGetRenderPassShadow` | `Renderer::get_render_pass_shadow(...)` | `pub fn get_render_pass_shadow(&self) -> VkRenderPass` | function |
| `Moss_VulkanGetRenderPass` | `Renderer::get_render_pass(...)` | `pub fn get_render_pass(&self) -> VkRenderPass` | function |
| `Moss_VulkanGetPipelineLayout` | `Renderer::get_pipeline_layout(...)` | `pub fn get_pipeline_layout(&self) -> VkPipelineLayout` | function |
| `Moss_VulkanGetCommandBuffer` | `Renderer::get_command_buffer(...)` | `pub fn get_command_buffer(&self) -> VkCommandBuffer` | function |
| `Moss_VulkanStartTempCommandBuffer` | `Renderer::start_temp_command_buffer(...)` | `pub fn start_temp_command_buffer(&self) -> VkCommandBuffer` | function |
| `Moss_VulkanEndTempCommandBuffer` | `Renderer::end_temp_command_buffer(...)` | `pub fn end_temp_command_buffer(&mut self, command_buffer: VkCommandBuffer) -> ()` | function |
| `Moss_VulkanAllocateMemory` | `Renderer::allocate_memory(...)` | `pub fn allocate_memory(&self, size: VkDeviceSize, memory_type_bits: u32, properties: VkMemoryPropertyFlags, out_memory: &mut VkDeviceMemory) -> ()` | function |
| `Moss_VulkanFreeMemory` | `Renderer::free_memory(...)` | `pub fn free_memory(&self, memory: VkDeviceMemory, size: VkDeviceSize) -> ()` | function |
| `Moss_VulkanCreateBuffer` | `Renderer::create_buffer(...)` | `pub fn create_buffer(&mut self, size: VkDeviceSize, usage: VkBufferUsageFlags, properties: VkMemoryPropertyFlags, out_buffer: &mut BufferVK) -> ()` | function |
| `Moss_VulkanCopyBuffer` | `Renderer::copy_buffer(...)` | `pub fn copy_buffer(&self, src: VkBuffer, dst: VkBuffer, size: VkDeviceSize) -> ()` | function |
| `Moss_VulkanCreateDeviceLocalBuffer` | `Renderer::create_device_local_buffer(...)` | `pub fn create_device_local_buffer(&mut self, data: &[u8], size: VkDeviceSize, usage: VkBufferUsageFlags, out_buffer: &mut BufferVK) -> ()` | function |
| `Moss_VulkanFreeBuffer` | `Renderer::free_buffer(...)` | `pub fn free_buffer(&self, buffer: &mut BufferVK) -> ()` | function |
| `Moss_VulkanCreateConstantBuffer` | `Renderer::create_constant_buffer(...)` | `pub fn create_constant_buffer(&mut self, buffer_size: VkDeviceSize) -> Result<ConstantBufferVK>` | function |
| `Moss_VulkanCreateImage` | `Renderer::create_image(...)` | `pub fn create_image(&mut self, width: u32, height: u32, format: VkFormat, tiling: VkImageTiling, usage: VkImageUsageFlags, properties: VkMemoryPropertyFlags, out_image: &mut VkImage, out_memory: &mut VkDeviceMemory) -> ()` | function |
| `Moss_VulkanDestroyImage` | `Renderer::destroy_image(...)` | `pub fn destroy_image(&mut self, image: VkImage, memory: VkDeviceMemory) -> ()` | function |
| `Moss_VulkanCreateImageView` | `Renderer::create_image_view(...)` | `pub fn create_image_view(&mut self, image: VkImage, format: VkFormat, aspect_flags: VkImageAspectFlags) -> VkImageView` | function |
| `Moss_VulkanFindDepthFormat` | `Renderer::find_depth_format(...)` | `pub fn find_depth_format(&self) -> VkFormat` | function |
| `Moss_Vulkan_EncodeSPIRV` | `renderer::encode_spirv(...)` | `pub fn encode_spirv(path: &str) -> ()` | function |
| `Moss_Vulkan_DecodeSPIRV` | `renderer::decode_spirv(...)` | `pub fn decode_spirv(path: &str) -> ()` | function |
| `Moss_DX12GetDevice` | `Renderer::dx12_get_device(...)` | `pub fn dx12_get_device(&self) -> Result<IdDIMENSION3D1DIMENSION2Device>` | function |
| `Moss_DX12GetRootSignature` | `Renderer::dx12_get_root_signature(...)` | `pub fn dx12_get_root_signature(&self) -> Result<IdDIMENSION3D12RootSignature>` | function |
| `Moss_DX12GetCommandList` | `Renderer::dx12_get_command_list(...)` | `pub fn dx12_get_command_list(&self) -> Result<IdDIMENSION3D12GraphicsCommandList>` | function |
| `Moss_DX12GetUploadQueue` | `Renderer::dx12_get_upload_queue(...)` | `pub fn dx12_get_upload_queue(&mut self) -> Result<CommandQueueDX12>` | function |
| `Moss_DX12GetDSVHeap` | `Renderer::dx12_get_dsv_heap(...)` | `pub fn dx12_get_dsv_heap(&self) -> Result<DescriptorHeapDX12>` | function |
| `Moss_DX12GetSRVHeap` | `Renderer::dx12_get_srv_heap(...)` | `pub fn dx12_get_srv_heap(&self) -> Result<DescriptorHeapDX12>` | function |
| `Moss_DX_CompileHLSL` | `renderer::compile_hlsl(...)` | `pub fn compile_hlsl(input_path: &str, output_path: &str, entry_point: &str, target: &str) -> bool` | function |
| `Moss_DX_EnableDebugLayer` | `renderer::enable_debug_layer(...)` | `pub fn enable_debug_layer(enable: bool) -> ()` | function |
| `Moss_DX_GetShaderModel` | `renderer::get_shader_model(...)` | `pub fn get_shader_model() -> u32` | function |
| `Moss_MetalGetView` | `Renderer::get_view(...)` | `pub fn get_view(&self) -> Result<MTKView>` | function |
| `Moss_MetalGetDevice` | `Renderer::get_device(...)` | `pub fn get_device(&self) -> &mut [u8]` | function |
| `Moss_MetalGetRenderEncoder` | `Renderer::get_render_encoder(...)` | `pub fn get_render_encoder(&self) -> &mut [u8]` | function |
| `Moss_Metal_CompileMSL` | `renderer::compile_msl(...)` | `pub fn compile_msl(input_path: &str, output_path: &str) -> bool` | function |
| `Moss_Metal_SupportsFamily` | `renderer::supports_family(...)` | `pub fn supports_family(family: u32) -> bool` | function |

### `Moss_stdinc.h` → `moss::core`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `Allocate` | `core::allocator::allocate` | `pub type Allocate = unsafe extern "C" fn(...)` | callback |
| `Reallocate` | `core::allocator::reallocate` | `pub type Reallocate = unsafe extern "C" fn(...)` | callback |
| `Free` | `core::allocator::free` | `pub type Free = unsafe extern "C" fn(...)` | callback |
| `AlignedAllocate` | `core::allocator::aligned_allocate` | `pub type AlignedAllocate = unsafe extern "C" fn(...)` | callback |
| `AlignedFree` | `core::allocator::aligned_free` | `pub type AlignedFree = unsafe extern "C" fn(...)` | callback |
| `RegisterDefaultAllocator` | `core::register_default_allocator(...)` | `pub fn register_default_allocator() -> ()` | function |
| `Allocate` | `core::allocate(...)` | `pub fn allocate(in_size: usize) -> &mut [u8]` | function |
| `Reallocate` | `core::reallocate(...)` | `pub fn reallocate(in_block: &mut [u8], in_old_size: usize, in_new_size: usize) -> &mut [u8]` | function |
| `Free` | `core::free(...)` | `pub fn free(in_block: &mut [u8]) -> ()` | function |
| `AlignedAllocate` | `core::aligned_allocate(...)` | `pub fn aligned_allocate(in_size: usize, in_alignment: usize) -> &mut [u8]` | function |
| `AlignedFree` | `core::aligned_free(...)` | `pub fn aligned_free(in_block: &mut [u8]) -> ()` | function |
| `Moss_rand` | `core::rand(...)` | `pub fn rand(n: Int32) -> Int32` | function |
| `Moss_randf` | `core::randf(...)` | `pub fn randf() -> f32` | function |
| `Moss_randi` | `core::randi(...)` | `pub fn randi() -> i32` | function |
| `Moss_srand` | `core::srand(...)` | `pub fn srand(seed: Int64) -> ()` | function |
| `Moss_randi_range` | `core::randi_range(...)` | `pub fn randi_range(from: Int32, to: Int32) -> Int32` | function |
| `Moss_randf_range` | `core::randf_range(...)` | `pub fn randf_range(from: f32, to: f32) -> f32` | function |
| `Moss_rand_gaussian` | `core::rand_gaussian(...)` | `pub fn rand_gaussian(mean: f32, stddev: f32) -> f32` | function |
| `Moss_rand_unit` | `core::rand_unit(...)` | `pub fn rand_unit() -> f32` | function |
| `Moss_GetTicks` | `core::get_ticks(...)` | `pub fn get_ticks() -> Time` | function |
| `Moss_GetSeconds` | `core::get_seconds(...)` | `pub fn get_seconds(time: Time) -> f64` | function |
| `Moss_GetMilliseconds` | `core::get_milliseconds(...)` | `pub fn get_milliseconds(ticks: Time) -> f32` | function |
| `Moss_GetMillisecondsAndReset` | `core::get_milliseconds_and_reset(...)` | `pub fn get_milliseconds_and_reset(ticks: &mut Time) -> f32` | function |
| `Moss_Yield` | `core::yield(...)` | `pub fn yield() -> ()` | function |
| `Moss_Delay` | `core::delay(...)` | `pub fn delay(delay: f64) -> f64` | function |
| `Moss_LocalTime` | `core::local_time(...)` | `pub fn local_time() -> Option<String>` | function |
| `Moss_TimeStamp` | `core::time_stamp(...)` | `pub fn time_stamp() -> Option<String>` | function |
| `Moss_TimeNow` | `core::time_now(...)` | `pub fn time_now() -> Option<String>` | function |
| `Moss_CTimeNow` | `core::c_time_now(...)` | `pub fn c_time_now() -> Option<String>` | function |
| `Moss_TimeStampET` | `core::time_stamp_et(...)` | `pub fn time_stamp_et() -> Option<String>` | function |
| `Moss_TimeStampCT` | `core::time_stamp_ct(...)` | `pub fn time_stamp_ct() -> Option<String>` | function |
| `Moss_TimeStampMT` | `core::time_stamp_mt(...)` | `pub fn time_stamp_mt() -> Option<String>` | function |
| `Moss_TimeStampPT` | `core::time_stamp_pt(...)` | `pub fn time_stamp_pt() -> Option<String>` | function |
| `Moss_FormatTime` | `core::format_time(...)` | `pub fn format_time(fmt: &str) -> Option<String>` | function |

### `Moss_XR.h` → `moss::xr`

| Native symbol (traceability) | Public API projection | Target-language signature | Kind |
|---|---|---|---|
| `Moss_XR_Initialize` | `xr::initialize(...)` | `pub fn initialize(&self) -> bool` | function |
| `Moss_XR_Shutdown` | `xr::shutdown(...)` | `pub fn shutdown() -> ()` | function |
| `Moss_XR_GetCapabilities` | `xr::get_capabilities(...)` | `pub fn get_capabilities() -> Result<XrCapabilities>` | function |
| `Moss_XR_EnableValidationLayers` | `xr::enable_validation_layers(...)` | `pub fn enable_validation_layers() -> bool` | function |
| `Moss_XR_CreateSession` | `xr::create_session(...)` | `pub fn create_session() -> Result<XrSession>` | function |
| `Moss_XR_DestroySession` | `XrSession::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_XR_GetSessionState` | `XrSession::get_state(...)` | `pub fn get_state(&self) -> XrSessionState` | function |
| `Moss_XR_BeginFrame` | `XrSession::begin_frame(...)` | `pub fn begin_frame(&mut self) -> bool` | function |
| `Moss_XR_EndFrame` | `XrSession::end_frame(...)` | `pub fn end_frame(&mut self) -> ()` | function |
| `Moss_XR_GetPredictedDisplayTime` | `XrSession::get_predicted_display_time(...)` | `pub fn get_predicted_display_time(&self) -> XrTime` | function |
| `Moss_XR_GetDeltaSeconds` | `xr::get_delta_seconds(...)` | `pub fn get_delta_seconds() -> f32` | function |
| `Moss_XR_HandleEvents` | `XrSession::handle_events(...)` | `pub fn handle_events(&self) -> ()` | function |
| `Moss_XR_GetViewCount` | `XrSession::get_view_count(...)` | `pub fn get_view_count(&self) -> u32` | function |
| `Moss_XR_GetView` | `XrSession::get_view(...)` | `pub fn get_view(&self, index: u32, out_view: &mut XrView) -> bool` | function |
| `Moss_XR_CreateActionSet` | `xr::create_action_set(...)` | `pub fn create_action_set(name: &str) -> Result<XrActionSet>` | function |
| `Moss_XR_DestroyActionSet` | `XrActionSet::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_XR_CreateAction` | `XrActionSet::create_action(...)` | `pub fn create_action(&mut self, name: &str, type: XrActionType) -> Result<XrAction>` | function |
| `Moss_XR_DestroyAction` | `XrAction::close(...)` | `pub fn close(&mut self) -> ()` | function |
| `Moss_XR_AttachActionSet` | `XrActionSet::attach(...)` | `pub fn attach(&mut self, session: &mut XrSession) -> ()` | function |
| `Moss_XR_SyncActions` | `XrSession::sync_actions(...)` | `pub fn sync_actions(&self) -> ()` | function |
| `Moss_XR_GetActionBoolean` | `XrAction::get_boolean(...)` | `pub fn get_boolean(&self) -> bool` | function |
| `Moss_XR_GetActionFloat` | `XrAction::get_float(...)` | `pub fn get_float(&self) -> f32` | function |
| `Moss_XR_GetActionPose` | `XrAction::get_pose(...)` | `pub fn get_pose(&self, out_pose: &mut XrPose) -> bool` | function |
| `Moss_XR_StopHaptic` | `XrAction::stop_haptic(...)` | `pub fn stop_haptic(&mut self) -> ()` | function |
| `Moss_XR_CreateHand` | `xr::create_hand(...)` | `pub fn create_hand() -> Result<XrHandModifier>` | function |
| `Moss_XR_GetHandJointPose` | `XrHandModifier::get_hand_joint_pose(...)` | `pub fn get_hand_joint_pose(&self, joint: XrHandJoint, out_pose: &mut XrPose) -> bool` | function |
| `Moss_XR_DestroyHand` | `XrHandModifier::destroy_hand(...)` | `pub fn destroy_hand(&mut self) -> ()` | function |
| `Moss_XR_GetHandPose` | `XrHandModifier::get_hand_pose(...)` | `pub fn get_hand_pose(&self, out_pose: &mut XrPose) -> bool` | function |
| `Moss_XR_CreateBody` | `xr::create_body(...)` | `pub fn create_body() -> Result<XrBodyModifier>` | function |
| `Moss_XR_GetBodyJointPose` | `XrBodyModifier::get_body_joint_pose(...)` | `pub fn get_body_joint_pose(&self, joint: XrBodyJoint, out_pose: &mut XrPose) -> bool` | function |
| `Moss_XR_DestroyBody` | `XrBodyModifier::destroy_body(...)` | `pub fn destroy_body(&mut self) -> ()` | function |
| `Moss_XR_CreateFace` | `xr::create_face(...)` | `pub fn create_face() -> Result<XrFaceModifier>` | function |
| `Moss_XR_DestroyFace` | `XrFaceModifier::destroy_face(...)` | `pub fn destroy_face(&mut self) -> ()` | function |
| `Moss_XR_UpdateBodyTracking` | `XrSession::update_body_tracking(...)` | `pub fn update_body_tracking(&mut self) -> ()` | function |
| `Moss_XR_UpdateHandTracking` | `XrSession::update_hand_tracking(...)` | `pub fn update_hand_tracking(&mut self) -> ()` | function |
| `Moss_XR_EnablePassthrough` | `xr::enable_passthrough(...)` | `pub fn enable_passthrough(enable: bool) -> bool` | function |
| `Moss_XR_EnableFoveatedRendering` | `xr::enable_foveated_rendering(...)` | `pub fn enable_foveated_rendering(enable: bool) -> bool` | function |
| `Moss_XR_GetBackendName` | `xr::get_backend_name(...)` | `pub fn get_backend_name() -> Option<String>` | function |
| `Moss_XR_GetBackendVersion` | `xr::get_backend_version(...)` | `pub fn get_backend_version() -> u32` | function |
| `Moss_XR_BeginDebugLabel` | `xr::begin_debug_label(...)` | `pub fn begin_debug_label(label: &str) -> ()` | function |
| `Moss_XR_EndDebugLabel` | `xr::end_debug_label(...)` | `pub fn end_debug_label() -> ()` | function |


## 8. Native header and feature coverage

The public binding must cover the complete Moss header tree, not only the exported function index. Many Core, Geometry, Physics, Renderer, Resources, Skeleton, TriangleSplitter and Variants APIs are class members, templates, inline methods, or declarations not represented by the top-level free-function catalogue.

## Complete header inventory

Each link opens the corresponding source file in `include/Moss/`. The rightmost count shows declarations beginning with `MOSS_API` in that header; zero does not mean the header has no public API (it may expose class members, templates, inline methods, typedefs, or preprocessor declarations).

### Core (40 headers)

Foundational C++ utilities.

| Header | Export-marked declarations |
|---|---:|
| [`Core/BinaryHeap.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/BinaryHeap.h) | 0 |
| [`Core/ByteBuffer.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/ByteBuffer.h) | 0 |
| [`Core/FPControlWord.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/FPControlWord.h) | 0 |
| [`Core/FPException.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/FPException.h) | 0 |
| [`Core/FPFlushDenormals.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/FPFlushDenormals.h) | 0 |
| [`Core/Factory.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/Factory.h) | 0 |
| [`Core/FixedSizeFreeList.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/FixedSizeFreeList.h) | 0 |
| [`Core/FreeBlockAlloc.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/FreeBlockAlloc.h) | 0 |
| [`Core/HashCombine.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/HashCombine.h) | 0 |
| [`Core/HashTable.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/HashTable.h) | 0 |
| [`Core/InsertionSort.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/InsertionSort.h) | 0 |
| [`Core/JobSystem.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/JobSystem.h) | 0 |
| [`Core/JobSystemSingleThreaded.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/JobSystemSingleThreaded.h) | 0 |
| [`Core/JobSystemThreadPool.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/JobSystemThreadPool.h) | 0 |
| [`Core/JobSystemWithBarrier.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/JobSystemWithBarrier.h) | 0 |
| [`Core/LinearCurve.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/LinearCurve.h) | 0 |
| [`Core/LockFreeHashMap.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/LockFreeHashMap.h) | 0 |
| [`Core/Memory.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/Memory.h) | 11 |
| [`Core/Mutex.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/Mutex.h) | 0 |
| [`Core/MutexArray.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/MutexArray.h) | 0 |
| [`Core/NonCopyable.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/NonCopyable.h) | 0 |
| [`Core/Profiler.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/Profiler.h) | 4 |
| [`Core/QuickSort.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/QuickSort.h) | 0 |
| [`Core/RTTI.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/RTTI.h) | 0 |
| [`Core/Reference.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/Reference.h) | 0 |
| [`Core/Result.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/Result.h) | 0 |
| [`Core/STLAlignedAllocator.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/STLAlignedAllocator.h) | 0 |
| [`Core/STLAllocator.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/STLAllocator.h) | 0 |
| [`Core/STLLocalAllocator.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/STLLocalAllocator.h) | 0 |
| [`Core/STLTempAllocator.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/STLTempAllocator.h) | 0 |
| [`Core/ScopeExit.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/ScopeExit.h) | 0 |
| [`Core/Semaphore.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/Semaphore.h) | 0 |
| [`Core/StreamIn.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/StreamIn.h) | 0 |
| [`Core/StreamOut.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/StreamOut.h) | 0 |
| [`Core/StreamUtils.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/StreamUtils.h) | 0 |
| [`Core/StreamWrapper.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/StreamWrapper.h) | 0 |
| [`Core/StridedPtr.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/StridedPtr.h) | 0 |
| [`Core/StringTools.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/StringTools.h) | 6 |
| [`Core/TempAllocator.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/TempAllocator.h) | 0 |
| [`Core/TickCounter.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/TickCounter.h) | 0 |

### Geometry (9 headers)

Geometry algorithms and primitives.

| Header | Export-marked declarations |
|---|---:|
| [`Geometry/ClipPoly.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/ClipPoly.h) | 0 |
| [`Geometry/ClosestPoint.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/ClosestPoint.h) | 0 |
| [`Geometry/ConvexHullBuilder.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/ConvexHullBuilder.h) | 0 |
| [`Geometry/ConvexSupport.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/ConvexSupport.h) | 0 |
| [`Geometry/EPAConvexHullBuilder.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/EPAConvexHullBuilder.h) | 0 |
| [`Geometry/EPAPenetrationDepth.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/EPAPenetrationDepth.h) | 0 |
| [`Geometry/GJKClosestPoint.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/GJKClosestPoint.h) | 0 |
| [`Geometry/RayCylinder.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/RayCylinder.h) | 0 |
| [`Geometry/Triangle.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Geometry/Triangle.h) | 0 |

### Physics (69 headers)

Simulation, body, shape, collision, constraint and query implementation headers.

| Header | Export-marked declarations |
|---|---:|
| [`Physics/AABBTree/AABBTreeBuilder.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/AABBTree/AABBTreeBuilder.h) | 0 |
| [`Physics/AABBTree/AABBTreeToBuffer.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/AABBTree/AABBTreeToBuffer.h) | 0 |
| [`Physics/AABBTree/NodeCodec/NodeCodecQuadTreeHalfFloat.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/AABBTree/NodeCodec/NodeCodecQuadTreeHalfFloat.h) | 0 |
| [`Physics/AABBTree/NodeCodec/NodeCodecQuadTreeHalfFloat2D.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/AABBTree/NodeCodec/NodeCodecQuadTreeHalfFloat2D.h) | 0 |
| [`Physics/AABBTree/TriangleCodec/TriangleCodecIndexed8BitPackSOA4Flags.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/AABBTree/TriangleCodec/TriangleCodecIndexed8BitPackSOA4Flags.h) | 0 |
| [`Physics/AABBTree/TriangleCodec/TriangleCodecIndexed8BitPackSOA4Flags2D.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/AABBTree/TriangleCodec/TriangleCodecIndexed8BitPackSOA4Flags2D.h) | 0 |
| [`Physics/Body/Body.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Body/Body.h) | 1 |
| [`Physics/Body/BodyID.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Body/BodyID.h) | 0 |
| [`Physics/Character/Character.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Character/Character.h) | 0 |
| [`Physics/Cloth/Cloth.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Cloth/Cloth.h) | 0 |
| [`Physics/Collision/ActiveEdges.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/ActiveEdges.h) | 0 |
| [`Physics/Collision/BroadPhase/BroadPhase.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/BroadPhase/BroadPhase.h) | 0 |
| [`Physics/Collision/BroadPhase/BroadPhaseLayerInterfaceMask.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/BroadPhase/BroadPhaseLayerInterfaceMask.h) | 0 |
| [`Physics/Collision/BroadPhase/QuadTree.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/BroadPhase/QuadTree.h) | 0 |
| [`Physics/Collision/CastConvexVsTriangles.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CastConvexVsTriangles.h) | 0 |
| [`Physics/Collision/CastResult.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CastResult.h) | 0 |
| [`Physics/Collision/CastSphereVsTriangles.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CastSphereVsTriangles.h) | 0 |
| [`Physics/Collision/CollideConvexVsTriangles.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollideConvexVsTriangles.h) | 0 |
| [`Physics/Collision/CollidePointResult.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollidePointResult.h) | 0 |
| [`Physics/Collision/CollideShape.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollideShape.h) | 0 |
| [`Physics/Collision/CollideShapeVsShapePerLeaf.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollideShapeVsShapePerLeaf.h) | 0 |
| [`Physics/Collision/CollideSoftBodyVertexIterator.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollideSoftBodyVertexIterator.h) | 0 |
| [`Physics/Collision/CollideSoftBodyVerticesVsTriangles.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollideSoftBodyVerticesVsTriangles.h) | 0 |
| [`Physics/Collision/CollideSphereVsTriangles.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollideSphereVsTriangles.h) | 0 |
| [`Physics/Collision/CollisionCollector.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollisionCollector.h) | 0 |
| [`Physics/Collision/CollisionCollectorImpl.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollisionCollectorImpl.h) | 0 |
| [`Physics/Collision/CollisionDispatch.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollisionDispatch.h) | 0 |
| [`Physics/Collision/CollisionGroup.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/CollisionGroup.h) | 0 |
| [`Physics/Collision/ContactListener.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/ContactListener.h) | 0 |
| [`Physics/Collision/EstimateCollisionResponse.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/EstimateCollisionResponse.h) | 1 |
| [`Physics/Collision/GroupFilter.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/GroupFilter.h) | 0 |
| [`Physics/Collision/GroupFilterTable.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/GroupFilterTable.h) | 0 |
| [`Physics/Collision/InternalEdgeRemovingCollector.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/InternalEdgeRemovingCollector.h) | 0 |
| [`Physics/Collision/ManifoldBetweenTwoFaces.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/ManifoldBetweenTwoFaces.h) | 2 |
| [`Physics/Collision/NarrowPhaseQuery.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/NarrowPhaseQuery.h) | 0 |
| [`Physics/Collision/NarrowPhaseStats.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/NarrowPhaseStats.h) | 0 |
| [`Physics/Collision/ObjectLayer.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/ObjectLayer.h) | 0 |
| [`Physics/Collision/ObjectLayerPairFilterMask.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/ObjectLayerPairFilterMask.h) | 0 |
| [`Physics/Collision/ObjectLayerPairFilterTable.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/ObjectLayerPairFilterTable.h) | 0 |
| [`Physics/Collision/PhysicsMaterial.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/PhysicsMaterial.h) | 0 |
| [`Physics/Collision/PhysicsMaterialSimple.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/PhysicsMaterialSimple.h) | 0 |
| [`Physics/Collision/RayCast.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/RayCast.h) | 0 |
| [`Physics/Collision/Shape/CompoundShapeVisitors.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/Shape/CompoundShapeVisitors.h) | 0 |
| [`Physics/Collision/Shape/ConvexHullShape.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/Shape/ConvexHullShape.h) | 0 |
| [`Physics/Collision/Shape/GetTrianglesContext.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/Shape/GetTrianglesContext.h) | 0 |
| [`Physics/Collision/Shape/PolyhedronSubmergedVolumeCalculator.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/Shape/PolyhedronSubmergedVolumeCalculator.h) | 0 |
| [`Physics/Collision/Shape/Shapes.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/Shape/Shapes.h) | 0 |
| [`Physics/Collision/Shape/SubShapeID.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/Shape/SubShapeID.h) | 0 |
| [`Physics/Collision/Shape/SubShapeIDPair.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/Shape/SubShapeIDPair.h) | 0 |
| [`Physics/Collision/ShapeCast.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/ShapeCast.h) | 0 |
| [`Physics/Collision/ShapeFilter.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/ShapeFilter.h) | 0 |
| [`Physics/Collision/SimShapeFilter.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/SimShapeFilter.h) | 0 |
| [`Physics/Collision/SimShapeFilterWrapper.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/SimShapeFilterWrapper.h) | 0 |
| [`Physics/Collision/SortReverseAndStore.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/SortReverseAndStore.h) | 0 |
| [`Physics/Collision/TransformedShape.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Collision/TransformedShape.h) | 0 |
| [`Physics/Constraints/ConstraintManager.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Constraints/ConstraintManager.h) | 0 |
| [`Physics/Constraints/Constraints.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Constraints/Constraints.h) | 0 |
| [`Physics/Hair/Hair.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Hair/Hair.h) | 0 |
| [`Physics/IslandBuilder.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/IslandBuilder.h) | 0 |
| [`Physics/LargeIslandSplitter.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/LargeIslandSplitter.h) | 0 |
| [`Physics/PhysicsLock.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/PhysicsLock.h) | 0 |
| [`Physics/PhysicsScene.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/PhysicsScene.h) | 0 |
| [`Physics/PhysicsSystem.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/PhysicsSystem.h) | 0 |
| [`Physics/PhysicsUpdateContext.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/PhysicsUpdateContext.h) | 0 |
| [`Physics/Ragdoll/Ragdoll.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Ragdoll/Ragdoll.h) | 0 |
| [`Physics/SoftBody/Softbody.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/SoftBody/Softbody.h) | 0 |
| [`Physics/StateRecorder.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/StateRecorder.h) | 0 |
| [`Physics/StateRecorderImpl.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/StateRecorderImpl.h) | 0 |
| [`Physics/Vehicle/Vehicle.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Vehicle/Vehicle.h) | 0 |

### Renderer (65 headers)

Renderer resources and graphics backend headers.

| Header | Export-marked declarations |
|---|---:|
| [`Renderer/DX12/CommandQueueDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/CommandQueueDX12.h) | 0 |
| [`Renderer/DX12/ConstantBufferDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/ConstantBufferDX12.h) | 0 |
| [`Renderer/DX12/DescriptorHeapDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/DescriptorHeapDX12.h) | 0 |
| [`Renderer/DX12/LightingDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/LightingDX12.h) | 0 |
| [`Renderer/DX12/PipelineStateDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/PipelineStateDX12.h) | 0 |
| [`Renderer/DX12/RenderInstancesDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/RenderInstancesDX12.h) | 0 |
| [`Renderer/DX12/RenderPrimitiveDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/RenderPrimitiveDX12.h) | 0 |
| [`Renderer/DX12/RendererDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/RendererDX12.h) | 0 |
| [`Renderer/DX12/ShaderDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/ShaderDX12.h) | 0 |
| [`Renderer/DX12/TextureDX12.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DX12/TextureDX12.h) | 0 |
| [`Renderer/DebugRenderer.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DebugRenderer.h) | 0 |
| [`Renderer/DebugRendererPlayback.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DebugRendererPlayback.h) | 0 |
| [`Renderer/DebugRendererRecorder.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DebugRendererRecorder.h) | 0 |
| [`Renderer/DebugRendererSimple.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/DebugRendererSimple.h) | 0 |
| [`Renderer/FogVolume.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/FogVolume.h) | 0 |
| [`Renderer/Font.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Font.h) | 0 |
| [`Renderer/GL/FontGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/FontGL.h) | 0 |
| [`Renderer/GL/LightingGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/LightingGL.h) | 0 |
| [`Renderer/GL/MeshGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/MeshGL.h) | 0 |
| [`Renderer/GL/PipelineStateGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/PipelineStateGL.h) | 0 |
| [`Renderer/GL/PostProcess.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/PostProcess.h) | 0 |
| [`Renderer/GL/ReflectionProbe.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/ReflectionProbe.h) | 0 |
| [`Renderer/GL/Renderer_GL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/Renderer_GL.h) | 0 |
| [`Renderer/GL/ShaderGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/ShaderGL.h) | 0 |
| [`Renderer/GL/StorageBufferGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/StorageBufferGL.h) | 0 |
| [`Renderer/GL/SubViewportGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/SubViewportGL.h) | 0 |
| [`Renderer/GL/SurfaceGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/SurfaceGL.h) | 0 |
| [`Renderer/GL/TextureGL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/TextureGL.h) | 0 |
| [`Renderer/GL/glad.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/GL/glad.h) | 0 |
| [`Renderer/MTL/PipelineStateMTL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/MTL/PipelineStateMTL.h) | 0 |
| [`Renderer/MTL/RenderInstancesMTL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/MTL/RenderInstancesMTL.h) | 0 |
| [`Renderer/MTL/RenderPrimitiveMTL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/MTL/RenderPrimitiveMTL.h) | 0 |
| [`Renderer/MTL/RendererMTL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/MTL/RendererMTL.h) | 0 |
| [`Renderer/MTL/ShaderMTL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/MTL/ShaderMTL.h) | 0 |
| [`Renderer/MTL/TextureMTL.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/MTL/TextureMTL.h) | 0 |
| [`Renderer/Managers/DecalManager.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Managers/DecalManager.h) | 0 |
| [`Renderer/Managers/FontManager.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Managers/FontManager.h) | 0 |
| [`Renderer/Managers/LightingManager.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Managers/LightingManager.h) | 0 |
| [`Renderer/Managers/MeshManager3D.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Managers/MeshManager3D.h) | 0 |
| [`Renderer/Managers/ParticleEffectManager.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Managers/ParticleEffectManager.h) | 0 |
| [`Renderer/Managers/SurfaceManager.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Managers/SurfaceManager.h) | 0 |
| [`Renderer/Material2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Material2.h) | 0 |
| [`Renderer/Material3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Material3.h) | 0 |
| [`Renderer/Mesh.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Mesh.h) | 0 |
| [`Renderer/Model.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Model.h) | 0 |
| [`Renderer/PipelineState.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/PipelineState.h) | 0 |
| [`Renderer/Renderer.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Renderer.h) | 0 |
| [`Renderer/Shader.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Shader.h) | 0 |
| [`Renderer/SkyBox.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/SkyBox.h) | 0 |
| [`Renderer/StorageBuffer.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/StorageBuffer.h) | 0 |
| [`Renderer/SubViewport.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/SubViewport.h) | 0 |
| [`Renderer/Surface.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Surface.h) | 0 |
| [`Renderer/Texture.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/Texture.h) | 0 |
| [`Renderer/VK/ConstantBufferVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/ConstantBufferVK.h) | 0 |
| [`Renderer/VK/LightingManagerVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/LightingManagerVK.h) | 0 |
| [`Renderer/VK/MeshVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/MeshVK.h) | 0 |
| [`Renderer/VK/PipelineStateVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/PipelineStateVK.h) | 0 |
| [`Renderer/VK/RenderInstancesVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/RenderInstancesVK.h) | 0 |
| [`Renderer/VK/RenderPrimitiveVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/RenderPrimitiveVK.h) | 0 |
| [`Renderer/VK/RendererVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/RendererVK.h) | 0 |
| [`Renderer/VK/Renderer_VK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/Renderer_VK.h) | 4 |
| [`Renderer/VK/ShaderVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/ShaderVK.h) | 0 |
| [`Renderer/VK/StorageBufferVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/StorageBufferVK.h) | 0 |
| [`Renderer/VK/SubViewportVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/SubViewportVK.h) | 0 |
| [`Renderer/VK/TextureVK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Renderer/VK/TextureVK.h) | 0 |

### Resources (21 headers)

File/resource and serialization headers.

| Header | Export-marked declarations |
|---|---:|
| [`Resources/BezierCurve.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/BezierCurve.h) | 0 |
| [`Resources/Config.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/Config.h) | 0 |
| [`Resources/Gradient.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/Gradient.h) | 0 |
| [`Resources/Json.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/Json.h) | 0 |
| [`Resources/ObjectStream/GetPrimitiveTypeOfType.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/GetPrimitiveTypeOfType.h) | 0 |
| [`Resources/ObjectStream/ObjectStream.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/ObjectStream.h) | 4 |
| [`Resources/ObjectStream/ObjectStreamBinaryIn.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/ObjectStreamBinaryIn.h) | 0 |
| [`Resources/ObjectStream/ObjectStreamBinaryOut.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/ObjectStreamBinaryOut.h) | 0 |
| [`Resources/ObjectStream/ObjectStreamIn.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/ObjectStreamIn.h) | 0 |
| [`Resources/ObjectStream/ObjectStreamOut.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/ObjectStreamOut.h) | 0 |
| [`Resources/ObjectStream/ObjectStreamTextIn.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/ObjectStreamTextIn.h) | 0 |
| [`Resources/ObjectStream/ObjectStreamTextOut.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/ObjectStreamTextOut.h) | 0 |
| [`Resources/ObjectStream/ObjectStreamTypes.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/ObjectStreamTypes.h) | 0 |
| [`Resources/ObjectStream/SerializableAttribute.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/SerializableAttribute.h) | 0 |
| [`Resources/ObjectStream/SerializableAttributeEnum.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/SerializableAttributeEnum.h) | 0 |
| [`Resources/ObjectStream/SerializableAttributeTyped.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/SerializableAttributeTyped.h) | 0 |
| [`Resources/ObjectStream/SerializableObject.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/SerializableObject.h) | 0 |
| [`Resources/ObjectStream/TypeDeclarations.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/ObjectStream/TypeDeclarations.h) | 0 |
| [`Resources/PCK.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/PCK.h) | 0 |
| [`Resources/Timer.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/Timer.h) | 0 |
| [`Resources/Tween.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Resources/Tween.h) | 0 |

### Skeleton (1 headers)

Skeleton and animation declarations.

| Header | Export-marked declarations |
|---|---:|
| [`Skeleton/Skeleton.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Skeleton/Skeleton.h) | 0 |

### TriangleSplitter (1 headers)

Triangle splitting utilities.

| Header | Export-marked declarations |
|---|---:|
| [`TriangleSplitter/TriangleSplitter.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/TriangleSplitter/TriangleSplitter.h) | 0 |

### Variants (53 headers)

Math, geometry and container value types.

| Header | Export-marked declarations |
|---|---:|
| [`Variants/AABB2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/AABB2.h) | 0 |
| [`Variants/AABB3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/AABB3.h) | 0 |
| [`Variants/Basis.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Basis.h) | 0 |
| [`Variants/Color.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Color.h) | 0 |
| [`Variants/Math/EigenValueSymmetric.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Math/EigenValueSymmetric.h) | 0 |
| [`Variants/Math/GaussianElimination.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Math/GaussianElimination.h) | 0 |
| [`Variants/Math/HalfFloat.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Math/HalfFloat.h) | 0 |
| [`Variants/Math/MathTypes.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Math/MathTypes.h) | 0 |
| [`Variants/Math/Real.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Math/Real.h) | 0 |
| [`Variants/Math/Trigonometry.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Math/Trigonometry.h) | 0 |
| [`Variants/Matrix/DMat44.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/DMat44.h) | 0 |
| [`Variants/Matrix/DMat4x3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/DMat4x3.h) | 0 |
| [`Variants/Matrix/DynMatrix.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/DynMatrix.h) | 0 |
| [`Variants/Matrix/Mat22.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat22.h) | 0 |
| [`Variants/Matrix/Mat2x3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat2x3.h) | 0 |
| [`Variants/Matrix/Mat2x4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat2x4.h) | 0 |
| [`Variants/Matrix/Mat33.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat33.h) | 0 |
| [`Variants/Matrix/Mat3x2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat3x2.h) | 0 |
| [`Variants/Matrix/Mat3x4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat3x4.h) | 0 |
| [`Variants/Matrix/Mat44.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat44.h) | 0 |
| [`Variants/Matrix/Mat4x2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat4x2.h) | 0 |
| [`Variants/Matrix/Mat4x3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat4x3.h) | 0 |
| [`Variants/Matrix/TMatrix.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/TMatrix.h) | 0 |
| [`Variants/OBB2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/OBB2.h) | 0 |
| [`Variants/OBB3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/OBB3.h) | 0 |
| [`Variants/Plane.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Plane.h) | 0 |
| [`Variants/Quat.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Quat.h) | 0 |
| [`Variants/Rect.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Rect.h) | 0 |
| [`Variants/TArray.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/TArray.h) | 0 |
| [`Variants/TMap.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/TMap.h) | 0 |
| [`Variants/TSet.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/TSet.h) | 0 |
| [`Variants/TStaticArray.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/TStaticArray.h) | 0 |
| [`Variants/Vector/BVec16.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/BVec16.h) | 0 |
| [`Variants/Vector/DVec2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/DVec2.h) | 0 |
| [`Variants/Vector/DVec3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/DVec3.h) | 0 |
| [`Variants/Vector/DVec4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/DVec4.h) | 0 |
| [`Variants/Vector/Double2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Double2.h) | 0 |
| [`Variants/Vector/Double3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Double3.h) | 0 |
| [`Variants/Vector/Double4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Double4.h) | 0 |
| [`Variants/Vector/Float2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Float2.h) | 0 |
| [`Variants/Vector/Float3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Float3.h) | 0 |
| [`Variants/Vector/Float4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Float4.h) | 0 |
| [`Variants/Vector/Int2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Int2.h) | 0 |
| [`Variants/Vector/Int3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Int3.h) | 0 |
| [`Variants/Vector/Int4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Int4.h) | 0 |
| [`Variants/Vector/TVec.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/TVec.h) | 0 |
| [`Variants/Vector/UVec4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/UVec4.h) | 0 |
| [`Variants/Vector/Vec2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Vec2.h) | 1 |
| [`Variants/Vector/Vec3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Vec3.h) | 1 |
| [`Variants/Vector/Vec4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Vec4.h) | 0 |
| [`Variants/Vector/iVec2.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/iVec2.h) | 1 |
| [`Variants/Vector/iVec3.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/iVec3.h) | 1 |
| [`Variants/Vector/iVec4.h`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/iVec4.h) | 1 |

### Top-level subsystem headers (11 headers)

| Header | Export-marked declarations |
|---|---:|
| [`Moss.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss.h)` | 0 |
| [`Moss_Audio.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_Audio.h)` | 113 |
| [`Moss_GPU.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_GPU.h)` | 241 |
| [`Moss_GUI.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_GUI.h)` | 73 |
| [`Moss_Navigation.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_Navigation.h)` | 0 |
| [`Moss_Network.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_Network.h)` | 146 |
| [`Moss_Physics.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_Physics.h)` | 39 |
| [`Moss_Platform.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_Platform.h)` | 181 |
| [`Moss_Renderer.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_Renderer.h)` | 195 |
| [`Moss_XR.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_XR.h)` | 41 |
| [`Moss_stdinc.h](https://github.com/TxbiG/Moss/blob/main/include/Moss/Moss_stdinc.h)` | 34 |

## Inline implementation header inventory (19 files)

These files provide inline/template definitions or companion implementation details. They are included here because the public API surface cannot be understood from declarations alone when these files contain required definitions.

| Inline header |
|---|
| [`Core/FixedSizeFreeList.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/FixedSizeFreeList.inl) |
| [`Core/JobSystem.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/JobSystem.inl) |
| [`Core/LockFreeHashMap.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/LockFreeHashMap.inl) |
| [`Core/Profiler.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Core/Profiler.inl) |
| [`Physics/Body/Body.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Physics/Body/Body.inl) |
| [`Variants/Matrix/DMat44.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/DMat44.inl) |
| [`Variants/Matrix/Mat44.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Matrix/Mat44.inl) |
| [`Variants/Quat.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Quat.inl) |
| [`Variants/Vector/BVec16.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/BVec16.inl) |
| [`Variants/Vector/DVec2.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/DVec2.inl) |
| [`Variants/Vector/DVec3.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/DVec3.inl) |
| [`Variants/Vector/DVec4.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/DVec4.inl) |
| [`Variants/Vector/UVec4.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/UVec4.inl) |
| [`Variants/Vector/Vec2.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Vec2.inl) |
| [`Variants/Vector/Vec3.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Vec3.inl) |
| [`Variants/Vector/Vec4.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/Vec4.inl) |
| [`Variants/Vector/iVec2.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/iVec2.inl) |
| [`Variants/Vector/iVec3.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/iVec3.inl) |
| [`Variants/Vector/iVec4.inl`](https://github.com/TxbiG/Moss/blob/main/include/Moss/Variants/Vector/iVec4.inl) |

## 9. Native integration details to preserve

This is a language-level API specification, not a mechanical ABI mirror. When implementing the projection, keep these source-level distinctions intact:

- Convert pointer-plus-count inputs into a single slice/span/list/array/table/sequence; do not expose a redundant count parameter when the collection already carries its length.
- Convert output pointer pairs (for example, mouse coordinates) into a point/tuple/value object. Do not expose C-style `int*` output arguments.
- Resource factories return owned language objects and translate native allocation/backend failure into the language's normal error model. Destroy/terminate functions become deterministic resource disposal (`Dispose`, `Drop`, `close`, or context-manager release), not ordinary public `destroy_*` calls.
- GUI calls should be scoped to a `GuiContext` instance in every managed binding. They must not silently depend on mutable global “current context” state when the public API can retain an explicit context.
- Vulkan handles, callback pointers, allocator callbacks and backend-specific opaque handles stay behind advanced interop APIs unless the user explicitly opts into native interop. They must not be confused with Moss-owned managed resources.
- Where the native declaration lacks information required by the intended safe API (for example a pointer/count pair, output values, or a resource owner), resolve the native declaration or binding-side contract before implementing that projection; do not guess buffer length or object lifetime.

## 10. Build and distribution contract

- Ship matching native binaries and binding artifacts for each supported OS/architecture combination.
- Keep ABI declarations internal; public API users should not need to know calling conventions, struct packing, pointer widths or native library search paths.
- Make backend and optional dependency feature flags visible through capability queries or build metadata.
- Test a clean build, import/load, initialization/shutdown, object lifetime, callback detach, error translation and resource ownership in CI for every supported platform.
- Keep the generated language API index in sync with the native CMoss export catalogue whenever public native declarations change.

## 11. Compatibility and versioning

Treat the binding's public API as a separately versioned language library. A native API addition does not become a public binding API until the type mapping, ownership, failure semantics and example are specified. Avoid leaking raw `Moss_*` function names into application code. Deprecate public methods using the target language's normal annotation mechanism and provide a migration note in the API Reference itself when a breaking change is unavoidable.

---

**Canonical authority:** Moss C++/CMoss headers and the CMoss API Reference supplied for this documentation set. The native symbol column preserves traceability; the public API column is the intended language-specific design.
