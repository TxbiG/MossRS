//! Platform backend: windowing, monitors, input, and OS-level utilities.
//!
//! Safe wrapper around `Moss_Platform.h`. This assumes `moss-sys`'s
//! `build.rs` configures bindgen with `.rustified_enum(...)` for the C++
//! `enum class` types below, so e.g. `sys::Keyboard` is a plain Rust enum
//! with matching variant names and discriminants. If your bindgen setup
//! instead leaves them as newtype structs with associated consts, the
//! `From` impls in this file need a small tweak (swap the `transmute`-based
//! conversions for the struct's associated constants) — everything else is
//! unaffected.
//!
//! ## Not yet wrapped here
//! - **Haptic feedback** (`Moss_*Haptic*`, ~30 functions) — left as a stub
//!   module. It has real design questions (effect handle lifetime, who
//!   owns `Moss_HapticEffect` buffers) that deserve a dedicated pass rather
//!   than a mechanical 1:1 transliteration.
//! - **Camera capture** (`Moss_*Camera*` / `Moss_Capture`) — same reasoning,
//!   plus frame-buffer ownership (`Moss_AcquireCameraFrame` /
//!   `Moss_ReleaseCameraFrame`) needs a borrow-checked design.
//! - **GPU backend interop** (`vulkan`/`opengl`/`metal` cfg blocks) — thin
//!   pass-throughs are sketched at the bottom behind the matching Cargo
//!   features, but the Vulkan ones use raw handles rather than e.g. `ash`
//!   types; wire up a real Vulkan crate's handle types before using them.
//! - The window/monitor callbacks (`Moss_Set*Callback`) are plain C
//!   function pointers with **no userdata**, so only one Rust closure can
//!   be registered at a time, globally, per callback. None are wrapped yet;
//!   the dialog/directory callbacks below (which *do* carry userdata) show
//!   the trampoline pattern to reuse for them.
//!
//! ## A bug worth fixing upstream: `Moss_WindowFlags`
//! In the header, `Moss_WindowFlags` mixes small sequential values
//! (`TRANSPARENT = 3`, `POPUP = 5`, `ALWAYS_ON_TOP = 20`, ...) with actual
//! bitmask values (`SHOWN = 0x00000004`, `HIDDEN = 0x00000008`, ...). These
//! two numbering schemes collide — `SHOWN = 0x4` and a hypothetical flag at
//! plain value `4` would overlap — so the enum can't safely be treated as
//! a `bitflags`-style set as written. It's also not accepted as a
//! parameter by any function in this header (`Moss_CreateWindow` takes no
//! flags), so it's left out of this wrapper entirely rather than
//! reproduced incorrectly. Worth picking one numbering scheme upstream
//! before wiring flags into `Moss_CreateWindow`.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::PathBuf;
use std::ptr::NonNull;

use moss_sys as sys;

use crate::error::{MossError, Result};

// ===========================================================================
// IDs
// ===========================================================================

pub type FingerId = sys::Moss_FingerID;
pub type TouchId = sys::Moss_TouchID;
pub type CameraId = sys::Moss_CameraID;
pub type GamepadId = sys::Moss_GamepadID;
pub type PropertiesId = sys::Moss_PropertiesID;
pub type PenId = sys::Moss_PenID;

// ===========================================================================
// Enums
// ===========================================================================

/// Keyboard scan code. Mirrors `Keyboard` in `Moss_Platform.h`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Apostrophe, Backslash, Comma, Equal, GraveAccent, LeftBracket, Minus,
    Period, RightBracket, Semicolon, Slash, World1, World2,
    Backspace, Delete, End, Enter, Escape, Home, Insert, Menu, PageDown,
    PageUp, Pause, Space, Tab, CapsLock, NumLock, ScrollLock,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    LeftAlt, LeftControl, LeftShift, LeftSuper,
    PrintScreen, RightAlt, RightControl, RightShift, RightSuper,
    Down, Left, Right, Up,
    Kp0, Kp1, Kp2, Kp3, Kp4, Kp5, Kp6, Kp7, Kp8, Kp9,
    KpAdd, KpDecimal, KpDivide, KpEnter, KpEqual, KpMultiply, KpSubtract,
}

/// Mouse button. Mirrors `Mouse`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
    Button6,
    Button7,
    Button8,
}

/// Gamepad analog axis. Mirrors `GamepadAxis`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
    TouchpadX,
    TouchpadY,
    GyroX,
    GyroY,
    GyroZ,
}

/// Gamepad button. Mirrors `Moss_GamepadButton` (the enum the native API
/// actually uses — the older `Gamepad` enum in the header isn't referenced
/// by any function and is left out here as dead).
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    Misc1,
    RightPaddle1,
    LeftPaddle1,
    RightPaddle2,
    LeftPaddle2,
    Touchpad,
    Misc2,
    Misc3,
    Misc4,
    Misc5,
    Misc6,
}

/// Mirrors `Moss_MessageBoxFlags`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageBoxKind {
    AbortRetryIgnore = 0,
    CancelTryContinue = 1,
    Ok = 4,
    OkCancel = 5,
    RetryCancel = 6,
    YesNo = 7,
    YesNoCancel = 8,
}

