// Test helpers: expose C constants that are #define macros so Rust tests
// can validate them against ported Rust equivalents.
#include "doomdef.h"

int room_test_get_doom_191_version(void) {
    return DOOM_191_VERSION;
}
