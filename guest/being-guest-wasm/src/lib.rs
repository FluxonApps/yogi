//! The **real executor guest**, compiled to `wasm32-unknown-unknown`. It runs under wasmtime inside
//! `being-sandbox-wasm` with ZERO ambient authority: the only thing it can import is `host.request_effect`
//! (no WASI, no fs, no sockets), so every side effect must be authorized by the host capability broker.
//!
//! This is the real *mechanism* (vs the earlier WAT stand-in): actual compiled Rust executor logic that
//! (1) routes each effect through the broker and (2) only *performs* the effect when the broker grants
//! it — `act` returns the executed result (`arg * 2`, always `> 0`) when granted, or `-1` when denied.
//! Because it's a `cdylib` with a single import, it provably cannot reach anything except the broker.
#![no_std]

#[link(wasm_import_module = "host")]
extern "C" {
    /// Ask the host broker to authorize effect `kind` with argument `arg`. Returns 1 if granted, else 0.
    fn request_effect(kind: i32, arg: i32) -> i32;
}

/// Execute one step. Always asks the broker first (zero ambient authority), then performs the effect
/// only if granted. Returns the executed result (`> 0`) on grant, or `-1` on denial.
#[no_mangle]
pub extern "C" fn act(kind: i32, arg: i32) -> i32 {
    // SAFETY: the host always links `request_effect`; it is the guest's sole import.
    let granted = unsafe { request_effect(kind, arg) };
    if granted == 1 {
        // Perform the effect. A production executor would do real work here; the deterministic
        // stand-in returns arg*2 (clamped > 0) so the host can observe that execution actually ran.
        arg.wrapping_mul(2).max(1)
    } else {
        -1 // denied by the broker — the effect is NOT performed
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