/// Mirrors `Moss_PowerState`.
#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerState {
    Error = -1,
    Unknown = 0,
    OnBattery,
    NoBattery,
    Charging,
    Charged,
}

/// Mirrors `Moss_FileDialogType`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileDialogType {
    OpenFile,
    SaveFile,
    OpenFolder,
}

/// Mirrors `Moss_PenDeviceType`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PenDeviceType {
    Invalid = -1,
    Unknown = 0,
    Direct,
    Indirect,
}

/// Mirrors `Moss_TouchDeviceType`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchDeviceType {
    Invalid = -1,
    Direct = 0,
    IndirectAbsolute,
    IndirectRelative,
}

/// Mirrors `Moss_UserFolder`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserFolder {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    AppData,
    Cache,
}

/// Mirrors `Moss_PathType`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathType {
    Unknown = 0,
    File,
    Directory,
    Symlink,
}

/// Directory/file glob options. Mirrors `Moss_GlobFlags` — unlike
/// `WindowFlags` above, these values are already proper, non-colliding
/// powers of two, so combining them with `|` is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobFlags(u32);

impl GlobFlags {
    pub const NONE: Self = Self(0);
    pub const RECURSIVE: Self = Self(1 << 0);
    pub const CASE_INSENSITIVE: Self = Self(1 << 1);
    pub const FILES_ONLY: Self = Self(1 << 2);
    pub const DIRECTORIES_ONLY: Self = Self(1 << 3);
    pub const INCLUDE_HIDDEN: Self = Self(1 << 4);
    pub const FOLLOW_SYMLINKS: Self = Self(1 << 5);
    pub const FAIL_ON_ERROR: Self = Self(1 << 6);
}

impl std::ops::BitOr for GlobFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

// ===========================================================================
// Plain data types
// ===========================================================================

/// Mirrors `Moss_Locale`, but owns its strings instead of borrowing from
/// native memory of unknown lifetime.
#[derive(Debug, Clone)]
pub struct Locale {
    pub country: String,
    pub language: String,
}

/// Mirrors `Moss_PathInfo`.
#[derive(Debug, Clone, Copy)]
pub struct PathInfo {
    pub kind: PathType,
    pub size: u64,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    // create_time / modify_time / access_time (`Moss_Time`) are left out
    // until `Moss_Time`'s representation is pinned down elsewhere in the
    // headers (Moss_stdinc.h) — add them here once that's wrapped.
}

/// Name + glob pattern pair for file dialogs. Mirrors `Moss_DialogFileFilter`.
pub struct DialogFileFilter<'a> {
    pub name: &'a str,
    pub pattern: &'a str,
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Reads a native `char*` as an owned `String`, treating `NULL` as `None`.
///
/// # Safety
/// `ptr` must either be null or point at a valid, nul-terminated C string
/// that is safe to read for the duration of this call.
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

/// Calls a Moss function that fills a fixed `char* out, int max_len` buffer,
/// and returns the result as an owned `String`.
fn read_into_buffer(max_len: usize, f: impl FnOnce(*mut c_char, i32) -> bool) -> Option<String> {
    let mut buf = vec![0u8; max_len];
    let ok = f(buf.as_mut_ptr() as *mut c_char, max_len as i32);
    if !ok {
        return None;
    }
    let nul = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Some(String::from_utf8_lossy(&buf[..nul]).into_owned())
}

// ===========================================================================
// Window
// ===========================================================================

/// An application window. Destroyed automatically on drop
/// (`Moss_TerminateWindow`).
pub struct Window {
    raw: NonNull<sys::Moss_Window>,
}

impl Window {
    /// Creates a new top-level window.
    pub fn new(title: &str, width: i32, height: i32) -> Result<Self> {
        Self::new_on_monitor(title, width, height, None, None)
    }

    /// Creates a window on a specific monitor, optionally sharing a context
    /// with another window (as OpenGL context sharing does).
    pub fn new_on_monitor(
        title: &str,
        width: i32,
        height: i32,
        monitor: Option<&Monitor>,
        share: Option<&Window>,
    ) -> Result<Self> {
        let c_title = to_cstring(title)?;
        let monitor_ptr = monitor.map_or(std::ptr::null_mut(), |m| m.raw.as_ptr());
        let share_ptr = share.map_or(std::ptr::null_mut(), |w| w.raw.as_ptr());
        let raw = unsafe {
            sys::Moss_CreateWindow(c_title.as_ptr(), width, height, monitor_ptr, share_ptr)
        };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or(MossError::OutOfMemory)
    }

    pub fn as_raw(&self) -> *mut sys::Moss_Window {
        self.raw.as_ptr()
    }

    /// True once the window has received a close request (e.g. the user
    /// clicked the OS close button). Doesn't close the window itself —
    /// that's this struct's `Drop`.
    pub fn should_close(&self) -> bool {
        unsafe { sys::Moss_ShouldWindowClose(self.raw.as_ptr()) }
    }

    /// Requests that the window close. Distinct from dropping this value,
    /// which unconditionally destroys the native window.
    pub fn request_close(&self) {
        unsafe { sys::Moss_CloseWindow(self.raw.as_ptr()) }
    }

    pub fn set_title(&self, title: &str) -> Result<()> {
        let c_title = to_cstring(title)?;
        unsafe { sys::Moss_SetWindowTitle(self.raw.as_ptr(), c_title.as_ptr()) };
        Ok(())
    }

