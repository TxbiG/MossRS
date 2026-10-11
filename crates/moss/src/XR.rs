//! Cross-platform XR (VR/AR/MR), built on OpenXR 1.1. Enabled by the `xr`
//! feature (`openxr` implies it, per `Cargo.toml`).
//!
//! Safe wrapper around `Moss_XR.h`. The header carries its own warning
//! worth repeating here: its author doesn't have a headset to test
//! against, so **this API path is unverified upstream**, not just in this
//! Rust wrapper — expect rough edges.
//!
//! ## Assumptions
//! Same C++-linkage assumption as `audio.rs` (this header isn't `extern
//! "C"`, though unlike `Moss_Audio.h` it has no overloads or reference
//! parameters, so it's a much smaller ask of bindgen). Enums without an
//! explicit underlying type (`XRBodyJoint`, `XRHandJoint`) are assumed
//! `#[repr(i32)]`, C++'s default for a fixed-type-less `enum class`.
//!
//! `Quat` and `Mat44`, used in [`Pose`] and [`View`], aren't defined in
//! this header or any other header seen so far — see the note on
//! `crate::math::Quat`/`Mat44` for why those are bindgen-generated type
//! aliases rather than hand-written structs here.
//!
//! `ETextureFormat` (used by [`Swapchain::new`]) is defined in
//! `Moss_GPU.h`, not this header, and `gpu.rs` doesn't exist in this crate
//! yet — [`Swapchain::new`] takes the raw `moss_sys::ETextureFormat` for
//! now. Swap it for a proper `crate::gpu::TextureFormat` once that module
//! lands.
//!
//! `Moss_Renderer` (from `Moss_Renderer.h`, needed by [`init`]) is only
//! ever forward-declared in what's been reviewed so far — [`RendererHandle`]
//! is an opaque passthrough for it, the same pattern `audio.rs` uses for
//! `PhysicsHandle`. Once `renderer.rs` grows a real wrapper, that should
//! become the source of this handle instead of callers constructing it
//! themselves.
//!
//! ## Declared but unused in this header
//! `MossXR_Context`, `MossXR_Space`, and `MossXR_Origin` are
//! forward-declared and never appear in any function signature here — the
//! header's own top-of-file TODO comments (frame-graph layering, subaction
//! paths, per-feature capability probing) suggest these are placeholders
//! for work in progress. `Moss_XRImageLayout` and `Moss_XREventType` are
//! likewise declared but never consumed or returned by anything —
//! `Moss_XR_HandleEvents` takes no output parameter, so there's currently
//! no way to actually observe an event's type or payload. None of these
//! five are bound to anything below; there's nothing yet to bind.
//!
//! Output-parameter structs (`MossXR_Pose`, `MossXR_View`) are
//! zero-initialized with `std::mem::zeroed()` rather than `::default()`
//! before being passed to the native call that fills them in — this
//! doesn't require `moss-sys`'s bindgen to be configured with
//! `derive_default`, unlike `platform.rs`/`audio.rs`'s use of `::default()`
//! for the same purpose (`Moss_PathInfo::default()`,
//! `Moss_AudioRayTraceResult::default()`). Worth aligning those two to the
//! same `zeroed()` approach if bindgen isn't actually deriving `Default`
//! for you — either works if it is.
//!
//! `Moss_XR_PlayHaptic` is commented out in the header (its only
//! declaration references `XrPath`, an OpenXR type not otherwise
//! `#include`d here), so [`Action::stop_haptic`] exists with no
//! corresponding way to start one yet.
//!
//! Face tracking is asymmetric with hand/body tracking: [`Face::new`] /
//! `Drop` exist, but there's no `Moss_XR_GetFaceExpression`-equivalent and
//! no `Moss_XR_UpdateFaceTracking` — a `Face` can currently be created and
//! destroyed and nothing else.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr::NonNull;

use moss_sys as sys;

use crate::error::{MossError, Result};
use crate::math::{Mat44, Quat, Vec2, Vec3};

