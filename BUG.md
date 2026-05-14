# Bug: demo_playthrough fails at checkpoint 16 (tic 2950)

## Notes 

- When diagnosing, write to `.log` files in this repo. They are gitignored.
- Always extend this document with new areas to look into.
- Do not remove past investigations from this file, instead mark them as tried without success.

## Symptom

Rust produces `prndindex=32` at tic 2950; baseline expects `prndindex=33`.
One missing `P_Random()` call somewhere during gametic 2950's tick.

All 8 P_Random calls Rust makes at tic 2950 come from A_Chase context (confirmed via TRACE_PRND_SITES backtrace). C makes 9. The extra C call is also from A_Chase (no other call sites diverge).

Checkpoints 1–15 (tic 2900) all pass. Divergence is exactly at tic 2950.

## Key constants / code facts

Verified from instrumented run (checkpoint logging added to demo_playthrough.rs):

| checkpoint | tic (1-based) | gametic | leveltime (after tick) | lt during P_RunThinkers |
|---|---|---|---|---|
| 15 | 2900 | 2902 | 2731 | 2730 |
| 16 (FAILS) | 2950 | 2952 | 2781 | **2780** |

- `levelstarttic = gametic - leveltime = 2952 - 2781 = 171`
- tic=2950, lt=2780 is the exact failing thinker window
- TRACK/ACHASE instrumentation windows must cover `lt 2775–2785`
- Current instrumentation (p_mobj.rs, p_enemy.rs): windows set to `2775–2785` ✓

