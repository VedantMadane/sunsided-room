//! Rust port of vendor/doomgeneric/g_game.c.
//!
//! Core game logic: game state, input handling, demo recording/playback,
//! save/load, level transitions, player respawn, and the main game ticker.

#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    static_mut_refs,
    clippy::missing_safety_doc,
    clippy::needless_range_loop,
    clippy::manual_c_str_literals,
    clippy::comparison_chain
)]

use std::ffi::{c_char, c_int, c_long, c_short, c_uint, c_void};
use std::ptr::{self, null_mut};

use crate::doom::c_ffi::{
    ANG45, ANGLETOFINESHIFT, BODYQUESIZE, FINEANGLES, FRACBITS, FRACUNIT, SLOWTURNTICS, TICRATE,
    TURBOTHRESHOLD,
};
use crate::doom::info::{
    mobjinfo, states, MT_BRUISERSHOT, MT_HEADSHOT, MT_TROOPSHOT, S_SARG_PAIN2, S_SARG_RUN1,
};

const ev_keydown: c_int = 0;
const ev_keyup: c_int = 1;
const ev_mouse: c_int = 2;
const ev_joystick: c_int = 3;

use crate::doom::d_event::event_t;
use crate::doom::d_loop::{gametic, ticdup, BACKUPTICS};
use crate::doom::d_mode;
use crate::doom::d_player::{PlayerT, TiccmdT, MAXPLAYERS, NUMAMMO};
use crate::doom::doomstat::{gamemission, gamemode, gameversion};
use crate::doom::m_controls::{
    dclick_use, joybfire, joybnextweapon, joybprevweapon, joybspeed, joybstrafe, joybstrafeleft,
    joybstraferight, joybuse, key_down, key_fire, key_left, key_nextweapon, key_pause,
    key_prevweapon, key_right, key_speed, key_spy, key_strafe, key_strafeleft, key_straferight,
    key_up, key_use, key_weapon1, key_weapon2, key_weapon3, key_weapon4, key_weapon5, key_weapon6,
    key_weapon7, key_weapon8, mousebbackward, mousebfire, mousebforward, mousebnextweapon,
    mousebprevweapon, mousebstrafe, mousebstrafeleft, mousebstraferight, mousebuse,
};
use crate::doom::p_saveg::{save_stream, savegame_error};
use crate::doom::wi_stuff::wbstartstruct_t;
use crate::doom::z_zone::Z_Malloc;

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

type gamestate_t = c_int;
type skill_t = c_int;
type boolean = c_int;
type byte = u8;
type fixed_t = c_int;
type weapontype_t = c_int;

// Game state values
const GS_LEVEL: gamestate_t = 0;
const GS_INTERMISSION: gamestate_t = 1;
const GS_FINALE: gamestate_t = 2;
const GS_DEMOSCREEN: gamestate_t = 3;

// Game action values
pub const ga_nothing: gameaction_t = 0;
pub const ga_loadlevel: gameaction_t = 1;
pub const ga_newgame: gameaction_t = 2;
pub const ga_loadgame: gameaction_t = 3;
pub const ga_savegame: gameaction_t = 4;
pub const ga_playdemo: gameaction_t = 5;
pub const ga_completed: gameaction_t = 6;
pub const ga_victory: gameaction_t = 7;
pub const ga_worlddone: gameaction_t = 8;
pub const ga_screenshot: gameaction_t = 9;

type gameaction_t = c_int;

// Skill values
const sk_baby: skill_t = 0;
const sk_easy: skill_t = 1;
const sk_medium: skill_t = 2;
const sk_hard: skill_t = 3;
const sk_nightmare: skill_t = 4;

// Weapon values
const wp_fist: weapontype_t = 0;
const wp_pistol: weapontype_t = 1;
const wp_shotgun: weapontype_t = 2;
const wp_chaingun: weapontype_t = 3;
const wp_missile: weapontype_t = 4;
const wp_plasma: weapontype_t = 5;
const wp_bfg: weapontype_t = 6;
const wp_chainsaw: weapontype_t = 7;
const wp_supershotgun: weapontype_t = 8;
const wp_nochange: weapontype_t = 10;

// Powers
const pw_strength: c_int = 1;

// Zone memory tags
const PU_STATIC: c_int = 1;

// Button masks
const BT_ATTACK: byte = 0x01;
const BT_USE: byte = 0x02;
const BT_CHANGE: byte = 0x04;
const BT_WEAPONSHIFT: c_int = 3;
const BT_SPECIAL: byte = 0x80;
const BTS_PAUSE: byte = 0x01;
const BTS_SAVEGAME: byte = 0x02;
const BTS_SAVESHIFT: c_int = 2;

// Demo
const DEMOMARKER: byte = 0x80;
const SAVEGAMESIZE: c_int = 0x2c000;
const MAX_JOY_BUTTONS: usize = 20;
const NUMKEYS: usize = 256;
const MAXPLMOVE: c_int = 0x32;

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

#[no_mangle]
pub static mut oldgamestate: gamestate_t = GS_LEVEL;

#[no_mangle]
pub static mut gameaction: gameaction_t = 0;

#[no_mangle]
pub static mut gamestate: gamestate_t = GS_LEVEL;

#[no_mangle]
pub static mut gameskill: skill_t = 0;

#[no_mangle]
pub static mut respawnmonsters: boolean = 0;

#[no_mangle]
pub static mut gameepisode: c_int = 0;

#[no_mangle]
pub static mut gamemap: c_int = 0;

#[no_mangle]
pub static mut timelimit: c_int = 0;

#[no_mangle]
pub static mut paused: boolean = 0;

#[no_mangle]
pub static mut sendpause: boolean = 0;

#[no_mangle]
pub static mut sendsave: boolean = 0;

#[no_mangle]
pub static mut usergame: boolean = 0;

#[no_mangle]
pub static mut timingdemo: boolean = 0;

#[no_mangle]
pub static mut nodrawers: boolean = 0;

#[no_mangle]
pub static mut starttime: c_int = 0;

#[no_mangle]
pub static mut viewactive: boolean = 0;

#[no_mangle]
pub static mut deathmatch: c_int = 0;

#[no_mangle]
pub static mut netgame: boolean = 0;

#[no_mangle]
pub static mut playeringame: [c_int; MAXPLAYERS] = [0; MAXPLAYERS];

#[no_mangle]
pub static mut players: [PlayerT; MAXPLAYERS] = unsafe { std::mem::zeroed() };

#[no_mangle]
pub static mut turbodetected: [c_int; MAXPLAYERS] = [0; MAXPLAYERS];

#[no_mangle]
pub static mut consoleplayer: c_int = 0;

#[no_mangle]
pub static mut displayplayer: c_int = 0;

#[no_mangle]
pub static mut levelstarttic: c_int = 0;

#[no_mangle]
pub static mut totalkills: c_int = 0;

#[no_mangle]
pub static mut totalitems: c_int = 0;

#[no_mangle]
pub static mut totalsecret: c_int = 0;

// Demo globals
#[no_mangle]
pub static mut demoname: *mut c_char = null_mut();

#[no_mangle]
pub static mut demorecording: boolean = 0;

#[no_mangle]
pub static mut longtics: boolean = 0;

#[no_mangle]
pub static mut lowres_turn: boolean = 0;

#[no_mangle]
pub static mut demoplayback: boolean = 0;

#[no_mangle]
pub static mut netdemo: boolean = 0;

#[no_mangle]
pub static mut demobuffer: *mut byte = null_mut();

#[no_mangle]
pub static mut demo_p: *mut byte = null_mut();

#[no_mangle]
pub static mut demoend: *mut byte = null_mut();

#[no_mangle]
pub static mut singledemo: boolean = 0;

#[no_mangle]
pub static mut defdemoname: *mut c_char = null_mut();

#[no_mangle]
pub static mut precache: c_int = 1;

#[no_mangle]
pub static mut testcontrols: boolean = 0;

#[no_mangle]
pub static mut testcontrols_mousespeed: c_int = 0;

// Intermission
#[no_mangle]
pub static mut wminfo: wbstartstruct_t = unsafe { std::mem::zeroed() };

// Consistency
#[no_mangle]
pub static mut consistancy: [[c_int; BACKUPTICS]; MAXPLAYERS] = [[0; BACKUPTICS]; MAXPLAYERS];

// Save/load
#[no_mangle]
pub static mut savename: [c_char; 256] = [0; 256];

// Par times
#[no_mangle]
pub static mut pars: [[c_int; 10]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 30, 75, 120, 90, 165, 180, 180, 30, 165],
    [0, 90, 90, 90, 120, 90, 360, 240, 30, 170],
    [0, 90, 45, 90, 150, 90, 90, 165, 30, 135],
];

