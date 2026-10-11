use std::ptr::NonNull;

use moss_sys as sys;

use crate::{error::Result, MossError};

pub struct Engine {
    raw: NonNull<sys::MossEngine>,
}

impl Engine {
    pub fn new() -> Result<Self> {
        let raw = unsafe { sys::moss_engine_create() };
        let raw = NonNull::new(raw).ok_or(MossError::OutOfMemory)?;

        let engine = Self { raw };
        if let Err(err) = engine.initialize() {
            return Err(err);
        }

        Ok(engine)
    }

    pub fn initialize(&self) -> Result<()> {
        let code = unsafe { sys::moss_engine_initialize(self.raw.as_ptr()) };
        MossError::from_code(code)
    }

    pub fn update(&mut self, delta_time: f64) -> Result<()> {
        let code = unsafe {
            sys::moss_engine_update(self.raw.as_ptr(), delta_time)
        };
        MossError::from_code(code)
    }

    pub fn render(&mut self) -> Result<()> {
        let code = unsafe { sys::moss_engine_render(self.raw.as_ptr()) };
        MossError::from_code(code)
    }

    pub fn as_raw(&self) -> *mut sys::MossEngine {
        self.raw.as_ptr()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            sys::moss_engine_shutdown(self.raw.as_ptr());
            sys::moss_engine_destroy(self.raw.as_ptr());
        }
    }
}

// Moss engine objects are deliberately not Send/Sync until the native engine
// explicitly guarantees those semantics.