**Previous estimate of lt≈2749 was wrong.** The prior TRACK data at lt=2699 was from
ticks much earlier than tic 2900 (the window `lt < 2700` closed 30 leveltime-increments before
the checkpoint, so lt=2699 was NOT the lt of tic 2900's P_RunThinkers).

Actual lt at tic 2900 P_RunThinkers = 2730. At tic 2950 P_RunThinkers = **2780**.

## Fixed (not the root cause)

### BT_SPECIALMASK: `0x7f` → `3`
- File: `g_game.rs` line 1176
- C: `switch (cmd.buttons & BT_SPECIALMASK)` where `BT_SPECIALMASK = 3`
- Rust had: `match cmd_buttons & 0x7f`
- Fixed to: `match cmd_buttons & 3`
- Verified in `d_event.h`: `BT_SPECIAL=128, BT_SPECIALMASK=3, BTS_PAUSE=1, BTS_SAVEGAME=2`
- **Test still fails after fix.** Not the root cause.

## Dead ends (ruled out)

| Candidate | Why ruled out |
|-----------|--------------|
| Sector special P_Random (p_spec.rs:826) | `leveltime & 31 ≠ 0` at lt≈2749 |
| T_FireFlicker periodic fire | Calculated fires don't land at lt≈2749 |
| T_PlatRaise | No P_Random during ticking (only at creation) |
| G_DeathMatchSpawnPlayer | `deathmatch=0` in shareware demo |
| longtics byte order in G_ReadDemoTiccmd | Shareware demo uses `longtics=0` |
| BT_CHANGE weapon handling | Ported before this commit, not in diff |
| G_DoLoadLevel / G_InitNew / G_PlayerReborn | Look identical to C |
| P_RunThinkers sentinel handling | Functionally equivalent to C |
| P_Ticker order | Identical to C |
| prndindex delta mismatch before tic 2950 | TRACE_PRND confirms tics 2901–2949 match |

## What we know about the divergence

- Per-tic TRACE_PRND: deltas match from tic 2901 to 2949. Divergence is **only** at tic 2950 (lt=2780).
- Rust tic 2950 delta = **8** P_Random calls. Baseline expects **9**.
- All 8 Rust calls at lt=2780 come from A_Chase (TRACE_PRND_SITES backtraces confirmed).
- Player state at checkpoint 15 (tic 2900) matches: `health=6, armorpoints=92, killcount=36, ammo=[69,17,0,0]`.

### Exact P_Random breakdown at lt=2780 (Rust, 8 calls total)

From TRACE_PRND_SITES + NEWCHASE + ACHASE combined run:

| # | type | monster | NEWCHASE? | P_Random source | count |
|---|------|---------|-----------|-----------------|-------|
| 1 | 1 (Zombie) | movecount=8, P_Move OK | no | activesound only | 1 |
| 2 | 12 (Demon) | movecount=0 → NEWCHASE | yes (diagonal P_TryWalk) | P_TryWalk + activesound | 2 |
| 3 | 1 (Zombie) | movecount=8, P_Move OK | no | activesound only | 1 |
| 4 | 11 (Imp) | movecount=10, P_Move fails | yes (path: line416 + line445 + P_TryWalk) | 3 NewChase + activesound | 4 |
| **total** | | | | | **8** |

### Monsters with tics=1 at lt=2780 that do NOT complete A_Chase (early exit)

From combined TRACK+ACHASE run (same run, matched by address):

| addr suffix | type | state | reason (inferred) |
|---|---|---|---|
| `...cbe8` | 12 (Demon) | 475 | no-target or melee range (no missilestate) |
| `...e088` | 1 (Zombie) | 174 | no-target or missile range |
| `...1e68` | 2 (Shotgunner) | 207 | no-target or missile range |
| `...2078` | 2 (Shotgunner) | 207 | no-target or missile range |
| `...2180` | 2 (Shotgunner) | 207 | no-target or missile range |

None of these had a NEWCHASE entry → they do NOT exit via JUSTATTACKED → they contribute
**0 P_Random each**. In C, if any one of these does NOT exit early, it adds 1 activesound
P_Random = total 9.

**Root cause is one of these 5 monsters completing A_Chase in C but not in Rust.**

## Root cause hypothesis (refined)

One of the 5 early-exit monsters (listed above) exits A_Chase early in Rust but NOT in C.
Early exit paths and whether they call P_Random:

| exit path | P_Random? | detectable via |
|---|---|---|
| no target / dead target | no | NEWCHASE absent ✓ |
| JUSTATTACKED → P_NewChaseDir | **yes** (1–3 calls) | NEWCHASE logged — NONE seen for these 5 |
| melee range → meleestate | no | |
| missile range → missilestate | no | |

Since no NEWCHASE for these 5, the exit is via no-target, melee, or missile. In C if any
one reaches activesound instead → +1 P_Random.

The Demon (`...cbe8`, type=12, state=475) is the most distinctive candidate: Demons have no
missilestate in Doom, so it exits only via no-target or melee. If in C it has a target and
is not in melee range, it continues to activesound.

The 3 Shotgunners (`...1e68/2078/2180`, type=2) are interesting together: 3 shotgunners
all exiting early suggests they all fired last tic (JUSTATTACKED cleared their flag this tic,
then exit via missile range or no-target). But no NEWCHASE for them → not JUSTATTACKED exit.
Could be missile range (all 3 are in missile range in Rust but not in C?).

**Most parsimonious**: one monster has a different target pointer or JUSTATTACKED flag state
in Rust vs C due to some earlier g_game.rs ordering bug.

## Root cause mechanism (current hypothesis: Scenario B — A_Look wake-up)

Per-tic P_Random counts match from tic 2901 to 2949 (50 tics). This means the mystery monster was
NOT already in A_Chase during that window — otherwise it would contribute P_Random calls every tic
and cause earlier divergence. Therefore:

**Mystery monster is still in STND state (A_Look) for all of tics 2901–2949, then wakes up (A_Look
fires A_Chase via P_SetMobjState(seestate)) at lt=2780 in C but NOT in Rust.**

### Confirmed: 5 STND monsters at lt=2780

From lt278x_full.log, the 5 monsters at lt=2780 with STND states and action=A_Look (addr=bca0):

| addr suffix | type | state | tics |
|---|---|---|---|
| `...cbe8` | 12 (Demon) | 475 (S_SARG_STND) | 1 |
| `...e088` | 1 (Zombie) | 174 (S_POSS_STND) | 1 |
| `...1e68` | 2 (Shotgunner) | 207 (S_SPOS_STND) | 1 |
| `...2078` | 2 (Shotgunner) | 207 (S_SPOS_STND) | 1 |
| `...2180` | 2 (Shotgunner) | 207 (S_SPOS_STND) | 1 |

Demon(dbe8) confirmed trajectory: tics=6 at lt=2775 → tics=1 at lt=2780 → fires A_Look.

The Demon(a648) in chase state (S_SARG_RUN, tics=2 at lt=2780) is ruled out: if it caused
divergence, we'd see per-tic count mismatch every 2 tics during 2901–2949 (it fires A_Chase every
2 tics alternating with tics>1), which we don't.