#[no_mangle]
pub static mut cpars: [c_int; 32] = [
    30, 90, 120, 120, 90, 150, 120, 120, 270, 90, 210, 150, 150, 150, 210, 150, 420, 150, 210, 150,
    240, 150, 180, 150, 150, 300, 330, 420, 300, 180, 120, 30,
];

// Movement tables
#[no_mangle]
pub static mut forwardmove: [fixed_t; 2] = [0x19, 0x32];

#[no_mangle]
pub static mut sidemove: [fixed_t; 2] = [0x18, 0x28];

#[no_mangle]
pub static mut angleturn: [fixed_t; 3] = [640, 1280, 320];

// Body queue
#[no_mangle]
pub static mut bodyque: [*mut c_void; BODYQUESIZE] = [null_mut(); BODYQUESIZE];

#[no_mangle]
pub static mut bodyqueslot: c_int = 0;

#[no_mangle]
pub static mut vanilla_savegame_limit: c_int = 1;

#[no_mangle]
pub static mut vanilla_demo_limit: c_int = 1;

// Secret exit flag
#[no_mangle]
pub static mut secretexit: boolean = 0;

// ---------------------------------------------------------------------------
// Private statics
// ---------------------------------------------------------------------------

// Weapon key array indices — these match the key_weapon1..8 variable indices
fn weapon_key_ptr(i: usize) -> *mut c_int {
    unsafe {
        match i {
            0 => &raw mut key_weapon1,
            1 => &raw mut key_weapon2,
            2 => &raw mut key_weapon3,
            3 => &raw mut key_weapon4,
            4 => &raw mut key_weapon5,
            5 => &raw mut key_weapon6,
            6 => &raw mut key_weapon7,
            7 => &raw mut key_weapon8,
            _ => unreachable!(),
        }
    }
}

static mut next_weapon: c_int = 0;

const WEAPON_ORDER: [(weapontype_t, weapontype_t); 9] = [
    (wp_fist, wp_fist),
    (wp_chainsaw, wp_fist),
    (wp_pistol, wp_pistol),
    (wp_shotgun, wp_shotgun),
    (wp_supershotgun, wp_shotgun),
    (wp_chaingun, wp_chaingun),
    (wp_missile, wp_missile),
    (wp_plasma, wp_plasma),
    (wp_bfg, wp_bfg),
];

static mut gamekeydown: [boolean; NUMKEYS] = [0; NUMKEYS];
static mut turnheld: c_int = 0;

const MAX_MOUSE_BUTTONS: usize = 8;
static mut mousearray: [boolean; MAX_MOUSE_BUTTONS + 1] = [0; MAX_MOUSE_BUTTONS + 1];

#[no_mangle]
pub static mut mousex: c_int = 0;

#[no_mangle]
pub static mut mousey: c_int = 0;

static mut dclicktime: c_int = 0;
static mut dclickstate: boolean = 0;
static mut dclicks: c_int = 0;
static mut dclicktime2: c_int = 0;
static mut dclickstate2: boolean = 0;
static mut dclicks2: c_int = 0;

static mut joyxmove: c_int = 0;
static mut joyymove: c_int = 0;
static mut joystrafemove: c_int = 0;
static mut joyarray: [boolean; MAX_JOY_BUTTONS + 1] = [0; MAX_JOY_BUTTONS + 1];

static mut savegameslot: c_int = 0;
static mut savedescription: [c_char; 32] = [0; 32];

static mut d_skill: skill_t = 0;
static mut d_episode: c_int = 0;
static mut d_map: c_int = 0;

// ---------------------------------------------------------------------------
// Extern declarations
// ---------------------------------------------------------------------------

extern "C" {
    static mut respawnparm: c_int;
    static mut fastparm: c_int;
    static mut nomonsters: c_int;

    static mut skyflatnum: c_int;
    static mut skytexture: c_int;
    static mut wipegamestate: gamestate_t;

    static mut mouseSensitivity: c_int;
    static mut automapactive: boolean;
    static mut menuactive: boolean;
    static mut setsizeneeded: boolean;

    static mut deathmatchstarts: [*mut c_void; 10];
    static mut deathmatch_p: *mut c_void;
    static mut playerstarts: [*mut c_void; 4];

    static mut maxammo: [c_int; NUMAMMO];

    static mut rndindex: c_int;
    static mut netcmds: *mut TiccmdT;

    static mut myargv: *mut *mut c_char;

    static mut finesine: [fixed_t; FINEANGLES];
    static mut finecosine: *const fixed_t;
    static mut finetangent: [fixed_t; FINEANGLES / 2];

    static mut player_names: [*mut c_char; 4];
    static mut leveltime: c_int;

    fn R_FlatNumForName(name: *const c_char) -> c_int;
    fn R_TextureNumForName(name: *const c_char) -> c_int;
    fn R_PointInSubsector(x: fixed_t, y: fixed_t) -> *mut c_void;
    fn R_FillBackScreen();
    fn R_ExecuteSetViewSize();

    fn P_SetupLevel(episode: c_int, map: c_int, playermask: c_int, skill: skill_t);
    fn P_Ticker();
    fn P_SpawnPlayer(mthing: *mut c_void);
    fn P_Random() -> c_int;
    fn P_CheckPosition(thing: *mut c_void, x: fixed_t, y: fixed_t) -> boolean;
    fn P_RemoveMobj(th: *mut c_void);
    fn P_SpawnMobj(x: fixed_t, y: fixed_t, z: fixed_t, type_0: c_int) -> *mut c_void;
    fn P_ReadSaveGameHeader() -> boolean;
    fn P_WriteSaveGameHeader(description: *mut c_char);
    fn P_ReadSaveGameEOF() -> boolean;
    fn P_WriteSaveGameEOF();
    fn P_ArchivePlayers();
    fn P_UnArchivePlayers();
    fn P_ArchiveWorld();
    fn P_UnArchiveWorld();
    fn P_ArchiveThinkers();
    fn P_UnArchiveThinkers();
    fn P_ArchiveSpecials();
    fn P_UnArchiveSpecials();
    fn P_TempSaveGameFile() -> *mut c_char;
    fn P_SaveGameFile(slot: c_int) -> *mut c_char;

    fn M_CheckParm(check: *const c_char) -> c_int;
    fn M_CheckParmWithArgs(check: *const c_char, num_args: c_int) -> c_int;
    fn M_StringCopy(dest: *mut c_char, src: *const c_char, dest_size: usize) -> boolean;
    fn M_TempFile(s: *mut c_char) -> *mut c_char;
    fn M_WriteFile(name: *mut c_char, source: *mut c_void, length: c_int) -> boolean;
    fn M_ClearRandom();
    fn M_StartControlPanel();

    fn Z_Free(ptr: *mut c_void);
    fn Z_CheckHeap();

    fn W_CacheLumpName(name: *const c_char, tag: c_int) -> *mut c_void;
    fn W_ReleaseLumpName(name: *const c_char);
    fn W_CheckNumForName(name: *const c_char) -> c_int;

    fn S_StartSound(origin: *mut c_void, sound_id: c_int);
    fn S_PauseSound();
    fn S_ResumeSound();

    fn HU_Responder(ev: *mut event_t) -> boolean;
    fn HU_Ticker();
    fn HU_dequeueChatChar() -> c_char;

    fn ST_Responder(ev: *mut event_t) -> boolean;
    fn ST_Ticker();

    fn AM_Responder(ev: *mut event_t) -> boolean;
    fn AM_Ticker();
    fn AM_Stop();

    fn WI_Start(wbstartstruct: *mut wbstartstruct_t);
    fn WI_End();
    fn WI_Ticker();

    fn F_Responder(ev: *mut event_t) -> boolean;
    fn F_Ticker();
    fn F_StartFinale();

    fn StatCopy(stats: *mut wbstartstruct_t);
    fn V_ScreenShot(format: *mut c_char);

    fn I_GetTime() -> c_int;
    fn I_Quit();
    fn I_Error(format: *const c_char, ...);

    fn D_PageTicker();
    fn D_AdvanceDemo();

    fn strlen(s: *const c_char) -> usize;
    fn atoi(nptr: *const c_char) -> c_int;
    fn fclose(stream: *mut libc::FILE) -> c_int;
    fn fopen(filename: *const c_char, modes: *const c_char) -> *mut libc::FILE;
    fn ftell(stream: *mut libc::FILE) -> c_long;
    fn remove(filename: *const c_char) -> c_int;
    fn rename(old: *const c_char, new: *const c_char) -> c_int;
}

const DEH_INITIAL_HEALTH: c_int = 100;
const DEH_INITIAL_BULLETS: c_int = 50;

static GGSAVED_BUF: [c_char; 13] = [
    b'g' as c_char,
    b'a' as c_char,
    b'm' as c_char,
    b'e' as c_char,
    b' ' as c_char,
    b's' as c_char,
    b'a' as c_char,
    b'v' as c_char,
    b'e' as c_char,
    b'd' as c_char,
    b'.' as c_char,
    b'\0' as c_char,
    0,
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline]
fn DEH_String(s: *mut c_char) -> *mut c_char {
    s
}

