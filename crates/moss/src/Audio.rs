//! Audio playback, effects, listeners, and ray-traced spatial audio.
//!
//! Safe wrapper around `Moss_Audio.h`. Enabled by the `audio` feature.
//!
//! ## Assumptions this relies on
//! Unlike `Moss_Platform.h`, this header declares plain (non-`extern "C"`)
//! C++ functions, including **overloads** (two different
//! `Moss_AudioCreateEffect` signatures) and a C++ reference parameter
//! (`Moss_AudioListenerSetOrientation`'s `const Vec3&` args). That only
//! binds correctly if `moss-sys`'s `build.rs` compiles this header as C++
//! and lets bindgen emit the mangled symbol names (`.clang_arg("-x")`,
//! `.clang_arg("c++")`, `.clang_arg("-std=c++17")`, and bindgen resolving
//! the mangled `#[link_name = "..."]` per function) — the same thing
//! `engine.rs`'s `moss_engine_*` calls already imply is set up, though
//! those are plain `extern "C"` names so they don't exercise the mangled
//! path. If overload resolution doesn't come through correctly, the two
//! `Moss_AudioCreateEffect` symbols in particular are worth checking first.
//!
//! Reference parameters (`const Vec3&`) are assumed to lower to `*const
//! Vec3` in the generated bindings, per the Itanium C++ ABI's normal
//! reference-as-pointer representation.
//!
//! This also assumes `Moss_MicrophoneSetCallback` accepts a `NULL`
//! callback to unregister (needed for [`Microphone::set_callback`] /
//! `Drop` to free the boxed closure safely without risking the native side
//! calling back into freed memory) — the header doesn't document that
//! either way, so it's worth confirming.
//!
//! ## Real bugs found in this header (not fixed silently — see inline notes)
//! 1. **`Moss_AudioCreateEffect` is overloaded across two unrelated
//!    purposes**: `AudioEffect Moss_AudioCreateEffect(AudioEffectType)`
//!    creates an effect, while `void Moss_AudioCreateEffect(AudioEffect*,
//!    const char*, float)` — despite the identical name — *sets a
//!    parameter* on an existing effect (its own doc comment says so). The
//!    second one should be named something like `Moss_AudioSetEffectParam`.
//!    Bound below as [`effects::Effect::set_param`] regardless of the
//!    confusing native name.
//! 2. **`AudioEffect` is only forward-declared** (`struct AudioEffect;`)
//!    in this header, yet `Moss_AudioCreateEffect(AudioEffectType)`
//!    returns it *by value*. Returning an incomplete C++ type by value
//!    isn't valid C++, and even if it somehow builds, a by-value non-POD
//!    C++ return can't be bound correctly over FFI without knowing its
//!    real layout/constructors. **Effect creation is left unbound** — see
//!    [`effects`] — pending a fix upstream (e.g. returning `AudioEffect*`
//!    instead). The pointer-based effect functions (`AddChannelEffect`
//!    etc.) are bound as opaque-pointer operations, since those don't
//!    depend on constructing an `AudioEffect` value in Rust.
//! 3. **Three stray `Moss_AudioStream3D*` declarations take
//!    `AudioStream2D*`**, not `AudioStream3D*`:
//!    ```text
//!    Moss_AudioStream3DSetPosition(AudioStream2D*)
//!    Moss_AudioStream3DSetVelocity(AudioStream2D*)
//!    Moss_AudioStream3DSetMaxDistance(AudioStream2D*)
//!    ```
//!    immediately followed, later in the same header, by correctly-typed
//!    ones taking `AudioStream3D*`. Legal C++ overloading, but clearly a
//!    copy-paste mistake (a "3D" function operating on a 2D stream). Only
//!    the `AudioStream3D*`-typed versions are bound.
//! 4. **Every `Set{Position,Velocity,MaxDistance}` function — 2D and 3D
//!    alike — takes no value parameter**, e.g.
//!    `Moss_AudioStream3DSetPosition(AudioStream3D* audiostream)` with no
//!    position argument. Bound faithfully (an arity mismatch would fail to
//!    link), but as written these can't do anything useful. Needs a
//!    `Vec3 position` / `Vec3 velocity` / `float max_distance` parameter
//!    added upstream.
//! 5. **`Audio_RemoveChannel` is missing the `Moss_` prefix** that every
//!    other function in this header has. Bound as [`Channel::remove`]
//!    regardless.
//! 6. **`Float2`/`Float3`** (used only by the four
//!    `Moss_Audio*Create*Listener*` functions) are never defined or
//!    `#include`d in this header, unlike `Vec2`/`Vec3` which are pulled in
//!    explicitly from `Moss/Variants/Vector/`. This looks like it should
//!    say `Vec2`/`Vec3` too. Bound below assuming `Float2`/`Float3` have
//!    the same `{x, y[, z]}: f32` layout as `Vec2`/`Vec3` — worth
//!    confirming against wherever `Float2`/`Float3` actually live
//!    (`Moss_stdinc.h`?).
//! 7. **Two overlapping microphone APIs** are declared side by side: a
//!    struct-based one (`Moss_MicrophoneOpen(desc) -> Moss_Microphone*`,
//!    `Read`/`SetGain`/`SetCallback`/`GetLevels`/...) and a simpler global
//!    one (`Moss_AudioMicrophoneOpen()`/`Play`/`Stop`/`ID`/...) that seems
//!    to duplicate the same functionality without a device handle. Only
//!    the struct-based [`Microphone`] API is wrapped here, since binding
//!    both would present two competing ways to do the same thing; the
//!    global functions are left out.
//! 8. `Moss_AudioLoadWav()` (zero args) is documented as always returning
//!    null — bound as [`AudioSource::load_wav_noop`] with that behavior
//!    called out, rather than omitted, since it's a real linkable symbol.
//! 9. `Moss_AudioDecodeCallback` / `Moss_AudioRegisterDecoder` are left
//!    out: the only function that would use them is commented out in the
//!    header, so there's currently no way to actually register one.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_void};
use std::ptr::NonNull;

