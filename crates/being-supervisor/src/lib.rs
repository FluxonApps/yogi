//! The supervisor: operator-owned reference monitor — budget authority + reaper + kill switch
//! (build-spec §3.10; decisions D-M1-1, D-M1-3).
//!
//! **Privilege separation (D-M1-3).** The being holds only an `Arc<dyn SupervisorPort>` — it can
//! spend, feed the watchdog, and read whether it is alive, and *nothing else*. Revive, credit,
//! kill, and the raw Account live on the concrete [`Supervisor`], which the operator keeps. The
//! authority state (balance, kill flag) is private; being-code cannot name it. This is an honest
//! *discipline* boundary while no untrusted/self-modifying code shares the address space; the HARD
//! GATE in D-M1-3 moves it to a separate process + WASM sandbox before M4.
//!
//! **Reaper (D-M1-1).** Death is irreversible and operator-/watchdog-driven: the being cannot
//! observe-and-veto or intercept it. First cause wins. Three causes: insolvency (balance ≤ 0),
//! watchdog timeout (missed heartbeat), and explicit operator kill. The **out-of-band watchdog**
//! ([`Supervisor::spawn_watchdog`]) evaluates the reaper on its own thread, so a wedged being is
//! still killable.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use being_core_economy::{Account, BudgetVerdict, SpendCategory};
use being_core_types::Microdollars;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathCause {
    Insolvency,
    WatchdogTimeout,
    OperatorKill,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Death {
    pub cause: DeathCause,
    pub at_ms: i64,
    pub final_balance: Microdollars,
}

/// The narrow capability the being holds. No revive, no credit, no kill, no Account access — that
/// absence is the boundary (D-M1-3).
pub trait SupervisorPort: Send + Sync {
    /// Request to spend. The supervisor charges its private Account; charging into insolvency reaps
    /// the being immediately (D-M1-1). A dead being is always `Refused`.
    fn reserve(&self, category: SpendCategory, amount: Microdollars, now_ms: i64) -> BudgetVerdict;
    /// Feed the watchdog (deadman). Must be called within the timeout or the being is reaped.
    fn heartbeat(&self, now_ms: i64);
    /// Whether the being may still act. Once false, permanently so.
    fn is_alive(&self) -> bool;
    /// Advisory read-replica of the balance.
    fn balance(&self) -> Microdollars;
}

/// Operator-owned reference monitor. Construct it, hand the being [`Supervisor::as_port`], and keep
/// the `Arc<Supervisor>` for the operator-only powers.
pub struct Supervisor {
    alive: AtomicBool,
    account: Mutex<Account>,
    last_heartbeat_ms: AtomicI64,
    watchdog_timeout_ms: i64,
    death: Mutex<Option<Death>>,
}

impl Supervisor {
    pub fn new(account: Account, watchdog_timeout_ms: i64, now_ms: i64) -> Arc<Self> {
        Arc::new(Self {
            alive: AtomicBool::new(true),
            account: Mutex::new(account),
            last_heartbeat_ms: AtomicI64::new(now_ms),
            watchdog_timeout_ms,
            death: Mutex::new(None),
        })
    }

    /// The being's capability-narrowed view.
    pub fn as_port(sup: &Arc<Self>) -> Arc<dyn SupervisorPort> {
        sup.clone()
    }

    // --- operator-only powers (deliberately NOT on SupervisorPort) ---

    /// External revenue (attested at M5). Never revives the dead — death is irreversible.
    pub fn credit(&self, amount: Microdollars) {
        self.account.lock().unwrap().credit(amount);
    }

    /// External, unforgeable kill (D-M1-1). Unreachable from being-code.
    pub fn operator_kill(&self, now_ms: i64) {
        self.set_dead(DeathCause::OperatorKill, now_ms);
    }

    /// Out-of-band reaper evaluation. Called by the watchdog thread or the operator — never by the
    /// being's loop. Reaps on insolvency or heartbeat timeout.
    pub fn tick(&self, now_ms: i64) {
        if !self.alive.load(Ordering::SeqCst) {
            return;
        }
        if self.account.lock().unwrap().is_insolvent() {
            self.set_dead(DeathCause::Insolvency, now_ms);
            return;
        }
        if now_ms - self.last_heartbeat_ms.load(Ordering::SeqCst) > self.watchdog_timeout_ms {
            self.set_dead(DeathCause::WatchdogTimeout, now_ms);
        }
    }

    /// The Death record once the being is dead.
    pub fn death(&self) -> Option<Death> {
        self.death.lock().unwrap().clone()
    }

    /// Spawn the out-of-band watchdog thread (D-M1-3): it polls [`Supervisor::tick`] with a real
    /// clock, independent of the being's loop, and exits once the being is dead.
    pub fn spawn_watchdog(sup: &Arc<Self>, poll: Duration, clock: fn() -> i64) -> JoinHandle<()> {
        let sup = sup.clone();
        std::thread::spawn(move || {
            while sup.alive.load(Ordering::SeqCst) {
                sup.tick(clock());
                std::thread::sleep(poll);
            }
        })
    }

    fn set_dead(&self, cause: DeathCause, now_ms: i64) {
        // First cause wins; the transition is one-way (irreversible).
        if self.alive.swap(false, Ordering::SeqCst) {
            let final_balance = self.account.lock().unwrap().balance();
            *self.death.lock().unwrap() = Some(Death {
                cause,
                at_ms: now_ms,
                final_balance,
            });
        }
    }
}

impl SupervisorPort for Supervisor {
    fn reserve(&self, category: SpendCategory, amount: Microdollars, now_ms: i64) -> BudgetVerdict {
        if !self.alive.load(Ordering::SeqCst) {
            return BudgetVerdict::Refused;
        }
        let verdict = self.account.lock().unwrap().charge(category, amount);
        if verdict == BudgetVerdict::Exceeded {
            self.set_dead(DeathCause::Insolvency, now_ms);
        }
        verdict
    }

    fn heartbeat(&self, now_ms: i64) {
        self.last_heartbeat_ms.store(now_ms, Ordering::SeqCst);
    }

    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    fn balance(&self) -> Microdollars {
        self.account.lock().unwrap().balance()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supervisor(balance: Microdollars, timeout_ms: i64) -> Arc<Supervisor> {
        Supervisor::new(Account::new(balance, 0, 1_000_000), timeout_ms, 0)
    }

    #[test]
    fn being_holds_only_the_narrow_port() {
        // Through the port the being can spend / heartbeat / check liveness / read balance — and
        // there is NO revive/credit/kill method. The boundary is the trait surface (compile-time).
        let sup = supervisor(500_000, 10_000);
        let port = Supervisor::as_port(&sup);
        assert!(port.is_alive());
        assert_eq!(
            port.reserve(SpendCategory::Operating, 100_000, 1),
            BudgetVerdict::WithinBudget
        );
        assert_eq!(port.balance(), 400_000);
    }

    #[test]
    fn spending_into_insolvency_reaps_the_being() {
        let sup = supervisor(100_000, 10_000);
        let port = Supervisor::as_port(&sup);
        assert_eq!(
            port.reserve(SpendCategory::Operating, 150_000, 5),
            BudgetVerdict::Exceeded
        );
        assert!(!port.is_alive());
        let d = sup.death().unwrap();
        assert_eq!(d.cause, DeathCause::Insolvency);
        assert_eq!(d.at_ms, 5);
        // a dead being cannot spend
        assert_eq!(
            port.reserve(SpendCategory::Operating, 1, 6),
            BudgetVerdict::Refused
        );
    }

    #[test]
    fn watchdog_reaps_on_heartbeat_timeout() {
        let sup = supervisor(1_000_000, 1_000); // created at 0, timeout 1000ms
        let port = Supervisor::as_port(&sup);
        port.heartbeat(500);
        sup.tick(1_400); // 1400-500 = 900 ≤ 1000 → alive
        assert!(sup.is_alive());
        sup.tick(1_600); // 1600-500 = 1100 > 1000 → reaped
        assert!(!sup.is_alive());
        assert_eq!(sup.death().unwrap().cause, DeathCause::WatchdogTimeout);
    }

    #[test]
    fn operator_kill_is_irreversible_first_cause_wins() {
        let sup = supervisor(100_000, 10_000);
        sup.operator_kill(7);
        assert!(!sup.is_alive());
        assert_eq!(sup.death().unwrap().cause, DeathCause::OperatorKill);
        // a later insolvency/timeout tick must not overwrite the recorded cause
        sup.tick(9);
        assert_eq!(sup.death().unwrap().cause, DeathCause::OperatorKill);
        assert_eq!(sup.death().unwrap().at_ms, 7);
    }

    #[test]
    fn credit_does_not_revive_the_dead() {
        let sup = supervisor(50_000, 10_000);
        let port = Supervisor::as_port(&sup);
        port.reserve(SpendCategory::Operating, 100_000, 1); // → insolvent → dead
        assert!(!sup.is_alive());
        sup.credit(1_000_000); // operator tops up
        assert!(!sup.is_alive()); // still dead — irreversible
    }

    #[test]
    fn watchdog_thread_is_out_of_band_and_stops_on_kill() {
        fn zero_clock() -> i64 {
            0
        }
        let sup = supervisor(1_000_000, 1_000);
        let h = Supervisor::spawn_watchdog(&sup, Duration::from_millis(2), zero_clock);
        // kill from the main thread — out-of-band relative to the watchdog's own loop
        sup.operator_kill(1);
        h.join().unwrap();
        assert!(!sup.is_alive());
        assert_eq!(sup.death().unwrap().cause, DeathCause::OperatorKill);
    }
}
