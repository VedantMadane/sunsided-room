# Porting Progress

All engine modules have been ported to Rust. This document is retained as a
historical record of the porting effort and a reference for follow-up work.

> A module is considered "ported" when the `.c` file is removed from
> `doomgeneric-sys/build.rs` and a Rust replacement exists in the `room`
> crate.  The checklist in [README.md](README.md#porting-progress) is the
> source of truth.

A transpiled reference Rust code is available in `c2rust-intermediate/`. Use it to verify assumptions in addition to the original C code. The transpiled code is not part of the build.

All ported code has been validated against the unit tests and the `demo_playthrough` integration test.

## Summary

| Metric | Value |
|--------|------:|
| Remaining C modules | 0 |
| Total remaining LoC | 0 |
| Already ported LoC | ~55,658 (est.) |
| Port completeness | **100%** |

All C modules have been ported to Rust. The remaining C code in `doomgeneric-sys/` consists of the platform-agnostic `doomgeneric` frontend (`doomgeneric.c`) and a few small system-level stubs (`i_timer.c`, `i_video.c`) that are intentionally left as C glue between the engine and the operating system.

## Porting History

The modules below were ported in roughly this order. All are now complete.

1. ~~**Finish the renderer** — `r_things.c` is now ported. The renderer pipeline
   (`r_data`, `r_draw`, `r_segs`, `r_main`, `r_plane`, `r_bsp`, `r_sky`,
   `r_things`) is fully Rust-native.~~
2. ~~**Map objects** — `p_mobj.c`. Needed by `p_enemy.c`, `g_game.c`, and
   `p_map.c`. Once ported, the thinker list becomes fully Rust-native.~~
3. ~~**Map utilities** — `p_maputl.c`. Building block for `p_map.c`; many of
   its types (`divline_t`, `intercept_t`, `mobj_t`) are already mirrored in
   `c_ffi.rs`.~~
4. ~~**Collision detection** — `p_map.c`. Required by `p_enemy.c` and
   `g_game.c`. Heavy geometry code, but its utility layer (`p_maputl.c`)
   should be done first.~~
5. ~~**Special actions** — `p_spec.c` is now ported. The sector/line
   special dispatcher (`p_spec.rs`) is fully Rust-native.~~
6. ~~**Enemy AI** — `p_enemy.c`. Complex state machines, but all dependencies
   (`p_mobj`, `p_map`, `p_maputl`, `p_spec`) should be in place by this
   point.~~
7. ~~**UI / display modules** — `st_stuff.c`, `i_scale.c`, and `wi_stuff.c`
   are now ported.~~
8. ~~**Save/load** — `p_saveg.c`. Save/load game serialization with heavy struct
   layout work. Now fully ported to Rust (`room/src/doom/p_saveg.rs`).~~
9. ~~**Main orchestrator** — `d_main.c`. The main initialization and game loop
   entry point. Now fully ported to Rust (`room/src/doom/d_main.rs`).~~
10. ~~**Game logic last** — `g_game.c`. The last remaining C module. Now fully
    ported to Rust (`room/src/doom/g_game.rs`).~~

## Follow-up Work

Now that every engine module is in Rust, the next priorities are:

1. **Remove stale C glue**
   - Drop the `M_snprintf` → `M_snprintf_clamp` macro from `m_misc.h`; no
     engine module needs it any more.
   - Audit `c_ffi.rs` for declarations that are now unused and delete them.
2. **Incremental idiomatic Rustification**
   - Replace raw-pointer-heavy code (especially in `g_game.rs` and `d_main.rs`)
     with safe Rust where the call graph allows it.
   - Convert `#[no_mangle] pub unsafe extern "C"` functions to normal Rust
     signatures once every caller has been ported.
3. **Clean up the build**
   - Decide whether to keep `c2rust-intermediate/` as a reference or archive it.
   - Remove obsolete comments in `doomgeneric-sys/build.rs` that refer to
     already-ported modules.
4. **Safety & testing**
   - Run the full test suite under AddressSanitizer (`task asan:test`).
   - Add unit tests for any logic in `g_game.rs` that lacks coverage.

## Porting Strategy Notes

- **Variadic functions**: Stable Rust cannot define C variadic functions. All
  engine modules that previously relied on `M_snprintf` have been ported to
  Rust, where `M_snprintf_clamp` (a non-variadic Rust helper) is used
  instead. The macro redirection in `m_misc.h` can now be removed if the
  remaining C glue no longer needs it.
- **`d_net.c` is already ported**: Since `FEATURE_MULTIPLAYER` is not
  defined, the original module contained only stubs; the Rust replacement
  (`room/src/doom/d_net.rs`) is already active.
- **`z_zone.c` is ported**: The zone memory allocator is now Rust-native
  (`room/src/doom/z_zone.rs`). All ported modules now rely on safe allocation
  and automatic zeroing.
- **Renderer inner loops**: `r_draw.c` and `r_things.c` were ported with
  explicit wrapping arithmetic to match C overflow semantics and preserve
  demo determinism.

(End of file - total 104 lines)