use moss_sys as sys;

use crate::error::{MossError, Result};
use crate::math::{Vec2, Vec3};

// ===========================================================================
// Helpers (mirrors platform.rs)
// ===========================================================================

unsafe fn owned_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
    }
}

fn to_cstring(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| MossError::InvalidArgument)
}

/// Field-by-field copy into the native `Vec2` layout — not a pointer cast,
/// so this doesn't depend on `crate::math::Vec2` and `sys::Vec2` being the
/// exact same Rust type, only the same fields.
fn sys_vec2(v: Vec2) -> sys::Vec2 {
    sys::Vec2 { x: v.x, y: v.y }
}

fn sys_vec3(v: Vec3) -> sys::Vec3 {
    sys::Vec3 { x: v.x, y: v.y, z: v.z }
}

/// See header note 6: assumes `Float2`/`Float3` share `Vec2`/`Vec3`'s layout.
fn sys_float2(v: Vec2) -> sys::Float2 {
    sys::Float2 { x: v.x, y: v.y }
}

fn sys_float3(v: Vec3) -> sys::Float3 {
    sys::Float3 { x: v.x, y: v.y, z: v.z }
}

// ===========================================================================
// IDs
// ===========================================================================

pub type ChannelId = sys::ChannelID;
pub const CHANNEL_INVALID: ChannelId = 0;

// ===========================================================================
// Enums
// ===========================================================================

/// DSP effect type(s). Mirrors `AudioEffectType`. Unlike `Moss_WindowFlags`
/// in `platform.rs`, these values are genuine non-colliding powers of two
/// (aside from `LOWPASS = 0`, the conventional "no extra flags" base
/// value), so combining them with `|` is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioEffectType(u32);

impl AudioEffectType {
    pub const LOWPASS: Self = Self(0);
    pub const HIGHPASS: Self = Self(1 << 0);
    pub const ECHO: Self = Self(1 << 1);
    pub const FLANGE: Self = Self(1 << 2);
    pub const DISTORTION: Self = Self(1 << 3);
    pub const NORMALIZE: Self = Self(1 << 4);
    pub const PARAMEQ: Self = Self(1 << 5);
    pub const PITCHSHIFTER: Self = Self(1 << 6);
    pub const CHORUS: Self = Self(1 << 7);
    pub const COMPRESSOR: Self = Self(1 << 8);
    pub const REVERB: Self = Self(1 << 9);
    pub const DELAY: Self = Self(1 << 10);
    pub const DOPPLER: Self = Self(1 << 11);
    pub const PANNING: Self = Self(1 << 12);
    pub const DISTANCE_ATTENUATION: Self = Self(1 << 13);
}

impl std::ops::BitOr for AudioEffectType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl From<AudioEffectType> for sys::AudioEffectType {
    fn from(value: AudioEffectType) -> Self {
        // SAFETY: same repr/discriminant space as the native enum.
        unsafe { std::mem::transmute(value.0 as i32) }
    }
}

/// Mirrors `DistanceModel`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DistanceModel {
    Linear,
    Inverse,
    Exponential,
}

/// Mirrors `AudioLoadType`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioLoadType {
    FullyLoaded,
    Streaming,
}