    /// Shows a modal message box tied to this window.
    pub fn message_box(&self, title: &str, message: &str, kind: MessageBoxKind) -> Result<bool> {
        let c_title = to_cstring(title)?;
        let c_message = to_cstring(message)?;
        let shown = unsafe {
            sys::Moss_CreateMessageBox(
                c_title.as_ptr(),
                c_message.as_ptr(),
                kind as sys::Moss_MessageBoxFlags,
                self.raw.as_ptr(),
            )
        };
        Ok(shown)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { sys::Moss_TerminateWindow(self.raw.as_ptr()) }
    }
}

// TODO: confirm against native thread-safety guarantees before relying on
// this — Moss windows are typically tied to the platform thread that owns
// the event loop.
unsafe impl Send for Window {}

/// Pumps the platform event queue. Call once per frame from the thread that
/// owns the window(s).
pub fn poll_events() {
    unsafe { sys::Moss_PollEvents() }
}

/// Width of the currently active window, in pixels.
///
/// Note: the native function takes no window handle — it reports on
/// whichever window Moss currently considers "the" active one, not a
/// specific `Window` instance. Prefer this only when you have a single
/// window.
pub fn active_window_width() -> i32 {
    unsafe { sys::Moss_GetWindowWidth() }
}

/// Height of the currently active window, in pixels. See
/// [`active_window_width`] for the same caveat about which window this
/// reports on.
pub fn active_window_height() -> i32 {
    unsafe { sys::Moss_GetWindowHeight() }
}

// ===========================================================================
// Monitor
// ===========================================================================

/// A display monitor. Monitors are owned by the platform layer, not by
/// this wrapper, so there's no `Drop` impl here.
pub struct Monitor {
    raw: NonNull<sys::Moss_Monitor>,
}

impl Monitor {
    pub fn primary() -> Result<Self> {
        let raw = unsafe { sys::Moss_MonitorGetPrimary() };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or(MossError::NotInitialized)
    }

    /// `None` if there is no second monitor.
    pub fn secondary() -> Option<Self> {
        let raw = unsafe { sys::Moss_MonitorGetSecondary() };
        NonNull::new(raw).map(|raw| Self { raw })
    }

    pub fn as_raw(&self) -> *mut sys::Moss_Monitor {
        self.raw.as_ptr()
    }

    /// Physical size in millimetres, as `(width, height)`.
    pub fn physical_size_mm(&self) -> (i32, i32) {
        let mut width = 0;
        let mut height = 0;
        unsafe { sys::Moss_MonitorGetPhysicalSize(self.raw.as_ptr(), &mut width, &mut height) };
        (width, height)
    }

    /// DPI content scale, as `(x, y)`.
    pub fn content_scale(&self) -> (f32, f32) {
        let mut x = 0.0;
        let mut y = 0.0;
        unsafe { sys::Moss_MonitorGetContentScale(self.raw.as_ptr(), &mut x, &mut y) };
        (x, y)
    }

    /// Position of this monitor in the virtual desktop, as `(x, y)`.
    pub fn position(&self) -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        unsafe { sys::Moss_MonitorGetPosition(self.raw.as_ptr(), &mut x, &mut y) };
        (x, y)
    }

    pub fn name(&self) -> Option<String> {
        unsafe { owned_string(sys::Moss_MonitorGetName(self.raw.as_ptr())) }
    }

    pub fn set_gamma(&self, gamma: f32) {
        unsafe { sys::Moss_MonitorSetGamma(self.raw.as_ptr(), gamma) }
    }

    // `Moss_MonitorGetGammaRamp` / `Moss_MonitorSetGammaRamp` are left out:
    // `Moss_GammaRamp` in the header (`uint8_t* size, red, green, blue;`)
    // looks like a typo for four separate arrays sharing one pointer type,
    // which would need clarifying upstream before it can be wrapped safely.
}

// ===========================================================================
// Keyboard & mouse
// ===========================================================================

pub fn is_key_pressed(key: Key) -> bool {
    unsafe { sys::Moss_IsKeyPressed(key.into()) }
}

pub fn is_key_released(key: Key) -> bool {
    unsafe { sys::Moss_IsReleased(key.into()) }
}

pub fn is_key_just_pressed(key: Key) -> bool {
    unsafe { sys::Moss_IsKeyJustPressed(key.into()) }
}

pub fn is_key_just_released(key: Key) -> bool {
    unsafe { sys::Moss_IsKeyJustReleased(key.into()) }
}

pub fn is_mouse_pressed(button: MouseButton) -> bool {
    unsafe { sys::Moss_IsMousePressed(button.into()) }
}

pub fn is_mouse_released(button: MouseButton) -> bool {
    unsafe { sys::Moss_IsMouseReleased(button.into()) }
}

pub fn is_mouse_just_pressed(button: MouseButton) -> bool {
    unsafe { sys::Moss_IsMouseJustPressed(button.into()) }
}

pub fn is_mouse_just_released(button: MouseButton) -> bool {
    unsafe { sys::Moss_IsMouseJustReleased(button.into()) }
}

pub fn mouse_position() -> (i32, i32) {
    let mut x = 0;
    let mut y = 0;
    unsafe { sys::Moss_GetMousePosition(&mut x, &mut y) };
    (x, y)
}