fn to_cstring(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| MossError::InvalidArgument)
}

/// Field-by-field copy into the native `Vec3` layout (mirrors the
/// `sys_vec3` helper in `audio.rs` — not a pointer cast, so this doesn't
/// depend on `crate::math::Vec3` and `sys::Vec3` being the same Rust type).
fn sys_vec3(v: Vec3) -> sys::Vec3 {
    sys::Vec3 { x: v.x, y: v.y, z: v.z }
}

fn sys_vec2(v: Vec2) -> sys::Vec2 {
    sys::Vec2 { x: v.x, y: v.y }
}

unsafe fn borrowed_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        None
    } else {
        CStr::from_ptr(ptr).to_str().ok()
    }
}

// ===========================================================================
// IDs
// ===========================================================================

pub type XrTime = sys::MossXR_Time;

/// An opaque handle to the Moss renderer — see the module-level note.
pub struct RendererHandle(NonNull<sys::Moss_Renderer>);

impl RendererHandle {
    /// # Safety
    /// `raw` must be a valid `Moss_Renderer*` for as long as this handle
    /// (and anything it's passed to) is in use.
    pub unsafe fn from_raw(raw: *mut sys::Moss_Renderer) -> Option<Self> {
        NonNull::new(raw).map(Self)
    }
}

// ===========================================================================
// Enums
// ===========================================================================

/// Mirrors `MossXR_SessionState`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    Unknown,
    Ready,
    Synchronized,
    Visible,
    Focused,
    Stopping,
    Exiting,
    LossPending,
}

/// Mirrors `MossXR_ActionType`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionType {
    Boolean,
    Float,
    Vec2,
    Pose,
    Haptic,
}

/// Mirrors `MossXR_ViewType`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewType {
    Mono,
    Stereo,
}

/// Mirrors `MossXR_Handedness`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Handedness {
    Left,
    Right,
}

/// Mirrors `XRBodyJoint`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyJoint {
    Root,
    Spine,
    Chest,
    Neck,
    Head,
    LeftShoulder,
    LeftElbow,
    LeftHand,
    RightShoulder,
    RightElbow,
    RightHand,
    LeftHip,
    LeftKnee,
    LeftFoot,
    RightHip,
    RightKnee,
    RightFoot,
    /// Not a real joint — mirrors the header's `XR_BODY_JOINT_COUNT`
    /// sentinel, kept for parity rather than usefulness on its own.
    JointCount,
}

/// Mirrors `XRHandJoint`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandJoint {
    Wrist,
    ThumbMetacarpal,
    ThumbProximal,
    ThumbDistal,
    ThumbTip,
    IndexMetacarpal,
    IndexProximal,
    IndexIntermediate,
    IndexDistal,
    IndexTip,
    MiddleMetacarpal,
    MiddleProximal,
    MiddleIntermediate,
    MiddleDistal,
    MiddleTip,
    RingMetacarpal,
    RingProximal,
    RingIntermediate,
    RingDistal,
    RingTip,
    PinkyMetacarpal,
    PinkyProximal,
    PinkyIntermediate,
    PinkyDistal,
    PinkyTip,
    /// Sentinel — see [`BodyJoint::JointCount`].
    JointCount,
}

macro_rules! transmute_into {
    ($from:ty => $to:ty) => {
        impl From<$from> for $to {
            fn from(value: $from) -> Self {
                // SAFETY: `$from` is defined above with the same
                // discriminants, in the same order, as the native `$to`.
                unsafe { std::mem::transmute(value) }
            }
        }
    };
}

transmute_into!(ActionType => sys::MossXR_ActionType);
transmute_into!(BodyJoint => sys::XRBodyJoint);
transmute_into!(HandJoint => sys::XRHandJoint);