unsafe fn logical_gamemission() -> c_int {
    if gamemission == d_mode::pack_chex {
        d_mode::doom
    } else if gamemission == d_mode::pack_hacx {
        d_mode::doom2
    } else {
        gamemission
    }
}

fn joybuttons(idx: c_int) -> c_int {
    // Emulate C pointer offset: joybuttons = &joyarray[1]
    // so joybuttons[-1] maps to joyarray[0].
    let offset = (idx + 1) as usize;
    unsafe { joyarray[offset] }
}

fn joybuttons_mut(idx: usize, val: c_int) {
    unsafe {
        joyarray[idx + 1] = val;
    }
}

fn mousebuttons(idx: c_int) -> c_int {
    // Emulate C pointer offset: mousebuttons = &mousearray[1]
    // so mousebuttons[-1] maps to mousearray[0].
    let offset = (idx + 1) as usize;
    unsafe { mousearray[offset] }
}

fn mousebuttons_mut(idx: usize, val: c_int) {
    unsafe {
        mousearray[idx + 1] = val;
    }
}

// ---------------------------------------------------------------------------
// G_CmdChecksum
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_CmdChecksum(cmd: *mut TiccmdT) -> c_int {
    let mut sum: c_int = 0;
    let cmd_bytes = cmd as *const c_int;
    let iterations = (std::mem::size_of::<TiccmdT>() / 4) - 1;
    for i in 0..iterations {
        sum += *cmd_bytes.add(i);
    }
    sum
}

// ---------------------------------------------------------------------------
// WeaponSelectable
// ---------------------------------------------------------------------------

unsafe fn WeaponSelectable(weapon: weapontype_t) -> boolean {
    if weapon == wp_supershotgun && logical_gamemission() == d_mode::doom {
        return 0;
    }

    if (weapon == wp_plasma || weapon == wp_bfg)
        && gamemission == d_mode::doom
        && gamemode == d_mode::shareware
    {
        return 0;
    }

    if (*players.as_mut_ptr().offset(consoleplayer as isize)).weaponowned[weapon as usize] == 0 {
        return 0;
    }

    if weapon == wp_fist
        && (*players.as_mut_ptr().offset(consoleplayer as isize)).weaponowned[wp_chainsaw as usize]
            != 0
        && (*players.as_mut_ptr().offset(consoleplayer as isize)).powers[pw_strength as usize] == 0
    {
        return 0;
    }

    1
}

// ---------------------------------------------------------------------------
// G_NextWeapon
// ---------------------------------------------------------------------------

unsafe fn G_NextWeapon(direction: c_int) -> c_int {
    let mut weapon: weapontype_t;

    let pending = (*players.as_mut_ptr().offset(consoleplayer as isize)).pendingweapon;
    if pending == wp_nochange {
        weapon = (*players.as_mut_ptr().offset(consoleplayer as isize)).readyweapon;
    } else {
        weapon = pending;
    }

    let mut i: c_int = 0;
    while (i as usize) < WEAPON_ORDER.len() {
        if WEAPON_ORDER[i as usize].0 == weapon {
            break;
        }
        i += 1;
    }

    let start_i = i;
    loop {
        i += direction;
        i = (i + WEAPON_ORDER.len() as c_int) % WEAPON_ORDER.len() as c_int;
        if i == start_i || WeaponSelectable(WEAPON_ORDER[i as usize].0) != 0 {
            break;
        }
    }

    WEAPON_ORDER[i as usize].1 as c_int
}

// ---------------------------------------------------------------------------
// SetJoyButtons / SetMouseButtons
// ---------------------------------------------------------------------------

unsafe fn SetJoyButtons(buttons_mask: c_uint) {
    for i in 0..MAX_JOY_BUTTONS {
        let button_on = ((buttons_mask & (1 << i)) != 0) as c_int;

        if joybuttons(i as c_int) == 0 && button_on != 0 {
            if i as c_int == joybprevweapon {
                next_weapon = -1;
            } else if i as c_int == joybnextweapon {
                next_weapon = 1;
            }
        }

        joybuttons_mut(i, button_on);
    }
}

unsafe fn SetMouseButtons(buttons_mask: c_uint) {
    for i in 0..MAX_MOUSE_BUTTONS {
        let button_on = ((buttons_mask & (1 << i)) != 0) as c_int;

        if mousebuttons(i as c_int) == 0 && button_on != 0 {
            if i as c_int == mousebprevweapon {
                next_weapon = -1;
            } else if i as c_int == mousebnextweapon {
                next_weapon = 1;
            }
        }

        mousebuttons_mut(i, button_on);
    }
}

// ---------------------------------------------------------------------------
// G_BuildTiccmd
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_BuildTiccmd(cmd: *mut TiccmdT, maketic: c_int) {
    let mut forward: c_int;
    let mut side: c_int;
    let speed: c_int;
    let mut tspeed: c_int;

    ptr::write_bytes(cmd as *mut u8, 0, std::mem::size_of::<TiccmdT>());

    (*cmd).consistancy = consistancy[consoleplayer as usize][(maketic as usize) % BACKUPTICS] as u8;

    let strafe = gamekeydown[key_strafe as usize] != 0
        || mousebuttons(mousebstrafe) != 0
        || joybuttons(joybstrafe) != 0;

    speed = (key_speed >= NUMKEYS as c_int
        || joybspeed as usize >= MAX_JOY_BUTTONS
        || gamekeydown[key_speed as usize] != 0
        || joybuttons(joybspeed) != 0) as c_int;

    forward = 0;
    side = 0;

    if joyxmove < 0
        || joyxmove > 0
        || gamekeydown[key_right as usize] != 0
        || gamekeydown[key_left as usize] != 0
    {
        turnheld += ticdup;
    } else {
        turnheld = 0;
    }

    if turnheld < SLOWTURNTICS {
        tspeed = 2;
    } else {
        tspeed = speed;
    }

    if strafe {
        if gamekeydown[key_right as usize] != 0 {
            side += sidemove[speed as usize];
        }
        if gamekeydown[key_left as usize] != 0 {
            side -= sidemove[speed as usize];
        }
        if joyxmove > 0 {
            side += sidemove[speed as usize];
        }
        if joyxmove < 0 {
            side -= sidemove[speed as usize];
        }
    } else {
        if gamekeydown[key_right as usize] != 0 {
            (*cmd).angleturn -= angleturn[tspeed as usize] as i16;
        }
        if gamekeydown[key_left as usize] != 0 {
            (*cmd).angleturn += angleturn[tspeed as usize] as i16;
        }
        if joyxmove > 0 {
            (*cmd).angleturn -= angleturn[tspeed as usize] as i16;
        }
        if joyxmove < 0 {
            (*cmd).angleturn += angleturn[tspeed as usize] as i16;
        }
    }

    if gamekeydown[key_up as usize] != 0 {
        forward += forwardmove[speed as usize];
    }
    if gamekeydown[key_down as usize] != 0 {
        forward -= forwardmove[speed as usize];
    }

    if joyymove < 0 {
        forward += forwardmove[speed as usize];
    }
    if joyymove > 0 {
        forward -= forwardmove[speed as usize];
    }

    if gamekeydown[key_strafeleft as usize] != 0
        || joybuttons(joybstrafeleft) != 0
        || mousebuttons(mousebstrafeleft) != 0
        || joystrafemove < 0
    {
        side -= sidemove[speed as usize];
    }

    if gamekeydown[key_straferight as usize] != 0
        || joybuttons(joybstraferight) != 0
        || mousebuttons(mousebstraferight) != 0
        || joystrafemove > 0
    {
        side += sidemove[speed as usize];
    }

    (*cmd).chatchar = HU_dequeueChatChar() as u8;

    if gamekeydown[key_fire as usize] != 0
        || mousebuttons(mousebfire) != 0
        || joybuttons(joybfire) != 0
    {
        (*cmd).buttons |= BT_ATTACK;
    }

    if gamekeydown[key_use as usize] != 0
        || joybuttons(joybuse) != 0
        || mousebuttons(mousebuse) != 0
    {
        (*cmd).buttons |= BT_USE;
        dclicks = 0;
    }

    if gamestate == GS_LEVEL && next_weapon != 0 {
        let i = G_NextWeapon(next_weapon);
        (*cmd).buttons |= BT_CHANGE;
        (*cmd).buttons |= (i << BT_WEAPONSHIFT) as u8;
    } else {
        for i in 0..8 {
            let key = *weapon_key_ptr(i);
            if gamekeydown[key as usize] != 0 {
                (*cmd).buttons |= BT_CHANGE;
                (*cmd).buttons |= ((i as c_int) << BT_WEAPONSHIFT) as u8;
                break;
            }
        }
    }

    next_weapon = 0;

    if mousebuttons(mousebforward) != 0 {
        forward += forwardmove[speed as usize];
    }
    if mousebuttons(mousebbackward) != 0 {
        forward -= forwardmove[speed as usize];
    }

    if dclick_use != 0 {
        if mousebuttons(mousebforward) != dclickstate && dclicktime > 1 {
            dclickstate = mousebuttons(mousebforward);
            if dclickstate != 0 {
                dclicks += 1;
            }
            if dclicks == 2 {
                (*cmd).buttons |= BT_USE;
                dclicks = 0;
            } else {
                dclicktime = 0;
            }
        } else {
            dclicktime += ticdup;
            if dclicktime > 20 {
                dclicks = 0;
                dclickstate = 0;
            }
        }

        let bstrafe = mousebuttons(mousebstrafe) != 0 || joybuttons(joybstrafe) != 0;
        if bstrafe != (dclickstate2 != 0) && dclicktime2 > 1 {
            dclickstate2 = bstrafe as c_int;
            if dclickstate2 != 0 {
                dclicks2 += 1;
            }
            if dclicks2 == 2 {
                (*cmd).buttons |= BT_USE;
                dclicks2 = 0;
            } else {
                dclicktime2 = 0;
            }
        } else {
            dclicktime2 += ticdup;
            if dclicktime2 > 20 {
                dclicks2 = 0;
                dclickstate2 = 0;
            }
        }
    }

    forward += mousey;

    if strafe {
        side += mousex * 2;
    } else {
        (*cmd).angleturn -= (mousex * 0x8) as i16;
    }

    if mousex == 0 {
        testcontrols_mousespeed = 0;
    }

    mousex = 0;
    mousey = 0;

    if forward > MAXPLMOVE {
        forward = MAXPLMOVE;
    } else if forward < -MAXPLMOVE {
        forward = -MAXPLMOVE;
    }
    if side > MAXPLMOVE {
        side = MAXPLMOVE;
    } else if side < -MAXPLMOVE {
        side = -MAXPLMOVE;
    }

    (*cmd).forwardmove += forward as i8;
    (*cmd).sidemove += side as i8;

    if sendpause != 0 {
        sendpause = 0;
        (*cmd).buttons = BT_SPECIAL | BTS_PAUSE;
    }

    if sendsave != 0 {
        sendsave = 0;
        (*cmd).buttons = BT_SPECIAL | BTS_SAVEGAME | ((savegameslot << BTS_SAVESHIFT) as u8);
    }

    if lowres_turn != 0 {
        static mut carry: c_short = 0;
        let desired_angleturn = (*cmd).angleturn as c_int + carry as c_int;

        (*cmd).angleturn = ((desired_angleturn + 128) & !0xff) as i16;

        carry = (desired_angleturn - (*cmd).angleturn as c_int) as c_short;
    }
}

