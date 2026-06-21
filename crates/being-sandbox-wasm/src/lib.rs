//! `being-sandbox-wasm` — the wasmtime/WASI **enforcement** backend for the M4 capability broker.
//!
//! [`being_sandbox`] defines the *policy* (deny-by-default authorization). This crate is the
//! *mechanism* that makes it unbypassable: the executor logic runs as a WebAssembly guest with **zero
//! ambient authority** — no WASI, no filesystem, no sockets, no imports at all except a single
//! broker-mediated host function. WebAssembly's "nothing is allowed unless the host imports it" model
//! means the guest *cannot* reach the host or the OS except through that one function, and that
//! function routes every request through [`being_sandbox::Broker::authorize`]. So a compromised or
//! self-modified executor still can only do what the operator granted.
//!
//! The guest here is the **real executor compiled to wasm32** (`guest/being-guest-wasm`, prebuilt into
//! [`GUEST_WASM`]): actual Rust executor logic that routes each effect through the broker and only
//! *performs* it when granted. [`Sandbox::guest_imports`] proves its only authority is the broker call;
//! [`Sandbox::execute`] returns the guest's computed result, showing real compiled-Rust logic ran under
//! the boundary (not just a forwarding stub).

use being_sandbox::{Authorization, Broker, CapabilitySet, EffectRequest};
use wasmtime::{Caller, Engine, Linker, Module, Store};

// The real executor guest, compiled from `guest/being-guest-wasm` to wasm32 and committed here (rebuild
// with `scripts/build_guest_wasm.sh`). Embedded so the green-gate never builds wasm. Its ONLY import is
// `host.request_effect` — no WASI, no memory exports, nothing else — so it has zero ambient authority.
const GUEST_WASM: &[u8] = include_bytes!("../guest.wasm");

/// Effect-kind codes the guest passes across the i32 ABI.
pub const KIND_QUERY: i32 = 0;
pub const KIND_EGRESS: i32 = 1;
pub const KIND_PAYMENT: i32 = 2;
pub const KIND_MEMORY_WRITE: i32 = 3;
pub const KIND_SIGN: i32 = 4;

struct SandboxState {
    caps: CapabilitySet,
    /// Candidate egress hosts the guest references by index (the guest names a host by `arg`).
    hosts: Vec<String>,
    last: Option<Authorization>,
}

/// A wasmtime-backed sandbox: runs the guest under zero ambient authority, routing its sole effect
/// import through the capability broker.
pub struct Sandbox {
    engine: Engine,
    module: Module,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox {
    pub fn new() -> Self {
        let engine = Engine::default();
        let module =
            Module::from_binary(&engine, GUEST_WASM).expect("committed guest.wasm is valid");
        Self { engine, module }
    }

    /// The guest's complete import list — the totality of authority it could possibly exercise. For an
    /// isolated executor this MUST be exactly `["host::request_effect"]` (no WASI / fs / net).
    pub fn guest_imports(&self) -> Vec<String> {
        self.module
            .imports()
            .map(|i| format!("{}::{}", i.module(), i.name()))
            .collect()
    }

    /// Run the guest requesting effect `(kind, arg)` under `caps`; returns the broker's verdict. The
    /// guest can do nothing except call the broker-mediated host import — so this verdict is the only
    /// thing it can cause.
    pub fn request(
        &self,
        caps: CapabilitySet,
        hosts: Vec<String>,
        kind: i32,
        arg: i32,
    ) -> Authorization {
        self.execute(caps, hosts, kind, arg).0
    }

    /// Like [`request`](Self::request) but also returns the guest's own computed result from `act` — the
    /// real executor's output: `> 0` when it performed the (granted) effect, `-1` when the broker denied
    /// it. Demonstrates that real compiled-Rust logic ran inside the sandbox and obeyed the verdict.
    pub fn execute(
        &self,
        caps: CapabilitySet,
        hosts: Vec<String>,
        kind: i32,
        arg: i32,
    ) -> (Authorization, i32) {
        let mut store = Store::new(
            &self.engine,
            SandboxState {
                caps,
                hosts,
                last: None,
            },
        );
        let mut linker = Linker::new(&self.engine);
        linker
            .func_wrap(
                "host",
                "request_effect",
                |mut caller: Caller<'_, SandboxState>, kind: i32, arg: i32| -> i32 {
                    let req = {
                        let st = caller.data();
                        match kind {
                            KIND_EGRESS => EffectRequest::Egress {
                                host: st.hosts.get(arg as usize).cloned().unwrap_or_default(),
                            },
                            KIND_PAYMENT => EffectRequest::Payment {
                                microdollars: arg as i64,
                            },
                            KIND_MEMORY_WRITE => EffectRequest::MemoryWrite,
                            KIND_SIGN => EffectRequest::Sign,
                            _ => EffectRequest::Query,
                        }
                    };
                    let auth = Broker::authorize(&req, &caller.data().caps);
                    let granted = auth.is_granted();
                    caller.data_mut().last = Some(auth);
                    granted as i32
                },
            )
            .expect("link host fn");
        let instance = linker
            .instantiate(&mut store, &self.module)
            .expect("instantiate guest");
        let act = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "act")
            .expect("guest exports act");
        let result = act.call(&mut store, (kind, arg)).expect("guest call");
        let auth = store
            .data()
            .last
            .clone()
            .unwrap_or(Authorization::Denied("guest requested no effect"));
        (auth, result)
    }
}

