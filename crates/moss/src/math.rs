#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}


// `Quat` and `Mat44` aren't defined in any header seen so far (`Moss_XR.h`
// uses both but only `#include`s `Moss_Renderer.h`; they likely live in
// `Moss_stdinc.h` or a dedicated math header). Rather than guess their
// field order/layout by hand — getting a quaternion's `x,y,z,w` order or a
// matrix's row/column-major layout wrong is a silent-corruption bug, not a
// compile error — these alias whatever `moss-sys`'s bindgen actually
// generates from the real definition, so the layout is correct by
// construction. Swap these for hand-written `#[repr(C)]` types (matching
// `Vec2`/`Vec3`/`Color` above) once the source header is available, if a
// non-generated public type is preferred.
pub type Quat = moss_sys::Quat;
pub type Mat44 = moss_sys::Mat44;