impl From<sys::MossXR_SessionState> for SessionState {
    fn from(value: sys::MossXR_SessionState) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl From<sys::MossXR_ViewType> for ViewType {
    fn from(value: sys::MossXR_ViewType) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

// ===========================================================================
// Plain data types
// ===========================================================================

/// Mirrors `MossXR_Pose`.
#[derive(Debug, Clone, Copy)]
pub struct Pose {
    pub position: Vec3,
    pub orientation: Quat,
}

/// Mirrors `MossXR_Fov`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fov {
    pub left: f32,
    pub right: f32,
    pub up: f32,
    pub down: f32,
}

/// Mirrors `MossXR_View`.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub pose: Pose,
    pub fov: Fov,
    pub view: Mat44,
    pub projection: Mat44,
}

/// Mirrors `MossXR_Capabilities`.
#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    pub view_count: u32,
    pub view_type: ViewType,
    pub hand_tracking: bool,
    pub eye_tracking: bool,
    pub body_tracking: bool,
    pub face_tracking: bool,
    pub passthrough: bool,
    pub anchors: bool,
    pub depth_layers: bool,
}

impl From<&sys::MossXR_Capabilities> for Capabilities {
    fn from(c: &sys::MossXR_Capabilities) -> Self {
        Self {
            view_count: c.viewCount,
            view_type: c.viewType.into(),
            hand_tracking: c.handTracking,
            eye_tracking: c.eyeTracking,
            body_tracking: c.bodyTracking,
            face_tracking: c.faceTracking,
            passthrough: c.passthrough,
            anchors: c.anchors,
            depth_layers: c.depthLayers,
        }
    }
}

fn sys_pose(p: Pose) -> sys::MossXR_Pose {
    sys::MossXR_Pose { position: sys_vec3(p.position), orientation: p.orientation }
}

fn pose_from_sys(p: &sys::MossXR_Pose) -> Pose {
    Pose {
        position: Vec3::new(p.position.x, p.position.y, p.position.z),
        orientation: p.orientation,
    }
}

fn view_from_sys(v: &sys::MossXR_View) -> View {
    View {
        pose: pose_from_sys(&v.pose),
        fov: Fov { left: v.fov.left, right: v.fov.right, up: v.fov.up, down: v.fov.down },
        view: v.view,
        projection: v.projection,
    }
}

// ===========================================================================
// Instance
// ===========================================================================

/// Initializes the XR instance against the given renderer. Must be called
/// before anything else in this module.
pub fn init(renderer: &RendererHandle) -> Result<()> {
    let ok = unsafe { sys::Moss_XR_Initialize(renderer.0.as_ptr()) };
    ok.then_some(()).ok_or(MossError::NotInitialized)
}

pub fn shutdown() {
    unsafe { sys::Moss_XR_Shutdown() }
}

/// Runtime/device capabilities. `None` before [`init`] or if the native
/// side has no capabilities to report yet.
pub fn capabilities() -> Option<Capabilities> {
    let raw = unsafe { sys::Moss_XR_GetCapabilities() };
    unsafe { raw.as_ref() }.map(Capabilities::from)
}

pub fn enable_validation_layers() -> bool {
    unsafe { sys::Moss_XR_EnableValidationLayers() }
}

pub fn backend_name() -> Option<&'static str> {
    unsafe { borrowed_str(sys::Moss_XR_GetBackendName()) }
}

pub fn backend_version() -> u32 {
    unsafe { sys::Moss_XR_GetBackendVersion() }
}

/// Time elapsed since the last frame, in seconds. Not tied to a specific
/// [`Session`] — the native function takes no session parameter, so this
/// reports on whichever session Moss currently considers active, same
/// caveat as `platform::active_window_width`.
pub fn delta_seconds() -> f32 {
    unsafe { sys::Moss_XR_GetDeltaSeconds() }
}

pub fn begin_debug_label(label: &str) -> Result<()> {
    let c_label = to_cstring(label)?;
    unsafe { sys::Moss_XR_BeginDebugLabel(c_label.as_ptr()) };
    Ok(())
}

pub fn end_debug_label() {
    unsafe { sys::Moss_XR_EndDebugLabel() }
}

