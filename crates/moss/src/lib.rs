#![doc = "Safe and idiomatic Rust bindings for the Moss C++ engine."]

pub mod engine;
pub mod error;
pub mod math;
pub mod platform;

#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "gpu")]
pub mod gpu;
#[cfg(feature = "gui")]
pub mod gui;
#[cfg(feature = "networking")]
pub mod networking;
#[cfg(feature = "physics")]
pub mod physics;
#[cfg(feature = "rendering")]
pub mod rendering;
#[cfg(feature = "xr")]
pub mod xr;


pub use engine::Engine;
pub use error::{MossError, Result};
pub use math::{Color, Vec2, Vec3};

pub mod prelude {
    pub use crate::{
        engine::Engine,
        error::{MossError, Result},
        math::{Color, Vec2, Vec3},
    };
}