#[cfg(test)]
mod tests {
    use being_sandbox::{Capability, CapabilitySet};
    use std::collections::BTreeSet;

    use super::*;

    fn host_set(h: &str) -> Vec<String> {
        vec![h.to_string()]
    }

    #[test]
    fn guest_has_zero_ambient_authority() {
        // The whole point: the guest's ONLY import is the broker-mediated host fn — no WASI, no fs/net.
        let sb = Sandbox::new();
        assert_eq!(sb.guest_imports(), vec!["host::request_effect".to_string()]);
    }

    #[test]
    fn real_guest_executes_on_grant_and_not_on_denial() {
        // The compiled-Rust guest performs the effect (returns arg*2 > 0) only when the broker grants,
        // and returns -1 when denied — proving real executor logic ran inside the sandbox and obeyed
        // the verdict (not just a forwarding stub).
        // Use KIND_PAYMENT, whose `arg` is a numeric value (egress overloads `arg` as a host index).
        let sb = Sandbox::new();
        let caps = CapabilitySet::granted([Capability::Payment {
            max_microdollars: 1000,
        }]);
        let (granted, result) = sb.execute(caps, vec![], KIND_PAYMENT, 21);
        assert!(granted.is_granted());
        assert_eq!(
            result, 42,
            "guest should have performed the granted effect (21*2)"
        );

        let (denied, result) = sb.execute(CapabilitySet::none(), vec![], KIND_PAYMENT, 21);
        assert!(!denied.is_granted());
        assert_eq!(result, -1, "guest must NOT perform a denied effect");
    }

    #[test]
    fn pure_effect_passes_with_no_capabilities() {
        let sb = Sandbox::new();
        let a = sb.request(CapabilitySet::none(), vec![], KIND_QUERY, 0);
        assert!(a.is_granted());
    }

    #[test]
    fn egress_denied_without_capability_granted_with() {
        let sb = Sandbox::new();
        // no caps → the guest's egress request is denied at the broker boundary
        let denied = sb.request(
            CapabilitySet::none(),
            host_set("api.ok.test"),
            KIND_EGRESS,
            0,
        );
        assert!(!denied.is_granted());
        // grant egress to that host → now permitted
        let caps = CapabilitySet::granted([Capability::Egress {
            hosts: BTreeSet::from(["api.ok.test".to_string()]),
        }]);
        let granted = sb.request(caps, host_set("api.ok.test"), KIND_EGRESS, 0);
        assert!(granted.is_granted());
    }

    #[test]
    fn payment_is_bounded_through_the_sandbox() {
        let sb = Sandbox::new();
        let caps = || {
            CapabilitySet::granted([Capability::Payment {
                max_microdollars: 1000,
            }])
        };
        assert!(sb.request(caps(), vec![], KIND_PAYMENT, 1000).is_granted());
        assert!(!sb.request(caps(), vec![], KIND_PAYMENT, 1001).is_granted());
    }

    #[test]
    fn sign_and_memory_write_need_their_grant() {
        let sb = Sandbox::new();
        assert!(!sb
            .request(CapabilitySet::none(), vec![], KIND_SIGN, 0)
            .is_granted());
        let caps = CapabilitySet::granted([Capability::Sign]);
        assert!(sb.request(caps, vec![], KIND_SIGN, 0).is_granted());
    }
}