pub fn enable_passthrough(enable: bool) -> bool {
    unsafe { sys::Moss_XR_EnablePassthrough(enable) }
}

pub fn enable_foveated_rendering(enable: bool) -> bool {
    unsafe { sys::Moss_XR_EnableFoveatedRendering(enable) }
}

// ===========================================================================
// Session
// ===========================================================================

/// An XR session. Destroyed on drop.
pub struct Session {
    raw: NonNull<sys::MossXR_Session>,
}

impl Session {
    pub fn new() -> Result<Self> {
        let raw = unsafe { sys::Moss_XR_CreateSession() };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::NotInitialized)
    }

    pub fn state(&self) -> SessionState {
        unsafe { sys::Moss_XR_GetSessionState(self.raw.as_ptr()).into() }
    }

    /// Begins a frame. Returns `false` if the session isn't in a state to
    /// render this frame (e.g. not visible) — not an error, just "skip
    /// rendering this iteration".
    pub fn begin_frame(&self) -> bool {
        unsafe { sys::Moss_XR_BeginFrame(self.raw.as_ptr()) }
    }

    pub fn end_frame(&self) {
        unsafe { sys::Moss_XR_EndFrame(self.raw.as_ptr()) }
    }

    pub fn predicted_display_time(&self) -> XrTime {
        unsafe { sys::Moss_XR_GetPredictedDisplayTime(self.raw.as_ptr()) }
    }

    /// Pumps the XR runtime's event queue. See the module-level note about
    /// `Moss_XREventType` currently having no way to observe what, if
    /// anything, this processed.
    pub fn handle_events(&self) {
        unsafe { sys::Moss_XR_HandleEvents(self.raw.as_ptr()) }
    }

    pub fn view_count(&self) -> u32 {
        unsafe { sys::Moss_XR_GetViewCount(self.raw.as_ptr()) }
    }

    /// Gets view `index` (an eye, in stereo). `None` if the native call
    /// reports failure (e.g. `index` out of range).
    pub fn view(&self, index: u32) -> Option<View> {
        let mut out = unsafe { std::mem::zeroed() };
        let ok = unsafe { sys::Moss_XR_GetView(self.raw.as_ptr(), index, &mut out) };
        ok.then(|| view_from_sys(&out))
    }

    /// All views for this session (`0..view_count()`), skipping any index
    /// the native call fails on rather than stopping early.
    pub fn views(&self) -> Vec<View> {
        (0..self.view_count()).filter_map(|i| self.view(i)).collect()
    }

    pub fn attach_action_set(&self, set: &ActionSet) {
        unsafe { sys::Moss_XR_AttachActionSet(self.raw.as_ptr(), set.raw.as_ptr()) }
    }

    pub fn sync_actions(&self) {
        unsafe { sys::Moss_XR_SyncActions(self.raw.as_ptr()) }
    }

    pub fn update_body_tracking(&self) {
        unsafe { sys::Moss_XR_UpdateBodyTracking(self.raw.as_ptr()) }
    }

    pub fn update_hand_tracking(&self) {
        unsafe { sys::Moss_XR_UpdateHandTracking(self.raw.as_ptr()) }
    }

    /// Submits composition layers for this session's frame, in order
    /// (first layer is drawn first / lowest).
    pub fn submit_layers(&self, layers: &[&Layer]) {
        let mut raw_layers: Vec<*mut sys::MossXR_Layer> =
            layers.iter().map(|l| l.raw.as_ptr()).collect();
        unsafe {
            sys::Moss_XR_SubmitLayers(
                self.raw.as_ptr(),
                raw_layers.len() as u32,
                raw_layers.as_mut_ptr(),
            );
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroySession(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Swapchains
// ===========================================================================

/// An XR swapchain. Destroyed on drop.
///
/// See the module-level note: `format` is the raw `moss_sys::ETextureFormat`
/// until this crate has a `gpu` module of its own.
pub struct Swapchain {
    raw: NonNull<sys::MossXR_Swapchain>,
}

impl Swapchain {
    pub fn new(
        session: &Session,
        width: u32,
        height: u32,
        format: sys::ETextureFormat,
    ) -> Result<Self> {
        let raw = unsafe { sys::Moss_XR_CreateSwapchain(session.raw.as_ptr(), width, height, format) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    /// Acquires the next image to render into. The returned pointer's real
    /// type depends on the active graphics backend (a `VkImage`, a GL
    /// texture name cast to a pointer, etc.) — this crate has no way to
    /// know which without a `gpu` module, so it's left as a raw pointer
    /// for the caller to reinterpret per-backend.
    pub fn acquire_image(&self) -> Option<(*mut std::os::raw::c_void, u32)> {
        let mut index = 0u32;
        let ptr = unsafe { sys::Moss_XR_AcquireSwapchainImage(self.raw.as_ptr(), &mut index) };
        (!ptr.is_null()).then_some((ptr, index))
    }

    pub fn release_image(&self, image_index: u32) {
        unsafe { sys::Moss_XR_ReleaseSwapchainImage(self.raw.as_ptr(), image_index) }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroySwapchain(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Anchors
// ===========================================================================

/// A spatial anchor. Destroyed on drop.
pub struct Anchor {
    raw: NonNull<sys::MossXR_Anchor>,
}

impl Anchor {
    pub fn new(pose: Pose) -> Result<Self> {
        let raw_pose = sys_pose(pose);
        let raw = unsafe { sys::Moss_XR_CreateAnchor(&raw_pose) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    /// Current pose of this anchor at `time`. `None` if the native call
    /// reports it can't currently be located (e.g. tracking lost).
    pub fn locate(&self, time: XrTime) -> Option<Pose> {
        let mut out = unsafe { std::mem::zeroed() };
        let ok = unsafe { sys::Moss_XR_LocateAnchor(self.raw.as_ptr(), time, &mut out) };
        ok.then(|| pose_from_sys(&out))
    }
}

impl Drop for Anchor {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroyAnchor(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Composition layers
// ===========================================================================

/// A composition layer (projection or quad). Destroyed on drop.
pub struct Layer {
    raw: NonNull<sys::MossXR_Layer>,
}

impl Layer {
    /// A full-view projection layer — the usual "world" layer for stereo
    /// rendering.
    pub fn projection(swapchain: &Swapchain) -> Result<Self> {
        let raw = unsafe { sys::Moss_XR_CreateProjectionLayer(swapchain.raw.as_ptr()) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    /// A flat quad layer at a fixed pose/size — for UI panels and the like.
    pub fn quad(swapchain: &Swapchain, pose: Pose, size: Vec2) -> Result<Self> {
        let raw_pose = sys_pose(pose);
        let raw_size = sys_vec2(size);
        let raw =
            unsafe { sys::Moss_XR_CreateQuadLayer(swapchain.raw.as_ptr(), &raw_pose, raw_size) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroyLayer(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Actions
// ===========================================================================

/// A group of related input actions. Destroyed on drop.
pub struct ActionSet {
    raw: NonNull<sys::MossXR_ActionSet>,
}

impl ActionSet {
    pub fn new(name: &str) -> Result<Self> {
        let c_name = to_cstring(name)?;
        let raw = unsafe { sys::Moss_XR_CreateActionSet(c_name.as_ptr()) };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::OutOfMemory)
    }

    pub fn create_action(&self, name: &str, action_type: ActionType) -> Result<Action> {
        let c_name = to_cstring(name)?;
        let raw = unsafe {
            sys::Moss_XR_CreateAction(self.raw.as_ptr(), c_name.as_ptr(), action_type.into())
        };
        NonNull::new(raw).map(|raw| Action { raw }).ok_or(MossError::OutOfMemory)
    }
}

impl Drop for ActionSet {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroyActionSet(self.raw.as_ptr()) }
    }
}

/// A single input action (button, axis, pose, or haptic output). Destroyed
/// on drop. Owned independently of its parent [`ActionSet`] — the header
/// gives actions their own destroy function, so this doesn't borrow from
/// the set the way e.g. a `Gamepad`'s state might.
pub struct Action {
    raw: NonNull<sys::MossXR_Action>,
}

impl Action {
    pub fn as_bool(&self) -> bool {
        unsafe { sys::Moss_XR_GetActionBoolean(self.raw.as_ptr()) }
    }

    pub fn as_float(&self) -> f32 {
        unsafe { sys::Moss_XR_GetActionFloat(self.raw.as_ptr()) }
    }

    /// `None` if the native call reports the pose currently isn't valid
    /// (e.g. controller not tracked).
    pub fn as_pose(&self) -> Option<Pose> {
        let mut out = unsafe { std::mem::zeroed() };
        let ok = unsafe { sys::Moss_XR_GetActionPose(self.raw.as_ptr(), &mut out) };
        ok.then(|| pose_from_sys(&out))
    }

    /// Stops any haptic effect currently playing on this action. See the
    /// module-level note: there's currently no way to *start* one from
    /// this header (`Moss_XR_PlayHaptic` is commented out upstream).
    pub fn stop_haptic(&self) {
        unsafe { sys::Moss_XR_StopHaptic(self.raw.as_ptr()) }
    }
}

impl Drop for Action {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroyAction(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Tracking: hand, body, face
// ===========================================================================

/// Hand tracking state for one hand. Destroyed on drop. Call
/// [`Session::update_hand_tracking`] each frame before reading poses.
pub struct Hand {
    raw: NonNull<sys::MossXR_HandModifier>,
}

impl Hand {
    pub fn new() -> Result<Self> {
        let raw = unsafe { sys::Moss_XR_CreateHand() };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::Unsupported)
    }

    /// Pose of the whole hand (typically the palm or wrist — the header
    /// doesn't say which).
    pub fn pose(&self) -> Option<Pose> {
        let mut out = unsafe { std::mem::zeroed() };
        let ok = unsafe { sys::Moss_XR_GetHandPose(self.raw.as_ptr(), &mut out) };
        ok.then(|| pose_from_sys(&out))
    }

    pub fn joint_pose(&self, joint: HandJoint) -> Option<Pose> {
        let mut out = unsafe { std::mem::zeroed() };
        let ok = unsafe { sys::Moss_XR_GetHandJointPose(self.raw.as_ptr(), joint.into(), &mut out) };
        ok.then(|| pose_from_sys(&out))
    }
}

impl Drop for Hand {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroyHand(self.raw.as_ptr()) }
    }
}

/// Body tracking state. Destroyed on drop. Call
/// [`Session::update_body_tracking`] each frame before reading joints.
pub struct Body {
    raw: NonNull<sys::MossXR_BodyModifier>,
}

impl Body {
    pub fn new() -> Result<Self> {
        let raw = unsafe { sys::Moss_XR_CreateBody() };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::Unsupported)
    }

    pub fn joint_pose(&self, joint: BodyJoint) -> Option<Pose> {
        let mut out = unsafe { std::mem::zeroed() };
        let ok = unsafe { sys::Moss_XR_GetBodyJointPose(self.raw.as_ptr(), joint.into(), &mut out) };
        ok.then(|| pose_from_sys(&out))
    }
}

impl Drop for Body {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroyBody(self.raw.as_ptr()) }
    }
}

/// Face tracking handle. Destroyed on drop. See the module-level note:
/// there's currently nothing to *read* from this — no expression/blendshape
/// getter and no per-frame update function exist in this header yet.
pub struct Face {
    raw: NonNull<sys::MossXR_FaceModifier>,
}

impl Face {
    pub fn new() -> Result<Self> {
        let raw = unsafe { sys::Moss_XR_CreateFace() };
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(MossError::Unsupported)
    }
}

impl Drop for Face {
    fn drop(&mut self) {
        unsafe { sys::Moss_XR_DestroyFace(self.raw.as_ptr()) }
    }
}
