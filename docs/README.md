# MossRS architecture

## ABI boundary

The public Rust API must not depend on C++ ABI details.

The bridge uses:

```cpp
extern "C"
```

and only exposes:

- opaque pointers
- integer/boolean values
- floating point values
- fixed-size POD structures
- explicit result/error codes

## Ownership

C++ owns native engine resources. Rust owns the wrapper handles.

Example:

```text
Engine
  |
  +-- raw: NonNull<MossEngine>
```

`Drop` calls the matching bridge destructor.

## Error handling

C++ failures are converted to `MossResult` values. Rust converts these into `Result<T, MossError>`.

Exceptions must be caught inside the C++ bridge.

## Expansion order

Recommended implementation order:

1. Core engine lifecycle
2. Window/platform
3. Input
4. Math and transforms
5. Renderer
6. GPU resources
7. Assets
8. Physics
9. Audio
10. GUI
11. Navigation
12. Networking
13. XR/OpenXR
14. Rust-side ECS/gameplay conveniences

The native Moss implementation remains authoritative for the low-level systems.