// ---------------------------------------------------------------------------
// G_DoLoadLevel
// ---------------------------------------------------------------------------

unsafe fn G_DoLoadLevel() {
    skyflatnum = R_FlatNumForName(DEH_String(b"FLOOR7_2\0".as_ptr() as *mut c_char));

    if gamemode == d_mode::commercial
        && (gameversion == d_mode::exe_final2 || gameversion == d_mode::exe_chex)
    {
        let skytexturename: &'static [u8] = if gamemap < 12 {
            b"SKY1\0"
        } else if gamemap < 21 {
            b"SKY2\0"
        } else {
            b"SKY3\0"
        };
        skytexture = R_TextureNumForName(skytexturename.as_ptr() as *const c_char);
    }

    levelstarttic = gametic;

    if wipegamestate == GS_LEVEL {
        wipegamestate = -1;
    }

    gamestate = GS_LEVEL;

    for i in 0..MAXPLAYERS {
        turbodetected[i] = 0;
        if playeringame[i] != 0 && (*players.as_mut_ptr().offset(i as isize)).playerstate == 1 {
            (*players.as_mut_ptr().offset(i as isize)).playerstate = 2;
        }
        ptr::write_bytes(
            (*players.as_mut_ptr().offset(i as isize)).frags.as_ptr() as *mut u8,
            0,
            std::mem::size_of::<[c_int; MAXPLAYERS]>(),
        );
    }

    P_SetupLevel(gameepisode, gamemap, 0, gameskill);
    displayplayer = consoleplayer;
    gameaction = ga_nothing;
    Z_CheckHeap();

    ptr::write_bytes(
        gamekeydown.as_mut_ptr() as *mut u8,
        0,
        std::mem::size_of_val(&gamekeydown),
    );
    joyxmove = 0;
    joyymove = 0;
    joystrafemove = 0;
    mousex = 0;
    mousey = 0;
    sendpause = 0;
    sendsave = 0;
    paused = 0;
    ptr::write_bytes(
        mousearray.as_mut_ptr() as *mut u8,
        0,
        std::mem::size_of_val(&mousearray),
    );
    ptr::write_bytes(
        joyarray.as_mut_ptr() as *mut u8,
        0,
        std::mem::size_of_val(&joyarray),
    );

    if testcontrols != 0 {
        (*players.as_mut_ptr().offset(consoleplayer as isize)).message =
            b"Press escape to quit.\0".as_ptr() as *mut c_char;
    }
}

// ---------------------------------------------------------------------------
// G_Responder
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_Responder(ev: *mut event_t) -> boolean {
    if gamestate == GS_LEVEL
        && (*ev).type_ == ev_keydown
        && (*ev).data1 == key_spy
        && (singledemo != 0 || deathmatch == 0)
    {
        loop {
            displayplayer += 1;
            if displayplayer == MAXPLAYERS as c_int {
                displayplayer = 0;
            }
            if playeringame[displayplayer as usize] != 0 || displayplayer == consoleplayer {
                break;
            }
        }
        return 1;
    }

    if gameaction == ga_nothing
        && singledemo == 0
        && (demoplayback != 0 || gamestate == GS_DEMOSCREEN)
    {
        if (*ev).type_ == ev_keydown
            || ((*ev).type_ == ev_mouse && (*ev).data1 != 0)
            || ((*ev).type_ == ev_joystick && (*ev).data1 != 0)
        {
            M_StartControlPanel();
            return 1;
        }
        return 0;
    }

    if gamestate == GS_LEVEL {
        if HU_Responder(ev) != 0 {
            return 1;
        }
        if ST_Responder(ev) != 0 {
            return 1;
        }
        if AM_Responder(ev) != 0 {
            return 1;
        }
    }

    if gamestate == GS_FINALE {
        if F_Responder(ev) != 0 {
            return 1;
        }
    }

    if testcontrols != 0 && (*ev).type_ == ev_mouse {
        testcontrols_mousespeed = (*ev).data2.abs();
    }

    if (*ev).type_ == ev_keydown && (*ev).data1 == key_prevweapon {
        next_weapon = -1;
    } else if (*ev).type_ == ev_keydown && (*ev).data1 == key_nextweapon {
        next_weapon = 1;
    }

    match (*ev).type_ {
        ev_keydown => {
            if (*ev).data1 == key_pause {
                sendpause = 1;
            } else if ((*ev).data1 as usize) < NUMKEYS {
                gamekeydown[(*ev).data1 as usize] = 1;
            }
            return 1;
        }
        ev_keyup => {
            if ((*ev).data1 as usize) < NUMKEYS {
                gamekeydown[(*ev).data1 as usize] = 0;
            }
            return 0;
        }
        ev_mouse => {
            SetMouseButtons((*ev).data1 as c_uint);
            mousex = (*ev).data2 * (mouseSensitivity + 5) / 10;
            mousey = (*ev).data3 * (mouseSensitivity + 5) / 10;
            return 1;
        }
        ev_joystick => {
            SetJoyButtons((*ev).data1 as c_uint);
            joyxmove = (*ev).data2;
            joyymove = (*ev).data3;
            joystrafemove = (*ev).data4;
            return 1;
        }
        _ => {}
    }

    0
}