pub fn set_mouse_position(x: i32, y: i32) {
    unsafe { sys::Moss_SetMousePosition(x, y) }
}

pub fn set_mouse_visible(visible: bool) {
    unsafe { sys::Moss_SetMouseVisible(visible) }
}

// ===========================================================================
// Gamepad
// ===========================================================================

pub fn num_gamepads() -> i32 {
    unsafe { sys::Moss_GetNumGamepads() }
}

pub fn update_gamepads() {
    unsafe { sys::Moss_UpdateGamepads() }
}

pub fn reload_gamepad_mappings() {
    unsafe { sys::Moss_ReloadGamepadMappings() }
}

/// An open gamepad handle. Closed automatically on drop.
pub struct Gamepad {
    raw: NonNull<sys::Moss_Gamepad>,
}

impl Gamepad {
    pub fn open(id: GamepadId) -> Option<Self> {
        let raw = unsafe { sys::Moss_OpenGamepad(id) };
        NonNull::new(raw).map(|raw| Self { raw })
    }

    pub fn is_connected(&self) -> bool {
        unsafe { sys::Moss_GamepadConnected(self.raw.as_ptr()) }
    }

    pub fn is_button_pressed(&self, button: GamepadButton) -> bool {
        unsafe { sys::Moss_IsGamepadButtonPressed(self.raw.as_ptr(), button.into()) }
    }

    pub fn is_button_just_pressed(&self, button: GamepadButton) -> bool {
        unsafe { sys::Moss_IsGamepadButtonJustPressed(self.raw.as_ptr(), button.into()) }
    }

    pub fn is_button_just_released(&self, button: GamepadButton) -> bool {
        unsafe { sys::Moss_IsGamepadButtonJustReleased(self.raw.as_ptr(), button.into()) }
    }

    pub fn axis(&self, axis: GamepadAxis) -> f32 {
        unsafe { sys::Moss_GetGamepadAxis(self.raw.as_ptr(), axis.into()) }
    }

    pub fn rumble(&self, low: u16, high: u16, duration_ms: u32) -> bool {
        unsafe { sys::Moss_RumbleGamepad(self.raw.as_ptr(), low, high, duration_ms) }
    }

    pub fn rumble_triggers(&self, left: u16, right: u16, duration_ms: u32) -> bool {
        unsafe { sys::Moss_RumbleGamepadTriggers(self.raw.as_ptr(), left, right, duration_ms) }
    }

    pub fn set_led(&self, r: u8, g: u8, b: u8) -> bool {
        unsafe { sys::Moss_SetGamepadLED(self.raw.as_ptr(), r, g, b) }
    }

    pub fn name(&self) -> Option<String> {
        unsafe { owned_string(sys::Moss_GetGamepadName(self.raw.as_ptr())) }
    }

    pub fn id(&self) -> GamepadId {
        unsafe { sys::Moss_GetGamepadID(self.raw.as_ptr()) }
    }

    pub fn player_index(&self) -> i32 {
        unsafe { sys::Moss_GetGamepadPlayerIndex(self.raw.as_ptr()) }
    }

    /// `(power_state, battery_percent)`.
    pub fn power_info(&self) -> (PowerState, i32) {
        let mut percent = 0;
        let state = unsafe { sys::Moss_GetGamepadPowerInfo(self.raw.as_ptr(), &mut percent) };
        (state.into(), percent)
    }

    pub fn mapping(&self) -> Option<String> {
        unsafe { owned_string(sys::Moss_GetGamepadMapping(self.raw.as_ptr())) }
    }

    pub fn set_mapping(&self, mapping: &str) -> Result<bool> {
        let c_mapping = to_cstring(mapping)?;
        Ok(unsafe { sys::Moss_SetGamepadMapping(self.raw.as_ptr(), c_mapping.as_ptr()) })
    }

    // Touchpad queries (`Moss_GetNumGamepadTouchpads` and friends) are left
    // out for now — they're a natural fit for an iterator-style API
    // (`gamepad.touchpads().flat_map(|pad| pad.fingers())`) that's worth
    // designing on its own rather than mirroring the raw index-based C
    // calls 1:1.
}