impl From<DistanceModel> for sys::DistanceModel {
    fn from(value: DistanceModel) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl From<AudioLoadType> for sys::AudioLoadType {
    fn from(value: AudioLoadType) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

// ===========================================================================
// Plain data types
// ===========================================================================

/// Mirrors `Moss_MicrophoneDesc`. `Default` matches the native default
/// member initializers.
#[derive(Debug, Clone, Copy)]
pub struct MicrophoneDesc {
    pub device_index: u32,
    pub sample_rate: u32,
    pub channels: u32,
    pub buffer_frames: u32,
    pub ring_buffer_frames: u32,
    pub start_immediately: bool,
    pub enable_voice_metrics: bool,
}

impl Default for MicrophoneDesc {
    fn default() -> Self {
        Self {
            device_index: 0,
            sample_rate: 48_000,
            channels: 1,
            buffer_frames: 480,
            ring_buffer_frames: 48_000,
            start_immediately: true,
            enable_voice_metrics: true,
        }
    }
}

/// Mirrors `Moss_MicrophoneLevels`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MicrophoneLevels {
    pub rms: f32,
    pub peak: f32,
    pub smoothed_volume: f32,
    pub voice_activity: f32,
}

/// Mirrors `Moss_AudioRayHit2D`.
#[derive(Debug, Clone, Copy)]
pub struct AudioRayHit2D {
    pub hit: bool,
    pub fraction: f32,
    pub position: Vec2,
    pub normal: Vec2,
    pub absorption: f32,
    pub transmission: f32,
    pub material_id: u32,
}

impl Default for AudioRayHit2D {
    fn default() -> Self {
        Self {
            hit: false,
            fraction: 1.0,
            position: Vec2::ZERO,
            normal: Vec2::ZERO,
            absorption: 0.5,
            transmission: 0.25,
            material_id: 0,
        }
    }
}

/// Mirrors `Moss_AudioRayHit3D`.
#[derive(Debug, Clone, Copy)]
pub struct AudioRayHit3D {
    pub hit: bool,
    pub fraction: f32,
    pub position: Vec3,
    pub normal: Vec3,
    pub absorption: f32,
    pub transmission: f32,
    pub material_id: u32,
}

impl Default for AudioRayHit3D {
    fn default() -> Self {
        Self {
            hit: false,
            fraction: 1.0,
            position: Vec3::ZERO,
            normal: Vec3::ZERO,
            absorption: 0.5,
            transmission: 0.25,
            material_id: 0,
        }
    }
}

/// Mirrors `Moss_AudioRayTraceResult`.
#[derive(Debug, Clone, Copy)]
pub struct AudioRayTraceResult {
    pub audible: bool,
    pub occluded: bool,
    pub distance: f32,
    pub attenuation: f32,
    pub occlusion: f32,
    pub transmission_gain: f32,
    pub lowpass: f32,
    pub reflection_gain: f32,
    pub reflection_delay_seconds: f32,
    pub delay_seconds: f32,
}

impl Default for AudioRayTraceResult {
    fn default() -> Self {
        Self {
            audible: false,
            occluded: false,
            distance: 0.0,
            attenuation: 1.0,
            occlusion: 0.0,
            transmission_gain: 1.0,
            lowpass: 1.0,
            reflection_gain: 0.0,
            reflection_delay_seconds: 0.0,
            delay_seconds: 0.0,
        }
    }
}

impl AudioRayTraceResult {
    /// Computed listener gain for this trace result
    /// (`Moss_AudioRayTraceComputeGain`).
    pub fn compute_gain(&self) -> f32 {
        let raw: sys::Moss_AudioRayTraceResult = self.into();
        unsafe { sys::Moss_AudioRayTraceComputeGain(&raw) }
    }
}

impl From<&AudioRayTraceResult> for sys::Moss_AudioRayTraceResult {
    fn from(r: &AudioRayTraceResult) -> Self {
        sys::Moss_AudioRayTraceResult {
            audible: r.audible,
            occluded: r.occluded,
            distance: r.distance,
            attenuation: r.attenuation,
            occlusion: r.occlusion,
            transmission_gain: r.transmission_gain,
            lowpass: r.lowpass,
            reflection_gain: r.reflection_gain,
            reflection_delay_seconds: r.reflection_delay_seconds,
            delay_seconds: r.delay_seconds,
        }
    }
}

impl From<sys::Moss_AudioRayTraceResult> for AudioRayTraceResult {
    fn from(r: sys::Moss_AudioRayTraceResult) -> Self {
        Self {
            audible: r.audible,
            occluded: r.occluded,
            distance: r.distance,
            attenuation: r.attenuation,
            occlusion: r.occlusion,
            transmission_gain: r.transmission_gain,
            lowpass: r.lowpass,
            reflection_gain: r.reflection_gain,
            reflection_delay_seconds: r.reflection_delay_seconds,
            delay_seconds: r.delay_seconds,
        }
    }
}

/// An opaque handle to the physics world, passed straight through to the
/// audio ray-tracing callbacks without being touched here.
///
/// `PhysicsSystem` is only ever forward-declared across the headers seen so
/// far (this one and `Moss_Physics.h`'s stub) — once `physics.rs` grows a
/// real wrapper, that should become the source of this handle instead of
/// callers constructing it themselves.
pub struct PhysicsHandle(NonNull<sys::PhysicsSystem>);

impl PhysicsHandle {
    /// # Safety
    /// `raw` must be a valid `PhysicsSystem*` for as long as this handle
    /// (and anything it's passed to) is in use.
    pub unsafe fn from_raw(raw: *mut sys::PhysicsSystem) -> Option<Self> {
        NonNull::new(raw).map(Self)
    }

    pub fn as_raw(&self) -> *mut sys::PhysicsSystem {
        self.0.as_ptr()
    }
}

// ===========================================================================
// System init / update
// ===========================================================================

pub fn init() -> Result<()> {
    let code = unsafe { sys::Moss_Init_Audio() };
    if code == 0 {
        Ok(())
    } else {
        Err(MossError::Native(code))
    }
}

pub fn terminate() {
    unsafe { sys::Moss_Terminate_Audio() }
}

/// Call once per frame.
pub fn update(delta_time: f32) {
    unsafe { sys::Moss_AudioUpdate(delta_time) }
}

// ===========================================================================
// Audio sources
// ===========================================================================

/// A loaded (or streaming) audio source. Destroyed on drop.
pub struct AudioSource {
    raw: NonNull<sys::Moss_AudioSource>,
}

impl AudioSource {
    pub fn load_wav(path: &str) -> Result<Self> {
        let c_path = to_cstring(path)?;
        let raw = unsafe { sys::Moss_AudioLoadWavFile(c_path.as_ptr()) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::InvalidArgument)
    }