### A_Look wake-up mechanism

When A_Look sees player, it calls `P_SetMobjState(actor, (*info).seestate)` which immediately
invokes A_Chase (since P_SetMobjState calls the state's action directly). This adds activesound
P_Random call in the same tic.

Candidates for why C wakes up but Rust doesn't:

1. **MF_AMBUSH monsters + P_CheckSight difference** — if monster has MF_AMBUSH set, A_Look requires
   P_CheckSight to pass even when soundtarget is shootable. If player position differs by even 1
   unit (accumulated over 2950 tics of movement from any early divergence), P_CheckSight could flip.

2. **lastlook = 0 vs 1 at spawn** — P_SpawnMobj sets `lastlook = P_Random() % MAXPLAYERS`. If Rust
   spawns monster with lastlook=1, P_LookForPlayers iterates [1,2,3] and misses player index 0
   (the only ingame player). If C spawns it with lastlook=0, it finds player 0 immediately. This
   is purely RNG-dependent at spawn time, but the prndindex was identical up to this point so
   spawn-time values should match.

3. **soundtarget null in Rust** — if P_NoiseAlert wasn't called (or was called differently) in Rust,
   soundtarget could be null, forcing through the slow P_LookForPlayers path for non-ambush
   monsters too.

### A_Look instrumentation added

New ALOOK log in p_enemy.rs logs for lt=2775–2785:
- Entry: type, addr, flags, soundtarget ptr, tgt_shootable, ambush flag
- "no-see": when P_LookForPlayers returns false (no wake)
- "WOKE UP": when monster wakes up → A_Chase fires same tic

## BSP sight failure chains (from BSPNODE + SIGHT logs, bspend_debug.log)

All 5 STND monsters fail P_CheckSight in Rust at lt=2780 due to null_back walls:

| addr suffix | type | monster pos | blocked at ss | wall |
|---|---|---|---|---|
| `...cbe8` | 12 (Demon) | (1344,1280) | ss=349 | x=960, y=[1216,1280] (vertical) |
| `...e088` | 1 (Zombie) | (384,2048) | ss=297 | x=304–320, y=2000–2016 |
| `...1e68` | 2 (Shotgunner) | (992,992) | ss=349 | x=960, y=[960,1088] (vertical) |
| `...2078` | 2 (Shotgunner) | (1088,1328) | ss=348 | x=1024, y=[1280,1344] (vertical) |
| `...2180` | 2 (Shotgunner) | (1648,1088) | ss=338 | x=1536, y=[928,1336] (vertical) |

All blocking walls at x=960, 1024, 1536: player at x≈-1658, always left of all walls; monsters always right. Player x-position shift cannot flip any of these null_back checks.

## P_RecursiveSound dead end

P_NoiseAlert fires from sector 9 at lt=2776. RSOUND trace shows propagation reaches sectors 0–48 only. Sector 106 (Zombie's sector) never gets soundtarget. Blocked because sector 10 has `floor=-184, ceil=-184` (degenerate sector) → openrange ≤ 0 → RSOUND_STOP. Even if soundtarget were set, null_back walls still block sight. **Dead end: soundtarget absence explains why some monsters stay idle but does not explain the C/Rust divergence directly.**

## Current root cause hypothesis: small player position divergence (g_game.rs bug)

Player position accumulates over 2950 tics. A tiny off-by-one or wrong arithmetic in g_game.rs changes player (x,y) by 1–few fixed-point units. At lt=2780, BSP node 343 executes the "same side" check:

```
if side == P_DivlineSide(t2x, t2y, &bsp_div2) { return true; }  // skip second subtree → sight OK
```

In C: player position causes `P_DivlineSide` to return `side` → condition true → skip null_back subtree → **sight succeeds** → Demon wakes → A_Chase → activesound = +1 P_Random.

In Rust: player position causes `P_DivlineSide` to return `side^1` → condition false → traverses null_back subtree → sight blocked → Demon stays idle.

The node 343 second-call check is the single point where a 1-unit player position change could flip the sight result. This is confirmed by BSPNODE_END data showing Rust fails at that exact subtree.

**Next**: find the g_game.rs code responsible for player position accumulation divergence. Candidates:
- `G_ReadDemoTiccmd` angleturn field — verified CORRECT (both C and Rust produce same i16)
- `P_Thrust` application — check if forwardmove/sidemove applied identically
- Any momentum/friction formula differences

## Next steps

1. **~~Add A_Chase exit-reason logging~~** — superseded by Scenario B discovery (early-exit
   monsters are in STND/A_Look state, not A_Chase). Ruled out.

2. **~~Run ALOOK test~~** — done. alook_debug.log shows no WOKE UP for the 5 STND monsters. ✓

3. **~~Check ALOOK WOKE UP at lt=2780~~** — none in Rust → A_Look doesn't wake mystery monster.
   Root cause is P_CheckSight returning false (null_back walls). See BSP section above.

4. **~~Check P_LookForPlayers~~** — lastlook handled same as C. Dead end.

5. **~~Check A_Chase missile-range guard~~** — ruled out (Scenario B: monsters in STND state). ✓

6. **~~Extend TRACK window~~** — done (lt 2775–2785). ✓
7. **~~Extend ACHASE/NEWCHASE logging~~** — done (all types, lt 2775–2785). ✓
8. **~~Print leveltime/gametic at checkpoints~~** — done; confirmed lt=2780 at failing tic. ✓
9. **~~Confirm 5 STND monsters at lt=2780 as A_Look candidates~~** — done. ✓

10. **Find g_game.rs player position divergence bug** — in progress. Check P_Thrust, friction,
    any momentum update path that differs from C. BT_SPECIALMASK already ruled out.

## Commits verified

| Commit | What was checked |
|--------|-----------------|
| `77ca9ea` | First failing commit — "Port g_game.c to Rust, 2266 LoC". BT_SPECIALMASK bug found here. |
| Current HEAD | BT_SPECIALMASK fixed (`0x7f→3`), test still fails. |

## Instrumentation currently in tree

| File | What it does |
|------|-------------|
| `p_mobj.rs` | TRACK: all MF_COUNTKILL alive monsters, window `lt 2775–2785` |
| `p_mobj.rs` | P_MobjThinker header log when PRND_TRACE active |
| `p_enemy.rs` | ACHASE: all types, `lt 2775–2785` (logs when monster reaches activesound) |
| `p_enemy.rs` | NEWCHASE: all types, `lt 2775–2785` (logs P_NewChaseDir entry) |
| `m_random.rs` | `set_prnd_trace()` / `PRND_TRACE` atomic flag |
| `tests/demo_playthrough.rs` | TRACE_PRND: per-tic prndindex delta; TRACE_PRND_SITES: per-call backtrace at tic 2950 |
| `tests/demo_playthrough.rs` | checkpoint log includes `gametic` and `leveltime` |

Log files in repo root (gitignored):
- `debug_tic2950.log` — first TRACK run (stale window, ignore)
- `lt2780_track.log` — TRACK at lt=2780 only
- `lt2780_combined.log` — TRACK+ACHASE+NEWCHASE at lt=2780, same run
- `tic2950_trace.log` — TRACE_PRND_SITES output for tic 2950 (807 lines)
- `lt278x_full.log` — ENTER/EXIT/ACHASE/NEWCHASE/TRACK for lt=2775–2785 (608 lines)
- `alook_debug.log` — A_Look instrumentation output (535 KB); no WOKE UP for 5 STND monsters
- `sight_raw3.log` — BSP failure chains showing null_back walls (592 KB)
- `bspend_debug.log` — BSPNODE_END + PLAYER positions at lt=2775–2785
- `rsound_debug.log` — P_RecursiveSound sector propagation trace
- `rsound3_debug.log` — RSOUND_STOP with floor/ceil heights; shows sector 10 blocks propagation