impl Drop for Gamepad {
    fn drop(&mut self) {
        unsafe { sys::Moss_CloseGamepad(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Pen & touch
// ===========================================================================

pub fn pen_device_type(pen: PenId) -> PenDeviceType {
    unsafe { sys::Moss_GetPenDeviceType(pen).into() }
}

pub fn touch_device_type(touch: TouchId) -> TouchDeviceType {
    unsafe { sys::Moss_GetTouchDeviceType(touch).into() }
}

pub fn touch_device_name(touch: TouchId) -> Option<String> {
    unsafe { owned_string(sys::Moss_GetTouchDeviceName(touch)) }
}

// ===========================================================================
// CPU / RAM / power / locale
// ===========================================================================

pub fn available_cpu_cores() -> i32 {
    unsafe { sys::Moss_GetAvailableCPUCores() }
}

pub fn cpu_cache_line_size() -> i32 {
    unsafe { sys::Moss_GetCPUCacheLineSize() }
}

/// System RAM, in megabytes (matches the native function's unit).
pub fn system_ram_mb() -> i32 {
    unsafe { sys::Moss_GetSystemRAM() }
}

pub fn open_url(url: &str) -> Result<bool> {
    let c_url = to_cstring(url)?;
    Ok(unsafe { sys::Moss_OpenURL(c_url.as_ptr()) })
}

pub fn locale() -> Option<Locale> {
    let raw = unsafe { sys::Moss_GetLocale() };
    if raw.is_null() {
        return None;
    }
    unsafe {
        let country = owned_string((*raw).country)?;
        let language = owned_string((*raw).language)?;
        Some(Locale { country, language })
    }
}

/// `(power_state, seconds_remaining, battery_percent)`.
pub fn power_info() -> (PowerState, i32, i32) {
    let mut seconds = 0;
    let mut percent = 0;
    let state = unsafe { sys::Moss_GetPowerInfo(&mut seconds, &mut percent) };
    (state.into(), seconds, percent)
}

pub fn is_process_running(executable_name: &str) -> Result<bool> {
    let c_name = to_cstring(executable_name)?;
    Ok(unsafe { sys::Moss_IsProcessRunningByName(c_name.as_ptr()) })
}

// ===========================================================================
// Dynamic library loading
// ===========================================================================

/// A loaded shared library (.dll/.so/.dylib). Unloaded on drop.
pub struct DynamicLibrary {
    raw: NonNull<c_void>,
}

impl DynamicLibrary {
    pub fn load(path: &str) -> Result<Self> {
        let c_path = to_cstring(path)?;
        let raw = unsafe { sys::Moss_LoadDynamicLibrary(c_path.as_ptr()) };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or(MossError::Unsupported)
    }

    /// Looks up a symbol and casts it to `T`.
    ///
    /// # Safety
    /// The caller must ensure `T` is the correct type for the symbol named
    /// `symbol_name` (typically a `fn` pointer type). Getting this wrong is
    /// undefined behavior — the native call has no way to check it.
    pub unsafe fn symbol<T: Copy>(&self, symbol_name: &str) -> Result<T> {
        assert_eq!(
            std::mem::size_of::<T>(),
            std::mem::size_of::<*mut c_void>(),
            "DynamicLibrary::symbol::<T> requires a pointer-sized T (typically an fn pointer)"
        );
        let c_name = to_cstring(symbol_name)?;
        let ptr = sys::Moss_GetLibrarySymbol(self.raw.as_ptr(), c_name.as_ptr());
        if ptr.is_null() {
            return Err(MossError::InvalidArgument);
        }
        Ok(std::mem::transmute_copy(&ptr))
    }
}

impl Drop for DynamicLibrary {
    fn drop(&mut self) {
        unsafe { sys::Moss_UnloadDynamicLibrary(self.raw.as_ptr()) }
    }
}

// ===========================================================================
// Filesystem
// ===========================================================================

const PATH_BUF_LEN: usize = 4096;

pub fn copy_file(src: &str, dst: &str, overwrite: bool) -> Result<bool> {
    let c_src = to_cstring(src)?;
    let c_dst = to_cstring(dst)?;
    Ok(unsafe { sys::Moss_CopyFile(c_src.as_ptr(), c_dst.as_ptr(), overwrite) })
}

pub fn create_directory(path: &str, recursive: bool) -> Result<bool> {
    let c_path = to_cstring(path)?;
    Ok(unsafe { sys::Moss_CreateDirectory(c_path.as_ptr(), recursive) })
}

pub fn remove_path(path: &str, recursive: bool) -> Result<bool> {
    let c_path = to_cstring(path)?;
    Ok(unsafe { sys::Moss_RemovePath(c_path.as_ptr(), recursive) })
}

pub fn rename_path(old_path: &str, new_path: &str, overwrite: bool) -> Result<bool> {
    let c_old = to_cstring(old_path)?;
    let c_new = to_cstring(new_path)?;
    Ok(unsafe { sys::Moss_RenamePath(c_old.as_ptr(), c_new.as_ptr(), overwrite) })
}

pub fn path_info(path: &str) -> Result<Option<PathInfo>> {
    let c_path = to_cstring(path)?;
    let mut info = sys::Moss_PathInfo::default();
    let ok = unsafe { sys::Moss_GetPathInfo(c_path.as_ptr(), &mut info) };
    if !ok {
        return Ok(None);
    }
    Ok(Some(PathInfo {
        kind: info.type_.into(),
        size: info.size,
        readable: info.readable,
        writable: info.writable,
        executable: info.executable,
    }))
}

pub fn current_directory() -> Option<PathBuf> {
    read_into_buffer(PATH_BUF_LEN, |buf, len| unsafe {
        sys::Moss_GetCurrentDirectory(buf, len)
    })
    .map(PathBuf::from)
}

pub fn base_path() -> Option<PathBuf> {
    read_into_buffer(PATH_BUF_LEN, |buf, len| unsafe {
        sys::Moss_GetBasePath(buf, len)
    })
    .map(PathBuf::from)
}

pub fn user_folder(folder: UserFolder) -> Option<PathBuf> {
    read_into_buffer(PATH_BUF_LEN, |buf, len| unsafe {
        sys::Moss_GetUserFolder(folder.into(), buf, len)
    })
    .map(PathBuf::from)
}

pub fn pref_path(org_name: &str, app_name: &str) -> Result<Option<PathBuf>> {
    let c_org = to_cstring(org_name)?;
    let c_app = to_cstring(app_name)?;
    Ok(read_into_buffer(PATH_BUF_LEN, |buf, len| unsafe {
        sys::Moss_GetPrefPath(c_org.as_ptr(), c_app.as_ptr(), buf, len)
    })
    .map(PathBuf::from))
}

/// Walks `path`, calling `visit` once per entry. Return `false` from
/// `visit` to stop early. Returns whatever the native call reports for
/// overall success.
pub fn enumerate_directory(
    path: &str,
    recursive: bool,
    mut visit: impl FnMut(&PathInfo, &str) -> bool,
) -> Result<bool> {
    unsafe extern "C" fn trampoline<F: FnMut(&PathInfo, &str) -> bool>(
        info: *const sys::Moss_PathInfo,
        path: *const c_char,
        user_data: *mut c_void,
    ) -> bool {
        let visit = &mut *(user_data as *mut F);
        let info_owned = PathInfo {
            kind: (*info).type_.into(),
            size: (*info).size,
            readable: (*info).readable,
            writable: (*info).writable,
            executable: (*info).executable,
        };
        let path_str = CStr::from_ptr(path).to_string_lossy();
        visit(&info_owned, path_str.as_ref())
    }

    let c_path = to_cstring(path)?;
    let user_data = &mut visit as *mut _ as *mut c_void;
    Ok(unsafe {
        sys::Moss_EnumerateDirectory(
            c_path.as_ptr(),
            recursive,
            Some(trampoline::<dyn FnMut(&PathInfo, &str) -> bool>),
            user_data,
        )
    })
}

// ===========================================================================
// File dialogs
// ===========================================================================

/// Result of a file dialog: the files chosen, if any (an empty selection
/// means the dialog was cancelled), and which filter was active.
pub struct DialogResult {
    pub paths: Vec<PathBuf>,
    pub active_filter: i32,
}

fn c_filters(
    filters: &[DialogFileFilter],
) -> Result<(Vec<CString>, Vec<CString>, Vec<sys::Moss_DialogFileFilter>)> {
    let mut names = Vec::with_capacity(filters.len());
    let mut patterns = Vec::with_capacity(filters.len());
    for f in filters {
        names.push(to_cstring(f.name)?);
        patterns.push(to_cstring(f.pattern)?);
    }
    let raw = names
        .iter()
        .zip(&patterns)
        .map(|(n, p)| sys::Moss_DialogFileFilter {
            name: n.as_ptr(),
            pattern: p.as_ptr(),
        })
        .collect();
    Ok((names, patterns, raw))
}

unsafe extern "C" fn dialog_trampoline<F: FnOnce(DialogResult)>(
    user_data: *mut c_void,
    filelist: *const *const c_char,
    filter: i32,
) {
    let callback = Box::from_raw(user_data as *mut F);
    let mut paths = Vec::new();
    if !filelist.is_null() {
        let mut i = 0;
        loop {
            let entry = *filelist.add(i);
            if entry.is_null() {
                break;
            }
            if let Some(s) = owned_string(entry) {
                paths.push(PathBuf::from(s));
            }
            i += 1;
        }
    }
    callback(DialogResult { paths, active_filter: filter });
}

/// Shows a native "open file" dialog. `on_result` is called once, from
/// wherever the platform's dialog implementation runs it (which may not be
/// this thread), with the chosen path(s).
pub fn show_open_file_dialog(
    window: Option<&Window>,
    default_location: Option<&str>,
    allow_many: bool,
    on_result: impl FnOnce(DialogResult) + 'static,
) -> Result<()> {
    let c_location = default_location.map(to_cstring).transpose()?;
    let location_ptr = c_location.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
    let window_ptr = window.map_or(std::ptr::null_mut(), |w| w.raw.as_ptr());
    let boxed: Box<dyn FnOnce(DialogResult)> = Box::new(on_result);
    unsafe {
        sys::Moss_ShowOpenFileDialog(
            Some(dialog_trampoline::<dyn FnOnce(DialogResult)>),
            Box::into_raw(Box::new(boxed)) as *mut c_void,
            window_ptr,
            location_ptr,
            allow_many,
        );
    }
    Ok(())
}

/// Shows a native "open folder" dialog.
pub fn show_open_folder_dialog(
    window: Option<&Window>,
    filters: &[DialogFileFilter],
    default_location: Option<&str>,
    allow_many: bool,
    on_result: impl FnOnce(DialogResult) + 'static,
) -> Result<()> {
    let (_names, _patterns, raw_filters) = c_filters(filters)?;
    let c_location = default_location.map(to_cstring).transpose()?;
    let location_ptr = c_location.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
    let window_ptr = window.map_or(std::ptr::null_mut(), |w| w.raw.as_ptr());
    let boxed: Box<dyn FnOnce(DialogResult)> = Box::new(on_result);
    unsafe {
        sys::Moss_ShowOpenFolderDialog(
            Some(dialog_trampoline::<dyn FnOnce(DialogResult)>),
            Box::into_raw(Box::new(boxed)) as *mut c_void,
            window_ptr,
            raw_filters.as_ptr(),
            raw_filters.len() as i32,
            location_ptr,
            allow_many,
        );
    }
    Ok(())
}

// `Moss_ShowSaveFileDialog` and `Moss_ShowFileDialogWithProperties` follow
// the same trampoline pattern as the two above but take a `Moss_PropertiesID`
// / `Moss_FileDialogType` this wrapper doesn't otherwise use yet — worth
// adding once `Moss_PropertiesID`'s creation API (not present in this
// header) is wrapped elsewhere.

// ===========================================================================
// Storage
// ===========================================================================

/// A storage container (`Moss_Storage`) — an abstraction over "a
/// filesystem-like place to read/write files" that may be the real
/// filesystem, an app's user-data directory, or a title/read-only bundle
/// depending on how it was opened.
pub struct Storage {
    raw: NonNull<sys::Moss_Storage>,
}

impl Storage {
    pub fn open_file_storage(path: &str) -> Result<Self> {
        let c_path = to_cstring(path)?;
        let raw = unsafe { sys::Moss_OpenFileStorage(c_path.as_ptr()) };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or(MossError::Unsupported)
    }

    pub fn open_user_storage(org: &str, app: &str, props: PropertiesId) -> Result<Self> {
        let c_org = to_cstring(org)?;
        let c_app = to_cstring(app)?;
        let raw = unsafe { sys::Moss_OpenUserStorage(c_org.as_ptr(), c_app.as_ptr(), props) };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or(MossError::Unsupported)
    }

    pub fn open_title_storage(override_path: Option<&str>, props: PropertiesId) -> Result<Self> {
        let c_override = override_path.map(to_cstring).transpose()?;
        let override_ptr = c_override.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        let raw = unsafe { sys::Moss_OpenTitleStorage(override_ptr, props) };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or(MossError::Unsupported)
    }

    pub fn is_ready(&self) -> bool {
        unsafe { sys::Moss_StorageReady(self.raw.as_ptr()) }
    }

    pub fn file_size(&self, path: &str) -> Result<Option<u64>> {
        let c_path = to_cstring(path)?;
        let mut len: u64 = 0;
        let ok =
            unsafe { sys::Moss_GetStorageFileSize(self.raw.as_ptr(), c_path.as_ptr(), &mut len) };
        Ok(ok.then_some(len))
    }

    pub fn space_remaining(&self) -> u64 {
        unsafe { sys::Moss_GetStorageSpaceRemaining(self.raw.as_ptr()) }
    }

    pub fn read_file(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let Some(len) = self.file_size(path)? else {
            return Ok(None);
        };
        let c_path = to_cstring(path)?;
        let mut buf = vec![0u8; len as usize];
        let ok = unsafe {
            sys::Moss_ReadStorageFile(
                self.raw.as_ptr(),
                c_path.as_ptr(),
                buf.as_mut_ptr() as *mut c_void,
                len,
            )
        };
        Ok(ok.then_some(buf))
    }

    pub fn write_file(&self, path: &str, data: &[u8]) -> Result<bool> {
        let c_path = to_cstring(path)?;
        Ok(unsafe {
            sys::Moss_WriteStorageFile(
                self.raw.as_ptr(),
                c_path.as_ptr(),
                data.as_ptr() as *const c_void,
                data.len() as u64,
            )
        })
    }

    pub fn create_directory(&self, path: &str) -> Result<bool> {
        let c_path = to_cstring(path)?;
        Ok(unsafe { sys::Moss_CreateStorageDirectory(self.raw.as_ptr(), c_path.as_ptr()) })
    }

    pub fn remove_path(&self, path: &str) -> Result<bool> {
        let c_path = to_cstring(path)?;
        Ok(unsafe { sys::Moss_RemoveStoragePath(self.raw.as_ptr(), c_path.as_ptr()) })
    }

    pub fn rename_path(&self, old_path: &str, new_path: &str) -> Result<bool> {
        let c_old = to_cstring(old_path)?;
        let c_new = to_cstring(new_path)?;
        Ok(unsafe { sys::Moss_RenameStoragePath(self.raw.as_ptr(), c_old.as_ptr(), c_new.as_ptr()) })
    }

    pub fn copy_file(&self, old_path: &str, new_path: &str) -> Result<bool> {
        let c_old = to_cstring(old_path)?;
        let c_new = to_cstring(new_path)?;
        Ok(unsafe { sys::Moss_CopyStorageFile(self.raw.as_ptr(), c_old.as_ptr(), c_new.as_ptr()) })
    }

    // `Moss_GlobStorageDirectory` returns a raw `char**` with no paired
    // "free this" function visible in this header — wrapping it safely
    // means first confirming (in Moss_stdinc.h or elsewhere) what's meant
    // to free that array, so it's left out rather than guessing and
    // potentially leaking or double-freeing.
}

impl Drop for Storage {
    fn drop(&mut self) {
        unsafe {
            sys::Moss_CloseStorage(self.raw.as_ptr());
        }
    }
}

// ===========================================================================
// Haptics — not yet wrapped, see module docs above.
// ===========================================================================

pub mod haptics {
    //! TODO: safe wrapper for `Moss_Haptic*`. Left unimplemented — see the
    //! note in the parent module's docs.
}

// ===========================================================================
// Camera capture — not yet wrapped, see module docs above.
// ===========================================================================

pub mod camera {
    //! TODO: safe wrapper for `Moss_Capture` / `Moss_*Camera*`. Left
    //! unimplemented — see the note in the parent module's docs.
}

// ===========================================================================
// GPU backend interop
// ===========================================================================

#[cfg(feature = "opengl")]
pub mod opengl {
    //! Thin OpenGL/OpenGL-ES interop, gated the same way the native header
    //! gates it (`MOSS_GRAPHICS_OPENGL` / `MOSS_GRAPHICS_OPENGLES`) — make
    //! sure the native Moss build was compiled with one of those defined,
    //! or these will fail to link.
    use super::Window;
    use moss_sys as sys;

    pub fn make_context_current(window: &Window) {
        unsafe { sys::Moss_MakeContextCurrent(window.as_raw()) }
    }

    pub fn swap_buffers() {
        unsafe { sys::Moss_SwapBuffers() }
    }

    pub fn swap_buffers_interval(interval: i32) {
        unsafe { sys::Moss_SwapBuffersInterval(interval) }
    }

    /// # Safety
    /// The returned pointer's validity/signature depends entirely on
    /// `procname`; the caller must know what they're asking for.
    pub unsafe fn get_proc_address(procname: &str) -> *mut std::os::raw::c_void {
        let c_name = std::ffi::CString::new(procname).expect("procname must not contain NUL");
        sys::Moss_GetProcAddress(c_name.as_ptr())
    }
}

#[cfg(feature = "vulkan")]
pub mod vulkan {
    //! Vulkan interop. Deliberately left at the raw-handle level rather
    //! than pulling a Vulkan crate's types in here — wire these into `ash`
    //! (or whichever Vulkan bindings you use) at the call site.
    use moss_sys as sys;

    pub fn vulkan_supported() -> bool {
        unsafe { sys::Moss_VulkanSupported() != 0 }
    }

    /// Instance extension names required to present via Moss windows.
    pub fn required_instance_extensions() -> Vec<String> {
        let mut count = 0u32;
        unsafe {
            let ptr = sys::Moss_GetRequiredInstanceExtensions(&mut count);
            if ptr.is_null() {
                return Vec::new();
            }
            (0..count as usize)
                .filter_map(|i| super::owned_string(*ptr.add(i)))
                .collect()
        }
    }

    // `Moss_CreateWindowSurface`, `Moss_GetInstanceProcAddress`, and
    // `Moss_GetPhysicalDevicePresentationSupport` need real `VkInstance` /
    // `VkPhysicalDevice` / `VkSurfaceKHR` types, which come from whichever
    // Vulkan crate you pick — left out here to avoid pinning that choice
    // for you.
}

#[cfg(feature = "metal")]
pub mod metal {
    //! Metal interop.
    use super::Window;
    use moss_sys as sys;
    use std::os::raw::c_void;

    pub struct MetalView {
        raw: sys::Moss_MetalView,
    }

    impl MetalView {
        pub fn new(window: &Window) -> Self {
            Self {
                raw: unsafe { sys::Moss_Metal_CreateView(window.as_raw()) },
            }
        }

        /// Returns the underlying `CAMetalLayer*` as a raw pointer; cast at
        /// the call site to whatever Metal binding you're using.
        pub fn layer(&self) -> *mut c_void {
            unsafe { sys::Moss_Metal_GetLayer(self.raw) }
        }

        pub fn resize(&self, width: u32, height: u32) {
            unsafe { sys::Moss_Metal_Resize(self.raw, width, height) }
        }
    }

    impl Drop for MetalView {
        fn drop(&mut self) {
            unsafe { sys::Moss_Metal_DestroyView(self.raw) }
        }
    }
}

// ===========================================================================
// From/Into conversions to the sys crate's enum types
// ===========================================================================

macro_rules! transmute_into {
    ($from:ty => $to:ty) => {
        impl From<$from> for $to {
            fn from(value: $from) -> Self {
                // SAFETY: `$from` is defined above with the same
                // discriminants, in the same order, as the native `$to`
                // enum — see the module-level doc note about the
                // `.rustified_enum(...)` assumption this relies on.
                unsafe { std::mem::transmute(value) }
            }
        }
    };
}

transmute_into!(Key => sys::Keyboard);
transmute_into!(MouseButton => sys::Mouse);
transmute_into!(GamepadAxis => sys::GamepadAxis);
transmute_into!(GamepadButton => sys::Moss_GamepadButton);
transmute_into!(UserFolder => sys::Moss_UserFolder);

impl From<sys::Moss_PowerState> for PowerState {
    fn from(value: sys::Moss_PowerState) -> Self { unsafe { std::mem::transmute(value) } }
}

impl From<sys::Moss_PathType> for PathType {
    fn from(value: sys::Moss_PathType) -> Self { unsafe { std::mem::transmute(value) } }
}

impl From<sys::Moss_PenDeviceType> for PenDeviceType {
    fn from(value: sys::Moss_PenDeviceType) -> Self { unsafe { std::mem::transmute(value) } }
}

impl From<sys::Moss_TouchDeviceType> for TouchDeviceType {
    fn from(value: sys::Moss_TouchDeviceType) -> Self { unsafe { std::mem::transmute(value) } }
}