// ---------------------------------------------------------------------------
// G_Ticker
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_Ticker() {
    // Do player reborns if needed
    for i in 0..MAXPLAYERS {
        if playeringame[i] != 0 && (*players.as_mut_ptr().offset(i as isize)).playerstate == 2
        // PST_REBORN
        {
            G_DoReborn(i as c_int);
        }
    }

    // Do things to change the game state
    while gameaction != ga_nothing {
        match gameaction {
            ga_loadlevel => G_DoLoadLevel(),
            ga_newgame => G_DoNewGame(),
            ga_loadgame => G_DoLoadGame(),
            ga_savegame => G_DoSaveGame(),
            ga_playdemo => G_DoPlayDemo(),
            ga_completed => G_DoCompleted(),
            ga_victory => F_StartFinale(),
            ga_worlddone => G_DoWorldDone(),
            ga_screenshot => {
                V_ScreenShot(b"DOOM%02i.%s\0".as_ptr() as *mut c_char);
                (*players.as_mut_ptr().offset(consoleplayer as isize)).message =
                    DEH_String(b"screen shot\0".as_ptr() as *mut c_char);
                gameaction = ga_nothing;
            }
            _ => {}
        }
    }

    // Get commands, check consistency
    let buf = ((gametic / ticdup) as usize) % BACKUPTICS;

    for i in 0..MAXPLAYERS {
        if playeringame[i] != 0 {
            let cmd = &mut (*players.as_mut_ptr().offset(i as isize)).cmd;
            ptr::copy_nonoverlapping(netcmds.offset(i as isize), cmd, 1);

            if demoplayback != 0 {
                G_ReadDemoTiccmd(cmd);
            }
            if demorecording != 0 {
                G_WriteDemoTiccmd(cmd);
            }

            // Check for turbo cheats
            if cmd.forwardmove as c_int > TURBOTHRESHOLD {
                turbodetected[i] = 1;
            }

            if (gametic & 31) == 0
                && ((gametic >> 5) % MAXPLAYERS as c_int) == i as c_int
                && turbodetected[i] != 0
            {
                static mut TURBO_MSG_BUF: [c_char; 80] = [0; 80];
                libc::snprintf(
                    TURBO_MSG_BUF.as_mut_ptr(),
                    TURBO_MSG_BUF.len(),
                    c"%.32s is turbo!".as_ptr(),
                    *player_names.as_ptr().offset(i as isize),
                );
                (*players.as_mut_ptr().offset(consoleplayer as isize)).message =
                    TURBO_MSG_BUF.as_mut_ptr();
                turbodetected[i] = 0;
            }

            if netgame != 0 && netdemo == 0 && (gametic % ticdup == 0) {
                if gametic > BACKUPTICS as c_int
                    && consistancy[i][buf] as i16 != (*cmd).consistancy as i16
                {
                    I_Error(
                        c"consistency failure (%i should be %i)".as_ptr(),
                        (*cmd).consistancy as c_uint,
                        consistancy[i][buf],
                    );
                }
                if !(*players.as_mut_ptr().offset(i as isize)).mo.is_null() {
                    let mo = (*players.as_mut_ptr().offset(i as isize)).mo;
                    let mo_struct = mo as *const crate::doom::c_ffi::mobj_t;
                    consistancy[i][buf] = (*mo_struct).x;
                } else {
                    consistancy[i][buf] = rndindex;
                }
            }
        }
    }

    // Check for special buttons
    for i in 0..MAXPLAYERS {
        if playeringame[i] != 0 {
            let cmd_buttons = (*players.as_mut_ptr().offset(i as isize)).cmd.buttons;
            if cmd_buttons & BT_SPECIAL != 0 {
                match cmd_buttons & 3 {
                    BTS_PAUSE => {
                        paused ^= 1;
                        if paused != 0 {
                            S_PauseSound();
                        } else {
                            S_ResumeSound();
                        }
                    }
                    BTS_SAVEGAME => {
                        if savedescription[0] == 0 {
                            let net_msg = b"NET GAME\0";
                            for (j, &b) in net_msg.iter().enumerate() {
                                if j < 32 {
                                    savedescription[j] = b as c_char;
                                }
                            }
                        }
                        savegameslot = (((*players.as_mut_ptr().offset(i as isize)).cmd.buttons
                            & 0x1c)
                            >> BTS_SAVESHIFT) as c_int;
                        gameaction = ga_savegame;
                    }
                    _ => {}
                }
            }
        }
    }

    // Have we just finished displaying an intermission screen?
    if oldgamestate == GS_INTERMISSION && gamestate != GS_INTERMISSION {
        WI_End();
    }

    oldgamestate = gamestate;

    // Do main actions
    match gamestate {
        GS_LEVEL => {
            P_Ticker();
            ST_Ticker();
            AM_Ticker();
            HU_Ticker();
        }
        GS_INTERMISSION => {
            WI_Ticker();
        }
        GS_FINALE => {
            F_Ticker();
        }
        GS_DEMOSCREEN => {
            D_PageTicker();
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// G_InitPlayer / G_PlayerFinishLevel / G_PlayerReborn
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_InitPlayer(player: c_int) {
    G_PlayerReborn(player);
}

#[no_mangle]
pub unsafe extern "C" fn G_PlayerFinishLevel(player: c_int) {
    let p = &mut (*players.as_mut_ptr().offset(player as isize));

    ptr::write_bytes(
        p.powers.as_mut_ptr() as *mut u8,
        0,
        std::mem::size_of_val(&p.powers),
    );
    ptr::write_bytes(
        p.cards.as_mut_ptr() as *mut u8,
        0,
        std::mem::size_of_val(&p.cards),
    );

    // Clear MF_SHADOW flag (0x20) from mobj
    if !p.mo.is_null() {
        let mo = p.mo as *mut crate::doom::c_ffi::mobj_t;
        (*mo).flags &= !0x20;
    }

    p.extralight = 0;
    p.fixedcolormap = 0;
    p.damagecount = 0;
    p.bonuscount = 0;
}

#[no_mangle]
pub unsafe extern "C" fn G_PlayerReborn(player: c_int) {
    let mut frags: [c_int; MAXPLAYERS] = [0; MAXPLAYERS];
    let killcount: c_int;
    let itemcount: c_int;
    let secretcount: c_int;

    ptr::copy_nonoverlapping(
        (*players.as_mut_ptr().offset(player as isize))
            .frags
            .as_ptr(),
        frags.as_mut_ptr(),
        MAXPLAYERS,
    );
    killcount = (*players.as_mut_ptr().offset(player as isize)).killcount;
    itemcount = (*players.as_mut_ptr().offset(player as isize)).itemcount;
    secretcount = (*players.as_mut_ptr().offset(player as isize)).secretcount;

    let p = &mut (*players.as_mut_ptr().offset(player as isize));
    ptr::write_bytes(
        p as *mut PlayerT as *mut u8,
        0,
        std::mem::size_of::<PlayerT>(),
    );

    ptr::copy_nonoverlapping(
        frags.as_ptr(),
        (*players.as_mut_ptr().offset(player as isize))
            .frags
            .as_mut_ptr(),
        MAXPLAYERS,
    );
    (*players.as_mut_ptr().offset(player as isize)).killcount = killcount;
    (*players.as_mut_ptr().offset(player as isize)).itemcount = itemcount;
    (*players.as_mut_ptr().offset(player as isize)).secretcount = secretcount;

    p.usedown = 1;
    p.attackdown = 1;
    p.playerstate = 0; // PST_LIVE
    p.health = DEH_INITIAL_HEALTH;
    p.readyweapon = wp_pistol;
    p.pendingweapon = wp_pistol;
    p.weaponowned[wp_fist as usize] = 1;
    p.weaponowned[wp_pistol as usize] = 1;
    p.ammo[0] = DEH_INITIAL_BULLETS;

    for i in 0..NUMAMMO {
        p.maxammo[i] = maxammo[i];
    }
}

// ---------------------------------------------------------------------------
// G_CheckSpot
// ---------------------------------------------------------------------------

const MT_TFOG: c_int = 26;
const sfx_telept: c_int = 44;

#[no_mangle]
pub unsafe extern "C" fn G_CheckSpot(playernum: c_int, mthing: *mut c_void) -> boolean {
    let x: fixed_t;
    let y: fixed_t;

    let player_mo: *mut c_void =
        (*players.as_mut_ptr().offset(playernum as isize)).mo as *mut c_void;

    if player_mo.is_null() {
        // First spawn of level, before corpses
        for i in 0..playernum {
            let other_mo = (*players.as_mut_ptr().offset(i as isize)).mo;
            let other_mo_struct = other_mo as *const crate::doom::c_ffi::mobj_t;
            let other_x = (*other_mo_struct).x;
            let other_y = (*other_mo_struct).y;
            let mt = mthing as *const c_short;
            let mx = *mt.offset(0) as c_int;
            let my = *mt.offset(1) as c_int;
            if other_x == (mx as c_int) << FRACBITS && other_y == (my as c_int) << FRACBITS {
                return 0;
            }
        }
        return 1;
    }

    let mt = mthing as *const c_short;
    let mx = *mt.offset(0) as c_int;
    let my = *mt.offset(1) as c_int;

    x = (mx as c_int) << FRACBITS;
    y = (my as c_int) << FRACBITS;

    if P_CheckPosition(player_mo, x, y) == 0 {
        return 0;
    }

    // Flush an old corpse if needed
    if bodyqueslot >= BODYQUESIZE as c_int {
        P_RemoveMobj(bodyque[(bodyqueslot % BODYQUESIZE as c_int) as usize]);
    }
    bodyque[(bodyqueslot % BODYQUESIZE as c_int) as usize] = player_mo;
    bodyqueslot += 1;

    // Spawn a teleport fog
    let ss = R_PointInSubsector(x, y);

    {
        let mut xa: fixed_t;
        let mut ya: fixed_t;

        let mthing_angle = (*mt.offset(2)) as c_int;

        // Emulate vanilla Doom signed overflow behavior
        let an = (ANG45 >> ANGLETOFINESHIFT) as c_int * (mthing_angle / 45);

        match an {
            4096 => {
                xa = finetangent[2048];
                ya = finetangent[0];
            }
            5120 => {
                xa = finetangent[3072];
                ya = finetangent[1024];
            }
            6144 => {
                xa = finesine[0];
                ya = finetangent[2048];
            }
            7168 => {
                xa = finesine[1024];
                ya = finetangent[3072];
            }
            0 | 1024 | 2048 | 3072 => {
                xa = finecosine.add(an as usize).read();
                ya = finesine[an as usize];
            }
            _ => {
                I_Error(c"G_CheckSpot: unexpected angle %d".as_ptr(), an);
                xa = 0;
                ya = 0;
            }
        }

        // Get floor height from subsector's sector
        let ss_ptr = ss as *const *mut c_void;
        let sector = ss_ptr.read() as *const c_int;
        let floorheight = *sector;

        P_SpawnMobj(x + 20 * xa, y + 20 * ya, floorheight, MT_TFOG);
    }

    let p_viewz = (*players.as_mut_ptr().offset(consoleplayer as isize)).viewz;
    if p_viewz != 1 {
        S_StartSound(null_mut(), sfx_telept);
    }

    1
}

// ---------------------------------------------------------------------------
// G_DeathMatchSpawnPlayer
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_DeathMatchSpawnPlayer(playernum: c_int) {
    let selections: c_int = ((deathmatch_p as usize - deathmatchstarts.as_ptr() as usize)
        / std::mem::size_of::<*mut c_void>()) as c_int;
    if selections < 4 {
        I_Error(c"Only %i deathmatch spots, 4 required".as_ptr(), selections);
    }

    for _ in 0..20 {
        let i = P_Random() % selections;
        let dm_start = deathmatchstarts[i as usize];
        if G_CheckSpot(playernum, dm_start) != 0 {
            let type_ptr = (dm_start as *mut c_short).offset(3);
            *type_ptr = (playernum + 1) as c_short;
            P_SpawnPlayer(dm_start);
            return;
        }
    }

    P_SpawnPlayer(playerstarts[playernum as usize]);
}

// ---------------------------------------------------------------------------
// G_DoReborn
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_DoReborn(playernum: c_int) {
    if netgame == 0 {
        gameaction = ga_loadlevel;
    } else {
        // Disassociate the corpse
        let mo = (*players.as_mut_ptr().offset(playernum as isize)).mo;
        let mo = mo as *mut crate::doom::c_ffi::mobj_t;
        (*mo).player = null_mut();

        if deathmatch != 0 {
            G_DeathMatchSpawnPlayer(playernum);
            return;
        }

        let ps = playerstarts[playernum as usize];
        if G_CheckSpot(playernum, ps) != 0 {
            P_SpawnPlayer(ps);
            return;
        }

        for i in 0..MAXPLAYERS as c_int {
            let ps = playerstarts[i as usize];
            if G_CheckSpot(playernum, ps) != 0 {
                let type_ptr = (ps as *mut c_short).offset(3);
                *type_ptr = (playernum + 1) as c_short;
                P_SpawnPlayer(ps);
                *type_ptr = (i + 1) as c_short;
                return;
            }
        }
        P_SpawnPlayer(playerstarts[playernum as usize]);
    }
}

// ---------------------------------------------------------------------------
// G_ScreenShot / G_ExitLevel / G_SecretExitLevel
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_ScreenShot() {
    gameaction = ga_screenshot;
}

#[no_mangle]
pub unsafe extern "C" fn G_ExitLevel() {
    secretexit = 0;
    gameaction = ga_completed;
}

#[no_mangle]
pub unsafe extern "C" fn G_SecretExitLevel() {
    if gamemode == d_mode::commercial && W_CheckNumForName(c"map31".as_ptr()) < 0 {
        secretexit = 0;
    } else {
        secretexit = 1;
    }
    gameaction = ga_completed;
}

// ---------------------------------------------------------------------------
// G_DoCompleted
// ---------------------------------------------------------------------------

unsafe fn G_DoCompleted() {
    gameaction = ga_nothing;

    for i in 0..MAXPLAYERS {
        if playeringame[i] != 0 {
            G_PlayerFinishLevel(i as c_int);
        }
    }

    if automapactive != 0 {
        AM_Stop();
    }

    if gamemode != d_mode::commercial {
        if gameversion == d_mode::exe_chex {
            if gamemap == 5 {
                gameaction = ga_victory;
                return;
            }
        } else {
            match gamemap {
                8 => {
                    gameaction = ga_victory;
                    return;
                }
                9 => {
                    for i in 0..MAXPLAYERS {
                        (*players.as_mut_ptr().offset(i as isize)).didsecret = 1;
                    }
                }
                _ => {}
            }
        }
    }

    wminfo.didsecret = (*players.as_mut_ptr().offset(consoleplayer as isize)).didsecret;
    wminfo.epsd = gameepisode - 1;
    wminfo.last = gamemap - 1;

    if gamemode == d_mode::commercial {
        if secretexit != 0 {
            match gamemap {
                15 => wminfo.next = 30,
                31 => wminfo.next = 31,
                _ => {}
            }
        } else {
            match gamemap {
                31 | 32 => wminfo.next = 15,
                _ => wminfo.next = gamemap,
            }
        }
    } else {
        if secretexit != 0 {
            wminfo.next = 8;
        } else if gamemap == 9 {
            match gameepisode {
                1 => wminfo.next = 3,
                2 => wminfo.next = 5,
                3 => wminfo.next = 6,
                4 => wminfo.next = 2,
                _ => {}
            }
        } else {
            wminfo.next = gamemap;
        }
    }

    wminfo.maxkills = totalkills;
    wminfo.maxitems = totalitems;
    wminfo.maxsecret = totalsecret;
    wminfo.maxfrags = 0;

    if gamemode == d_mode::commercial {
        wminfo.partime = TICRATE * cpars[(gamemap - 1) as usize];
    } else if gameepisode < 4 {
        wminfo.partime = TICRATE * pars[gameepisode as usize][gamemap as usize];
    } else {
        wminfo.partime = TICRATE * cpars[gamemap as usize];
    }

    wminfo.pnum = consoleplayer;

    for i in 0..MAXPLAYERS {
        wminfo.plyr[i].in_ = playeringame[i];
        wminfo.plyr[i].skills = (*players.as_mut_ptr().offset(i as isize)).killcount;
        wminfo.plyr[i].sitems = (*players.as_mut_ptr().offset(i as isize)).itemcount;
        wminfo.plyr[i].ssecret = (*players.as_mut_ptr().offset(i as isize)).secretcount;
        wminfo.plyr[i].stime = leveltime;
        ptr::copy_nonoverlapping(
            (*players.as_mut_ptr().offset(i as isize)).frags.as_ptr(),
            wminfo.plyr[i].frags.as_mut_ptr(),
            MAXPLAYERS,
        );
    }

    gamestate = GS_INTERMISSION;
    viewactive = 0;
    automapactive = 0;

    StatCopy(&mut wminfo);

    WI_Start(&mut wminfo);
}

// ---------------------------------------------------------------------------
// G_WorldDone / G_DoWorldDone
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_WorldDone() {
    gameaction = ga_worlddone;

    if secretexit != 0 {
        (*players.as_mut_ptr().offset(consoleplayer as isize)).didsecret = 1;
    }

    if gamemode == d_mode::commercial {
        match gamemap {
            15 | 31 => {
                if secretexit != 0 {
                    F_StartFinale();
                }
            }
            6 | 11 | 20 | 30 => {
                F_StartFinale();
            }
            _ => {}
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn G_DoWorldDone() {
    gamestate = GS_LEVEL;
    gamemap = wminfo.next + 1;
    G_DoLoadLevel();
    gameaction = ga_nothing;
    viewactive = 1;
}

// ---------------------------------------------------------------------------
// G_LoadGame / G_DoLoadGame
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_LoadGame(name: *mut c_char) {
    M_StringCopy(
        savename.as_mut_ptr(),
        name,
        std::mem::size_of_val(&savename),
    );
    gameaction = ga_loadgame;
}

#[no_mangle]
pub unsafe extern "C" fn G_DoLoadGame() {
    let savedleveltime: c_int;

    gameaction = ga_nothing;

    save_stream = fopen(savename.as_ptr() as *const c_char, c"rb".as_ptr());

    if save_stream.is_null() {
        return;
    }

    savegame_error = 0;

    if P_ReadSaveGameHeader() == 0 {
        fclose(save_stream);
        return;
    }

    savedleveltime = leveltime;

    G_InitNew(gameskill, gameepisode, gamemap);

    leveltime = savedleveltime;

    P_UnArchivePlayers();
    P_UnArchiveWorld();
    P_UnArchiveThinkers();
    P_UnArchiveSpecials();

    if P_ReadSaveGameEOF() == 0 {
        I_Error(c"Bad savegame".as_ptr());
    }

    fclose(save_stream);

    if setsizeneeded != 0 {
        R_ExecuteSetViewSize();
    }

    R_FillBackScreen();
}

// ---------------------------------------------------------------------------
// G_SaveGame / G_DoSaveGame
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_SaveGame(slot: c_int, description: *mut c_char) {
    savegameslot = slot;
    M_StringCopy(
        savedescription.as_mut_ptr(),
        description,
        std::mem::size_of_val(&savedescription),
    );
    sendsave = 1;
}

#[no_mangle]
pub unsafe extern "C" fn G_DoSaveGame() {
    let temp_savegame_file = P_TempSaveGameFile();
    let savegame_file = P_SaveGameFile(savegameslot);

    let mut used_recovery = false;
    let mut recovery_savegame_file_holder: *mut c_char = null_mut();

    save_stream = fopen(temp_savegame_file, c"wb".as_ptr());

    if save_stream.is_null() {
        let recovery_savegame_file = M_TempFile(c"recovery.dsg".as_ptr() as *mut c_char);
        recovery_savegame_file_holder = recovery_savegame_file;
        save_stream = fopen(recovery_savegame_file, c"wb".as_ptr());
        if save_stream.is_null() {
            I_Error(
                c"Failed to open either '%s' or '%s' to write savegame.".as_ptr(),
                temp_savegame_file,
                recovery_savegame_file,
            );
        }
        used_recovery = true;
    }

    savegame_error = 0;

    P_WriteSaveGameHeader(savedescription.as_mut_ptr());

    P_ArchivePlayers();
    P_ArchiveWorld();
    P_ArchiveThinkers();
    P_ArchiveSpecials();

    P_WriteSaveGameEOF();

    if vanilla_savegame_limit != 0 && ftell(save_stream) > SAVEGAMESIZE as c_long {
        I_Error(c"Savegame buffer overrun".as_ptr());
    }

    fclose(save_stream);

    if used_recovery {
        I_Error(
            c"Failed to open savegame file '%s' for writing.\nBut your game has been saved to '%s' for recovery.".as_ptr(),
            temp_savegame_file,
            recovery_savegame_file_holder,
        );
    }

    remove(savegame_file);
    rename(temp_savegame_file, savegame_file);

    gameaction = ga_nothing;
    M_StringCopy(
        savedescription.as_mut_ptr(),
        c"".as_ptr(),
        std::mem::size_of_val(&savedescription),
    );

    (*players.as_mut_ptr().offset(consoleplayer as isize)).message =
        GGSAVED_BUF.as_ptr() as *mut c_char;

    R_FillBackScreen();
}

// ---------------------------------------------------------------------------
// G_DeferedInitNew / G_DoNewGame / G_InitNew
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn G_DeferedInitNew(skill: skill_t, episode: c_int, map: c_int) {
    d_skill = skill;
    d_episode = episode;
    d_map = map;
    gameaction = ga_newgame;
}

unsafe fn G_DoNewGame() {
    demoplayback = 0;
    netdemo = 0;
    netgame = 0;
    deathmatch = 0;
    playeringame[1] = 0;
    playeringame[2] = 0;
    playeringame[3] = 0;
    respawnparm = 0;
    fastparm = 0;
    nomonsters = 0;
    consoleplayer = 0;
    G_InitNew(d_skill, d_episode, d_map);
    gameaction = ga_nothing;
}

#[no_mangle]
pub unsafe extern "C" fn G_InitNew(skill: skill_t, episode: c_int, map: c_int) {
    if paused != 0 {
        paused = 0;
        S_ResumeSound();
    }

    let mut skill = skill;
    let mut episode = episode;
    let mut map = map;

    if skill > sk_nightmare {
        skill = sk_nightmare;
    }

    if gameversion >= d_mode::exe_ultimate {
        if episode == 0 {
            episode = 4;
        }
    } else {
        if episode < 1 {
            episode = 1;
        }
        if episode > 3 {
            episode = 3;
        }
    }

    if episode > 1 && gamemode == d_mode::shareware {
        episode = 1;
    }

    if map < 1 {
        map = 1;
    }

    if map > 9 && gamemode != d_mode::commercial {
        map = 9;
    }

    M_ClearRandom();

    if skill == sk_nightmare || respawnparm != 0 {
        respawnmonsters = 1;
    } else {
        respawnmonsters = 0;
    }

    if fastparm != 0 || (skill == sk_nightmare && gameskill != sk_nightmare) {
        // Speed up states S_SARG_RUN1..S_SARG_PAIN2
        for i in S_SARG_RUN1..=S_SARG_PAIN2 {
            states[i as usize].tics >>= 1;
        }
        mobjinfo[MT_BRUISERSHOT as usize].speed = 20 * FRACUNIT;
        mobjinfo[MT_HEADSHOT as usize].speed = 20 * FRACUNIT;
        mobjinfo[MT_TROOPSHOT as usize].speed = 20 * FRACUNIT;
    } else if skill != sk_nightmare && gameskill == sk_nightmare {
        for i in S_SARG_RUN1..=S_SARG_PAIN2 {
            states[i as usize].tics <<= 1;
        }
        mobjinfo[MT_BRUISERSHOT as usize].speed = 15 * FRACUNIT;
        mobjinfo[MT_HEADSHOT as usize].speed = 10 * FRACUNIT;
        mobjinfo[MT_TROOPSHOT as usize].speed = 10 * FRACUNIT;
    }

    for i in 0..MAXPLAYERS {
        (*players.as_mut_ptr().offset(i as isize)).playerstate = 2;
    }

    usergame = 1;
    paused = 0;
    demoplayback = 0;
    automapactive = 0;
    viewactive = 1;
    gameepisode = episode;
    gamemap = map;
    gameskill = skill;

    let skytexturename: &'static [u8] = if gamemode == d_mode::commercial {
        if gamemap < 12 {
            b"SKY1\0"
        } else if gamemap < 21 {
            b"SKY2\0"
        } else {
            b"SKY3\0"
        }
    } else {
        match gameepisode {
            2 => b"SKY2\0",
            3 => b"SKY3\0",
            4 => b"SKY4\0",
            _ => b"SKY1\0",
        }
    };

    skytexture = R_TextureNumForName(skytexturename.as_ptr() as *const c_char);

    G_DoLoadLevel();
}

// ---------------------------------------------------------------------------
// Demo recording/playback
// ---------------------------------------------------------------------------

const DOOM_191_VERSION: byte = 111;

#[no_mangle]
pub unsafe extern "C" fn G_ReadDemoTiccmd(cmd: *mut TiccmdT) {
    if *demo_p == DEMOMARKER {
        G_CheckDemoStatus();
        return;
    }

    (*cmd).forwardmove = *demo_p as i8;
    demo_p = demo_p.add(1);
    (*cmd).sidemove = *demo_p as i8;
    demo_p = demo_p.add(1);

    if longtics != 0 {
        // Longtics format: first byte is high-order, second byte is low-order.
        // Matches the C implementation: angleturn = (byte1 << 8) | byte2
        let high = *demo_p as u16;
        demo_p = demo_p.add(1);
        let low = *demo_p as u16;
        demo_p = demo_p.add(1);
        (*cmd).angleturn = ((high << 8) | low) as i16;
    } else {
        (*cmd).angleturn = (*demo_p as i16) << 8;
        demo_p = demo_p.add(1);
    }

    (*cmd).buttons = *demo_p;
    demo_p = demo_p.add(1);
}

unsafe fn IncreaseDemoBuffer() {
    let current_length = demoend.offset_from(demobuffer) as c_int;
    let new_length = current_length * 2;

    let new_demobuffer = Z_Malloc(new_length, PU_STATIC, null_mut()) as *mut byte;
    let new_demop = new_demobuffer.offset(demo_p.offset_from(demobuffer));

    ptr::copy_nonoverlapping(demobuffer, new_demobuffer, current_length as usize);

    Z_Free(demobuffer as *mut c_void);

    demobuffer = new_demobuffer;
    demo_p = new_demop;
    demoend = demobuffer.offset(new_length as isize);
}

#[no_mangle]
pub unsafe extern "C" fn G_WriteDemoTiccmd(cmd: *mut TiccmdT) {
    extern "C" {
        static mut key_demo_quit: c_int;
    }

    if gamekeydown[key_demo_quit as usize] != 0 {
        G_CheckDemoStatus();
        return;
    }

    let demo_start = demo_p;

    *demo_p = (*cmd).forwardmove as u8;
    demo_p = demo_p.add(1);
    *demo_p = (*cmd).sidemove as u8;
    demo_p = demo_p.add(1);

    if longtics != 0 {
        *demo_p = ((*cmd).angleturn & 0xff) as u8;
        demo_p = demo_p.add(1);
        *demo_p = ((*cmd).angleturn >> 8) as u8;
        demo_p = demo_p.add(1);
    } else {
        *demo_p = ((*cmd).angleturn >> 8) as u8;
        demo_p = demo_p.add(1);
    }

    *demo_p = (*cmd).buttons;
    demo_p = demo_p.add(1);

    demo_p = demo_start;

    if demo_p > demoend.offset(-16) {
        if vanilla_demo_limit != 0 {
            G_CheckDemoStatus();
            return;
        } else {
            IncreaseDemoBuffer();
        }
    }

    G_ReadDemoTiccmd(cmd);
}

#[no_mangle]
pub unsafe extern "C" fn G_RecordDemo(name: *mut c_char) {
    let mut maxsize: c_int = 0x20000;

    usergame = 0;

    let demoname_size = strlen(name) + 5;
    demoname = Z_Malloc(demoname_size as c_int, PU_STATIC, null_mut()) as *mut c_char;
    libc::snprintf(demoname, demoname_size, c"%s.lmp".as_ptr(), name);

    let i = M_CheckParmWithArgs(c"-maxdemo".as_ptr(), 1);
    if i != 0 {
        maxsize = atoi(*myargv.offset((i + 1) as isize)) * 1024;
    }

    demobuffer = Z_Malloc(maxsize, PU_STATIC, null_mut()) as *mut byte;
    demoend = demobuffer.offset(maxsize as isize);

    demorecording = 1;
}

#[no_mangle]
pub unsafe extern "C" fn G_VanillaVersionCode() -> c_int {
    match gameversion {
        d_mode::exe_doom_1_666 => 106,
        d_mode::exe_doom_1_7 => 107,
        d_mode::exe_doom_1_8 => 108,
        d_mode::exe_doom_1_9 => 109,
        _ => 109,
    }
}

#[no_mangle]
pub unsafe extern "C" fn G_BeginRecording() {
    longtics = (M_CheckParm(c"-longtics".as_ptr()) != 0) as c_int;
    lowres_turn = (longtics == 0) as c_int;

    demo_p = demobuffer;

    if longtics != 0 {
        *demo_p = DOOM_191_VERSION;
        demo_p = demo_p.add(1);
    } else {
        *demo_p = G_VanillaVersionCode() as u8;
        demo_p = demo_p.add(1);
    }

    *demo_p = gameskill as u8;
    demo_p = demo_p.add(1);
    *demo_p = gameepisode as u8;
    demo_p = demo_p.add(1);
    *demo_p = gamemap as u8;
    demo_p = demo_p.add(1);
    *demo_p = deathmatch as u8;
    demo_p = demo_p.add(1);
    *demo_p = respawnparm as u8;
    demo_p = demo_p.add(1);
    *demo_p = fastparm as u8;
    demo_p = demo_p.add(1);
    *demo_p = nomonsters as u8;
    demo_p = demo_p.add(1);
    *demo_p = consoleplayer as u8;
    demo_p = demo_p.add(1);

    for i in 0..MAXPLAYERS {
        *demo_p = playeringame[i] as u8;
        demo_p = demo_p.add(1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn G_DeferedPlayDemo(name: *const c_char) {
    defdemoname = name as *mut c_char;
    gameaction = ga_playdemo;
}

unsafe fn DemoVersionDescription(version: c_int) -> *const c_char {
    static mut RESULTBUF: [c_char; 16] = [0; 16];

    match version {
        104 => return c"v1.4".as_ptr(),
        105 => return c"v1.5".as_ptr(),
        106 => return c"v1.6/v1.666".as_ptr(),
        107 => return c"v1.7/v1.7a".as_ptr(),
        108 => return c"v1.8".as_ptr(),
        109 => return c"v1.9".as_ptr(),
        _ => {}
    }

    if version >= 0 && version <= 4 {
        return c"v1.0/v1.1/v1.2".as_ptr();
    } else {
        libc::snprintf(
            RESULTBUF.as_mut_ptr(),
            RESULTBUF.len(),
            c"%i.%i (unknown)".as_ptr(),
            version / 100,
            version % 100,
        );
        return RESULTBUF.as_ptr();
    }
}

#[no_mangle]
pub unsafe extern "C" fn G_DoPlayDemo() {
    let skill: skill_t;
    let mut episode: c_int;
    let mut map: c_int;
    let demoversion: c_int;

    gameaction = ga_nothing;
    demobuffer = W_CacheLumpName(defdemoname, PU_STATIC) as *mut byte;
    demo_p = demobuffer;

    demoversion = *demo_p as c_int;
    demo_p = demo_p.add(1);

    if demoversion == G_VanillaVersionCode() {
        longtics = 0;
    } else if demoversion == DOOM_191_VERSION as c_int {
        longtics = 1;
    } else {
        let message = c"Demo is from a different game version!\n(read %i, should be %i)\n\n*** You may need to upgrade your version of Doom to v1.9. ***\n    See: https://www.doomworld.com/classicdoom/info/patches.php\n    This appears to be %s.";
        libc::printf(
            message.as_ptr(),
            demoversion,
            G_VanillaVersionCode(),
            DemoVersionDescription(demoversion),
        );
        return;
    }

    skill = *demo_p as c_int;
    demo_p = demo_p.add(1);
    episode = *demo_p as c_int;
    demo_p = demo_p.add(1);
    map = *demo_p as c_int;
    demo_p = demo_p.add(1);
    deathmatch = *demo_p as c_int;
    demo_p = demo_p.add(1);
    respawnparm = *demo_p as c_int;
    demo_p = demo_p.add(1);
    fastparm = *demo_p as c_int;
    demo_p = demo_p.add(1);
    nomonsters = *demo_p as c_int;
    demo_p = demo_p.add(1);
    consoleplayer = *demo_p as c_int;
    demo_p = demo_p.add(1);

    for i in 0..MAXPLAYERS {
        playeringame[i] = *demo_p as c_int;
        demo_p = demo_p.add(1);
    }

    if playeringame[1] != 0
        || M_CheckParm(c"-solo-net".as_ptr()) > 0
        || M_CheckParm(c"-netdemo".as_ptr()) > 0
    {
        netgame = 1;
        netdemo = 1;
    }

    precache = 0;
    G_InitNew(skill, episode, map);
    precache = 1;
    starttime = I_GetTime();

    usergame = 0;
    demoplayback = 1;
}

#[no_mangle]
pub unsafe extern "C" fn G_TimeDemo(name: *mut c_char) {
    nodrawers = (M_CheckParm(c"-nodraw".as_ptr()) != 0) as c_int;

    timingdemo = 1;

    extern "C" {
        static mut singletics: c_int;
    }
    singletics = 1;

    defdemoname = name;
    gameaction = ga_playdemo;
}

#[no_mangle]
pub unsafe extern "C" fn G_CheckDemoStatus() -> boolean {
    if timingdemo != 0 {
        let endtime = I_GetTime();
        let realtics = endtime - starttime;
        let fps = ((gametic * TICRATE) as f32) / realtics as f32;

        timingdemo = 0;
        demoplayback = 0;

        I_Error(
            c"timed %i gametics in %i realtics (%f fps)".as_ptr(),
            gametic,
            realtics,
            fps as c_double,
        );
    }

    if demoplayback != 0 {
        W_ReleaseLumpName(defdemoname);
        demoplayback = 0;
        netdemo = 0;
        netgame = 0;
        deathmatch = 0;
        playeringame[1] = 0;
        playeringame[2] = 0;
        playeringame[3] = 0;
        respawnparm = 0;
        fastparm = 0;
        nomonsters = 0;
        consoleplayer = 0;

        if singledemo != 0 {
            I_Quit();
        } else {
            D_AdvanceDemo();
        }

        return 1;
    }

    if demorecording != 0 {
        *demo_p = DEMOMARKER;
        demo_p = demo_p.add(1);
        M_WriteFile(
            demoname,
            demobuffer as *mut c_void,
            (demo_p.offset_from(demobuffer)) as c_int,
        );
        Z_Free(demobuffer as *mut c_void);
        demorecording = 0;
        I_Error(c"Demo %s recorded".as_ptr(), demoname);
    }

    0
}

use std::ffi::c_double;