    /// Bound for completeness only — see header note 8. This always
    /// returns `Err`, matching the native function's documented behavior
    /// of always returning null.
    pub fn load_wav_noop() -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioLoadWav() };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::InvalidArgument)
    }

    pub fn load_ogg(path: &str, load_type: AudioLoadType) -> Result<Self> {
        let c_path = to_cstring(path)?;
        let raw = unsafe { sys::Moss_AudioLoadOgg(c_path.as_ptr(), load_type.into()) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::InvalidArgument)
    }

    pub fn load_mp3(path: &str) -> Result<Self> {
        let c_path = to_cstring(path)?;
        let raw = unsafe { sys::Moss_AudioLoadMP3(c_path.as_ptr()) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::InvalidArgument)
    }

    /// Creates a source that streams live microphone input.
    pub fn from_microphone(mic: &Microphone) -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioCaptureMicrophone(mic.raw.as_ptr()) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::InvalidArgument)
    }
}

impl Drop for AudioSource {
    fn drop(&mut self) {
        unsafe { sys::Moss_AudioSourceDestroy(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Effects — see header note 2. Creation is blocked; pointer ops are bound.
// ===========================================================================

pub mod effects {
    //! Effects can't currently be *created* from Rust — see header note 2
    //! at the top of the parent module for why. The pointer-taking
    //! operations below (attaching/detaching an already-existing native
    //! effect to a channel) are bound regardless, since a future fix
    //! upstream (e.g. `Moss_AudioCreateEffect` returning `AudioEffect*`)
    //! would only need a constructor added here, not a change to these.
    use super::*;

    /// An already-existing native effect, referenced by raw pointer.
    ///
    /// # Safety
    /// There is currently no safe way to obtain one of these from Rust —
    /// see the module-level note. Any `Effect` must come from a pointer
    /// the native engine handed you some other way.
    pub struct Effect(pub NonNull<sys::AudioEffect>);

    impl Effect {
        /// Sets a named parameter on this effect. Bound to the native
        /// `Moss_AudioCreateEffect(AudioEffect*, const char*, float)`
        /// overload — see header note 1 for why that name is misleading.
        pub fn set_param(&self, name: &str, value: f32) -> Result<()> {
            let c_name = to_cstring(name)?;
            unsafe { sys::Moss_AudioCreateEffect(self.0.as_ptr(), c_name.as_ptr(), value) };
            Ok(())
        }

        pub fn remove(self) {
            unsafe { sys::Moss_AudioRemoveEffect(self.0.as_ptr()) }
        }
    }
}

// ===========================================================================
// Channels
// ===========================================================================

/// A mixer channel. Channels are referenced by ID rather than an owning
/// Rust value — the native API has no visible "destroy" beyond
/// [`Channel::remove`], and `ChannelID` is `Copy`, so there's little value
/// in wrapping it as an owning type the way `AudioSource` is.
pub struct Channel(pub ChannelId);

impl Channel {
    /// Creates (or retrieves — see the native doc comment, which is
    /// ambiguous about which) a channel for the given ID.
    pub fn create(id: ChannelId) -> Self {
        Self(unsafe { sys::Moss_AudioCreateChannel(id) })
    }

    pub fn master() -> Self {
        Self(unsafe { sys::Moss_AudioGetMasterChannel() })
    }

    /// Bound to `Audio_RemoveChannel` — see header note 5 about its
    /// missing `Moss_` prefix.
    pub fn remove(self) {
        unsafe { sys::Audio_RemoveChannel(self.0) }
    }

    pub fn set_volume(&self, volume: f32) {
        unsafe { sys::Moss_AudioSetChannelVolume(self.0, volume) }
    }

    pub fn set_mute(&self, mute: bool) {
        unsafe { sys::Moss_AudioSetChannelMute(self.0, mute) }
    }

    pub fn add_effect(&self, effect: &effects::Effect) {
        unsafe { sys::Moss_AudioAddChannelEffect(self.0, effect.0.as_ptr()) }
    }

    pub fn remove_effect(&self, effect: &effects::Effect) {
        unsafe { sys::Moss_AudioRemoveChannelEffect(self.0, effect.0.as_ptr()) }
    }

    pub fn remove_all_effects(&self) {
        unsafe { sys::Moss_AudioRemoveAllChannelEffects(self.0) }
    }
}

// ===========================================================================
// Streams — non-spatial, 2D, and 3D
// ===========================================================================

/// A non-spatial audio stream.
pub struct AudioStream {
    raw: NonNull<sys::AudioStream>,
}

impl AudioStream {
    pub fn new() -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioStreamCreate() };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    pub fn play(&self) {
        unsafe { sys::Moss_AudioStreamPlay(self.raw.as_ptr()) }
    }

    pub fn stop(&self) {
        unsafe { sys::Moss_AudioStreamStop(self.raw.as_ptr()) }
    }

    pub fn set_volume(&self, volume: f32) {
        unsafe { sys::Moss_AudioStreamSetVolume(self.raw.as_ptr(), volume) }
    }

    pub fn set_pitch(&self, pitch: f32) {
        unsafe { sys::Moss_AudioStreamSetPitch(self.raw.as_ptr(), pitch) }
    }

    pub fn set_playback_rate(&self, rate: f32) {
        unsafe { sys::Moss_AudioStreamSetPlaybackRate(self.raw.as_ptr(), rate) }
    }

    pub fn set_pan(&self, pan: f32) {
        unsafe { sys::Moss_AudioStreamSetPan(self.raw.as_ptr(), pan) }
    }

    pub fn set_looping(&self, looping: bool) {
        unsafe { sys::Moss_AudioStreamSetLoop(self.raw.as_ptr(), looping) }
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        unsafe { sys::Moss_AudioStreamRemove(self.raw.as_ptr()) }
    }
}

/// A 2D (panned, non-distance-attenuated) spatial audio stream.
pub struct AudioStream2D {
    raw: NonNull<sys::AudioStream2D>,
}

impl AudioStream2D {
    pub fn new() -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioStream2DCreate() };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    pub fn play(&self) {
        unsafe { sys::Moss_AudioStream2DPlay(self.raw.as_ptr()) }
    }

    pub fn stop(&self) {
        unsafe { sys::Moss_AudioStream2DStop(self.raw.as_ptr()) }
    }

    pub fn set_volume(&self, volume: f32) {
        unsafe { sys::Moss_AudioStream2DSetVolume(self.raw.as_ptr(), volume) }
    }

    pub fn set_pitch(&self, pitch: f32) {
        unsafe { sys::Moss_AudioStream2DSetPitch(self.raw.as_ptr(), pitch) }
    }

    pub fn set_playback_rate(&self, rate: f32) {
        unsafe { sys::Moss_AudioStream2DSetPlaybackRate(self.raw.as_ptr(), rate) }
    }

    pub fn set_pan(&self, pan: f32) {
        unsafe { sys::Moss_AudioStream2DSetPan(self.raw.as_ptr(), pan) }
    }

    pub fn set_looping(&self, looping: bool) {
        unsafe { sys::Moss_AudioStream2DSetLoop(self.raw.as_ptr(), looping) }
    }

    // `Moss_AudioStream3DSet{Position,Velocity,MaxDistance}(AudioStream2D*)`
    // are deliberately NOT bound here — see header note 3. They're a
    // misnamed, likely-accidental overload; the correctly-typed 3D versions
    // live on `AudioStream3D` below.
}

impl Drop for AudioStream2D {
    fn drop(&mut self) {
        unsafe { sys::Moss_AudioStream2DRemove(self.raw.as_ptr()) }
    }
}

/// A 3D (positional, distance-attenuated) spatial audio stream.
pub struct AudioStream3D {
    raw: NonNull<sys::AudioStream3D>,
}

impl AudioStream3D {
    pub fn new() -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioStream3DCreate() };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    pub fn play(&self) {
        unsafe { sys::Moss_AudioStream3DPlay(self.raw.as_ptr()) }
    }

    pub fn stop(&self) {
        unsafe { sys::Moss_AudioStream3DStop(self.raw.as_ptr()) }
    }

    pub fn set_volume(&self, volume: f32) {
        unsafe { sys::Moss_AudioStream3DSetVolume(self.raw.as_ptr(), volume) }
    }

    pub fn set_pitch(&self, pitch: f32) {
        unsafe { sys::Moss_AudioStream3DSetPitch(self.raw.as_ptr(), pitch) }
    }

    pub fn set_playback_rate(&self, rate: f32) {
        unsafe { sys::Moss_AudioStream3DSetPlaybackRate(self.raw.as_ptr(), rate) }
    }

    pub fn set_pan(&self, pan: f32) {
        unsafe { sys::Moss_AudioStream3DSetPan(self.raw.as_ptr(), pan) }
    }

    pub fn set_looping(&self, looping: bool) {
        unsafe { sys::Moss_AudioStream3DSetLoop(self.raw.as_ptr(), looping) }
    }

    pub fn set_distance_model(&self, model: DistanceModel) {
        unsafe { sys::Moss_AudioStream3DSetDistanceModel(self.raw.as_ptr(), model.into()) }
    }

    /// See header note 4: the native function this calls takes no position
    /// argument, so calling this currently has no observable effect. Bound
    /// for symbol completeness; expect this to need a `position: Vec3`
    /// parameter once the header is fixed.
    pub fn set_position_noop(&self) {
        unsafe { sys::Moss_AudioStream3DSetPosition(self.raw.as_ptr()) }
    }

    /// See [`Self::set_position_noop`] — same caveat, for velocity.
    pub fn set_velocity_noop(&self) {
        unsafe { sys::Moss_AudioStream3DSetVelocity(self.raw.as_ptr()) }
    }

    /// See [`Self::set_position_noop`] — same caveat, for max distance.
    pub fn set_max_distance_noop(&self) {
        unsafe { sys::Moss_AudioStream3DSetMaxDistance(self.raw.as_ptr()) }
    }
}

impl Drop for AudioStream3D {
    fn drop(&mut self) {
        unsafe { sys::Moss_AudioStream3DRemove(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Listeners
// ===========================================================================

pub struct AudioListener2D {
    raw: NonNull<sys::AudioListener2D>,
}

impl AudioListener2D {
    /// See header note 6 about the `Float2` vs `Vec2` naming mismatch this
    /// assumes.
    pub fn new(position: Vec2) -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioCreateAudioListener2D(sys_float2(position)) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    pub fn set_active(&self, active: bool) {
        unsafe { sys::Moss_AudioActivateAudioListener2D(self.raw.as_ptr(), active) }
    }
}

impl Drop for AudioListener2D {
    fn drop(&mut self) {
        unsafe { sys::Moss_AudioRemoveAudioListener2D(self.raw.as_ptr()) }
    }
}

pub struct AudioListener3D {
    raw: NonNull<sys::AudioListener3D>,
}

impl AudioListener3D {
    pub fn new(position: Vec3) -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioCreateAudioListener3D(sys_float3(position)) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    pub fn set_active(&self, active: bool) {
        unsafe { sys::Moss_AudioActivateAudioListener3D(self.raw.as_ptr(), active) }
    }

    /// Sets listener orientation. The native parameters are C++
    /// references (`const Vec3&`); see the module-level ABI note about
    /// the `*const Vec3` assumption this relies on.
    pub fn set_orientation(&self, forward: Vec3, up: Vec3) {
        let forward = sys_vec3(forward);
        let up = sys_vec3(up);
        unsafe { sys::Moss_AudioListenerSetOrientation(self.raw.as_ptr(), &forward, &up) }
    }
}

impl Drop for AudioListener3D {
    fn drop(&mut self) {
        unsafe { sys::Moss_AudioRemoveAudioListener3D(self.raw.as_ptr()) }
    }
}

/// A callback that traces one audio ray against the physics world.
pub type RaycastFn2D = Box<dyn Fn(&PhysicsHandle, Vec2, Vec2, f32) -> Option<AudioRayHit2D> + 'static>;
pub type RaycastFn3D = Box<dyn Fn(&PhysicsHandle, Vec3, Vec3, f32) -> Option<AudioRayHit3D> + 'static>;

unsafe extern "C" fn raycast_2d_trampoline(
    physics: *mut sys::PhysicsSystem,
    origin: *const sys::Vec2,
    direction: *const sys::Vec2,
    distance: c_float,
    out_hit: *mut sys::Moss_AudioRayHit2D,
    user_data: *mut c_void,
) -> bool {
    let Some(handle) = PhysicsHandle::from_raw(physics) else {
        return false;
    };
    let callback = &*(user_data as *const RaycastFn2D);
    let origin = Vec2::new((*origin).x, (*origin).y);
    let direction = Vec2::new((*direction).x, (*direction).y);
    match callback(&handle, origin, direction, distance) {
        Some(hit) => {
            *out_hit = sys::Moss_AudioRayHit2D {
                hit: hit.hit,
                fraction: hit.fraction,
                position: sys_vec2(hit.position),
                normal: sys_vec2(hit.normal),
                absorption: hit.absorption,
                transmission: hit.transmission,
                material_id: hit.material_id,
            };
            true
        }
        None => false,
    }
}

unsafe extern "C" fn raycast_3d_trampoline(
    physics: *mut sys::PhysicsSystem,
    origin: *const sys::Vec3,
    direction: *const sys::Vec3,
    distance: c_float,
    out_hit: *mut sys::Moss_AudioRayHit3D,
    user_data: *mut c_void,
) -> bool {
    let Some(handle) = PhysicsHandle::from_raw(physics) else {
        return false;
    };
    let callback = &*(user_data as *const RaycastFn3D);
    let origin = Vec3::new((*origin).x, (*origin).y, (*origin).z);
    let direction = Vec3::new((*direction).x, (*direction).y, (*direction).z);
    match callback(&handle, origin, direction, distance) {
        Some(hit) => {
            *out_hit = sys::Moss_AudioRayHit3D {
                hit: hit.hit,
                fraction: hit.fraction,
                position: sys_vec3(hit.position),
                normal: sys_vec3(hit.normal),
                absorption: hit.absorption,
                transmission: hit.transmission,
                material_id: hit.material_id,
            };
            true
        }
        None => false,
    }
}

pub struct RayAudioListener2D {
    raw: NonNull<sys::RayAudioListener2D>,
    // Kept alive for as long as the listener holds a pointer to it.
    _raycast: Option<Box<RaycastFn2D>>,
}

impl RayAudioListener2D {
    pub fn new(position: Vec2) -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioCreateRayAudioListener2D(sys_float2(position)) };
        NonNull::new(raw)
            .map(|raw| Self { raw, _raycast: None })
            .ok_or(MossError::OutOfMemory)
    }

    pub fn set_active(&self, active: bool) {
        unsafe { sys::Moss_AudioActivateRayAudioListener2D(self.raw.as_ptr(), active) }
    }

    pub fn set_physics(&mut self, physics: &PhysicsHandle, raycast: RaycastFn2D) {
        let boxed = Box::new(raycast);
        let user_data = boxed.as_ref() as *const RaycastFn2D as *mut c_void;
        unsafe {
            sys::Moss_AudioRayListener2DSetPhysics(
                self.raw.as_ptr(),
                physics.as_raw(),
                Some(raycast_2d_trampoline),
                user_data,
            );
        }
        self._raycast = Some(boxed);
    }

    pub fn set_trace_settings(&self, max_distance: f32, reflection_rays: u32) {
        unsafe {
            sys::Moss_AudioRayListener2DSetTraceSettings(self.raw.as_ptr(), max_distance, reflection_rays)
        }
    }

    /// Traces from this listener toward `source_position`.
    pub fn trace_from(&self, source_position: Vec2) -> Option<AudioRayTraceResult> {
        let source = sys_vec2(source_position);
        let mut out = sys::Moss_AudioRayTraceResult::default();
        let ok = unsafe { sys::Moss_AudioRayTraceFromListener2D(self.raw.as_ptr(), &source, &mut out) };
        ok.then(|| out.into())
    }
}

impl Drop for RayAudioListener2D {
    fn drop(&mut self) {
        unsafe { sys::Moss_AudioRemoveRayAudioListener2D(self.raw.as_ptr()) }
    }
}

pub struct RayAudioListener3D {
    raw: NonNull<sys::RayAudioListener3D>,
    _raycast: Option<Box<RaycastFn3D>>,
}

impl RayAudioListener3D {
    pub fn new(position: Vec3) -> Result<Self> {
        let raw = unsafe { sys::Moss_AudioCreateRayAudioListener3D(sys_float3(position)) };
        NonNull::new(raw)
            .map(|raw| Self { raw, _raycast: None })
            .ok_or(MossError::OutOfMemory)
    }

    pub fn set_active(&self, active: bool) {
        unsafe { sys::Moss_AudioActivateRayAudioListener3D(self.raw.as_ptr(), active) }
    }

    pub fn set_physics(&mut self, physics: &PhysicsHandle, raycast: RaycastFn3D) {
        let boxed = Box::new(raycast);
        let user_data = boxed.as_ref() as *const RaycastFn3D as *mut c_void;
        unsafe {
            sys::Moss_AudioRayListener3DSetPhysics(
                self.raw.as_ptr(),
                physics.as_raw(),
                Some(raycast_3d_trampoline),
                user_data,
            );
        }
        self._raycast = Some(boxed);
    }

    pub fn set_trace_settings(&self, max_distance: f32, reflection_rays: u32) {
        unsafe {
            sys::Moss_AudioRayListener3DSetTraceSettings(self.raw.as_ptr(), max_distance, reflection_rays)
        }
    }

    pub fn trace_from(&self, source_position: Vec3) -> Option<AudioRayTraceResult> {
        let source = sys_vec3(source_position);
        let mut out = sys::Moss_AudioRayTraceResult::default();
        let ok = unsafe { sys::Moss_AudioRayTraceFromListener3D(self.raw.as_ptr(), &source, &mut out) };
        ok.then(|| out.into())
    }
}

impl Drop for RayAudioListener3D {
    fn drop(&mut self) {
        unsafe { sys::Moss_AudioRemoveRayAudioListener3D(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// One-shot ray tracing (not tied to a listener)
// ===========================================================================

/// One-off 2D trace, given an explicit listener/source position pair
/// rather than a [`RayAudioListener2D`].
pub fn ray_trace_2d(
    physics: &PhysicsHandle,
    raycast: &RaycastFn2D,
    listener_position: Vec2,
    source_position: Vec2,
    max_distance: f32,
    reflection_rays: u32,
) -> Option<AudioRayTraceResult> {
    let desc = sys::Moss_AudioRayTrace2DDesc {
        physics: physics.as_raw(),
        raycast: Some(raycast_2d_trampoline),
        user_data: raycast as *const RaycastFn2D as *mut c_void,
        listener_position: sys_vec2(listener_position),
        source_position: sys_vec2(source_position),
        max_distance,
        reflection_rays,
        direct_occlusion_strength: 1.0,
        reflection_strength: 0.35,
        air_absorption: 0.02,
    };
    let mut out = sys::Moss_AudioRayTraceResult::default();
    let ok = unsafe { sys::Moss_AudioRayTrace2D(&desc, &mut out) };
    ok.then(|| out.into())
}

/// One-off 3D trace, mirroring [`ray_trace_2d`].
pub fn ray_trace_3d(
    physics: &PhysicsHandle,
    raycast: &RaycastFn3D,
    listener_position: Vec3,
    source_position: Vec3,
    max_distance: f32,
    reflection_rays: u32,
) -> Option<AudioRayTraceResult> {
    let desc = sys::Moss_AudioRayTrace3DDesc {
        physics: physics.as_raw(),
        raycast: Some(raycast_3d_trampoline),
        user_data: raycast as *const RaycastFn3D as *mut c_void,
        listener_position: sys_vec3(listener_position),
        source_position: sys_vec3(source_position),
        max_distance,
        reflection_rays,
        direct_occlusion_strength: 1.0,
        reflection_strength: 0.35,
        air_absorption: 0.02,
    };
    let mut out = sys::Moss_AudioRayTraceResult::default();
    let ok = unsafe { sys::Moss_AudioRayTrace3D(&desc, &mut out) };
    ok.then(|| out.into())
}

// ===========================================================================
// Speaker devices
// ===========================================================================

pub fn is_speaker_device_ready() -> bool {
    unsafe { sys::Moss_IsSpeakerDeviceReady() }
}

pub fn speaker_open() {
    unsafe { sys::Moss_AudioSpeakerOpen() }
}

pub fn speaker_pause() {
    unsafe { sys::Moss_AudioSpeakerPause() }
}

pub fn speaker_resume() {
    unsafe { sys::Moss_AudioSpeakerResume() }
}

pub fn speaker_is_paused() -> bool {
    unsafe { sys::Moss_AudioSpeakerIsPaused() }
}

pub fn select_speaker_device(id: i32) -> bool {
    unsafe { sys::Moss_AudioSelectSpeakerDevice(id) }
}

pub fn current_speaker_device_id() -> i32 {
    unsafe { sys::Moss_GetCurrentSpeakerDeviceID() }
}

pub fn speaker_device_name(id: i32) -> Option<String> {
    unsafe { owned_string(sys::Moss_GetSpeakerDeviceName(id)) }
}

/// All speaker device names, indexed by device ID (`0..count`).
pub fn list_speaker_devices() -> Vec<String> {
    let count = unsafe { sys::Moss_ListSpeakerDevices() };
    (0..count).filter_map(speaker_device_name).collect()
}

// ===========================================================================
// Microphone — see header note 7 about the duplicate global API left out.
// ===========================================================================

/// An open microphone capture device. Closed on drop.
pub struct Microphone {
    raw: NonNull<sys::Moss_Microphone>,
    // The pointer handed to the native side as `Moss_MicrophoneSetCallback`
    // userdata. Kept as a raw pointer, not an owned `Box`, because the
    // native side may call back through it at any time until it's
    // explicitly unregistered — see `clear_callback`.
    callback_data: Option<*mut Box<dyn FnMut(&[f32]) + 'static>>,
}

unsafe extern "C" fn microphone_trampoline(buffer: *const f32, samples: i32, user_data: *mut c_void) {
    if buffer.is_null() || samples <= 0 || user_data.is_null() {
        return;
    }
    let callback = &mut *(user_data as *mut Box<dyn FnMut(&[f32])>);
    let slice = std::slice::from_raw_parts(buffer, samples as usize);
    callback(slice);
}

impl Microphone {
    pub fn open(desc: &MicrophoneDesc) -> Result<Self> {
        let raw_desc = sys::Moss_MicrophoneDesc {
            device_index: desc.device_index,
            sample_rate: desc.sample_rate,
            channels: desc.channels,
            buffer_frames: desc.buffer_frames,
            ring_buffer_frames: desc.ring_buffer_frames,
            start_immediately: desc.start_immediately,
            enable_voice_metrics: desc.enable_voice_metrics,
        };
        let raw = unsafe { sys::Moss_MicrophoneOpen(&raw_desc) };
        NonNull::new(raw)
            .map(|raw| Self { raw, callback_data: None })
            .ok_or(MossError::Unsupported)
    }

    pub fn start(&self) -> bool {
        unsafe { sys::Moss_MicrophoneStart(self.raw.as_ptr()) }
    }

    pub fn stop(&self) {
        unsafe { sys::Moss_MicrophoneStop(self.raw.as_ptr()) }
    }

    /// Reads up to `out.len()` frames into `out`, returning how many were
    /// actually read.
    pub fn read(&self, out: &mut [f32]) -> u32 {
        unsafe { sys::Moss_MicrophoneRead(self.raw.as_ptr(), out.as_mut_ptr(), out.len() as u32) }
    }

    pub fn set_gain(&self, gain: f32) {
        unsafe { sys::Moss_MicrophoneSetGain(self.raw.as_ptr(), gain) }
    }

    pub fn sample_rate(&self) -> u32 {
        unsafe { sys::Moss_MicrophoneGetSampleRate(self.raw.as_ptr()) }
    }

    pub fn channels(&self) -> u32 {
        unsafe { sys::Moss_MicrophoneGetChannels(self.raw.as_ptr()) }
    }

    /// Registers a callback invoked with each new block of captured
    /// samples. Replaces any previously-registered callback.
    pub fn set_callback(&mut self, callback: impl FnMut(&[f32]) + 'static) {
        self.clear_callback();
        let boxed: Box<dyn FnMut(&[f32])> = Box::new(callback);
        let ptr = Box::into_raw(Box::new(boxed));
        unsafe {
            sys::Moss_MicrophoneSetCallback(self.raw.as_ptr(), Some(microphone_trampoline), ptr as *mut c_void);
        }
        self.callback_data = Some(ptr);
    }

    /// Unregisters the current callback (if any) and frees it. Tells the
    /// native side to stop calling back *before* freeing the boxed
    /// closure — see the module-level note about the `NULL`-callback
    /// assumption this relies on.
    fn clear_callback(&mut self) {
        if let Some(ptr) = self.callback_data.take() {
            unsafe {
                sys::Moss_MicrophoneSetCallback(self.raw.as_ptr(), None, std::ptr::null_mut());
                drop(Box::from_raw(ptr));
            }
        }
    }

    pub fn levels(&self) -> MicrophoneLevels {
        let l = unsafe { sys::Moss_MicrophoneGetLevels(self.raw.as_ptr()) };
        MicrophoneLevels {
            rms: l.rms,
            peak: l.peak,
            smoothed_volume: l.smoothed_volume,
            voice_activity: l.voice_activity,
        }
    }
}

impl Drop for Microphone {
    fn drop(&mut self) {
        self.clear_callback();
        unsafe { sys::Moss_MicrophoneClose(self.raw.as_ptr()) }
    }
}

pub fn microphone_device_count() -> u32 {
    unsafe { sys::Moss_MicrophoneGetDeviceCount() }
}

pub fn microphone_device_name(index: u32) -> Option<String> {
    unsafe { owned_string(sys::Moss_MicrophoneGetDeviceName(index)) }
}

/// All microphone device names, indexed by device index (`0..count`).
pub fn list_microphone_devices() -> Vec<String> {
    (0..microphone_device_count()).filter_map(microphone_device_name).collect()
}
