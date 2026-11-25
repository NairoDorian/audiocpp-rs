# audiocpp-sys

Raw FFI bindings to [audio.cpp](https://github.com/0xShug0/audio.cpp)'s C ABI (`include/audiocpp.h`, ABI 0.4, 77 functions), and the build that compiles the pinned audio.cpp release into one shared library with only the model families you enable (`model-<target>` features).

Use the safe [`audiocpp`](../audiocpp) crate unless you need the raw surface. The declarations are written by hand and proved against the pinned header by `cargo xtask check-abi`. Build details, features and environment variables: [`docs/building.md`](../docs/building.md).
