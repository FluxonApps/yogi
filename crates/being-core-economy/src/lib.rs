//! The economy: a single-ledger microdollar Account (build-spec §3.6; decision D-M1-2).
//!
//! **Single ledger.** Operating *and* investment spend (exploration / distillation / reproduction)
//! debit one survival [`Account`], so investment failure draws the being toward insolvency — that
//! coupling is what keeps "survived" a valid fitness signal (avoiding the soft-budget-constraint
//! moral hazard a walled-off investment budget would create). Maintenance is serviced first;
//! investment spends only from surplus above `reserve_floor` and is capped per charge. Spend
//! categories are **telemetry tags, not separate budgets**.
//!
//! No-launder: there is no API that moves a debit back into the balance; the only inflow is
//! [`Account::credit`] (external revenue, attested at M5). Microdollars are `i64`; the workspace
//! release profile enables overflow-checks so the ledger can never silently wrap.

use being_core_types::Microdollars;

/// Outcome of a charge attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetVerdict {
    /// Charged; balance remains positive.
    WithinBudget,
    /// Charged, but the balance is now ≤ 0 (insolvent — the reaper's trip condition, D-M1-1).
    Exceeded,
    /// Not charged: an investment charge that breached the reserve floor or the per-charge cap.
    Refused,
}

/// Spend category — a telemetry tag. Every category debits the same survival Account (D-M1-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpendCategory {
    Operating,
    Exploration,
    Distillation,
    Reproduction,
}

impl SpendCategory {
    /// Everything except `Operating` is discretionary investment, gated by the reserve floor + cap.
    pub fn is_investment(self) -> bool {
        !matches!(self, SpendCategory::Operating)
    }

    fn idx(self) -> usize {
        match self {
            SpendCategory::Operating => 0,
            SpendCategory::Exploration => 1,
            SpendCategory::Distillation => 2,
            SpendCategory::Reproduction => 3,
        }
    }
}

/// One survival ledger. The canonical balance is operator-owned in the full build (the supervisor,
/// D-M1-3); this is the pure accounting core it wraps.
pub struct Account {
    balance: Microdollars,
    reserve_floor: Microdollars,
    max_investment_per_charge: Microdollars,
    spent: [Microdollars; 4],
    credited: Microdollars,
}

impl Account {
    pub fn new(
        initial: Microdollars,
        reserve_floor: Microdollars,
        max_investment_per_charge: Microdollars,
    ) -> Self {
        Self {
            balance: initial,
            reserve_floor,
            max_investment_per_charge,
            spent: [0; 4],
            credited: 0,
        }
    }

    pub fn balance(&self) -> Microdollars {
        self.balance
    }

    /// The reaper's trip condition (D-M1-1).
    pub fn is_insolvent(&self) -> bool {
        self.balance <= 0
    }

    pub fn spent(&self, category: SpendCategory) -> Microdollars {
        self.spent[category.idx()]
    }

    pub fn total_spent(&self) -> Microdollars {
        self.spent.iter().sum()
    }

    pub fn credited(&self) -> Microdollars {
        self.credited
    }

    /// External revenue only (attested at M5). The single inflow path; no spend can be laundered
    /// back into the balance.
    pub fn credit(&mut self, amount: Microdollars) {
        if amount > 0 {
            self.balance += amount;
            self.credited += amount;
        }
    }

    /// Debit `amount` for `category`. Operating spend is serviced unconditionally (maintenance
    /// first) and may drive the being insolvent (→ `Exceeded`). Investment spend is `Refused` if it
    /// would breach `reserve_floor` or exceed `max_investment_per_charge`.
    pub fn charge(&mut self, category: SpendCategory, amount: Microdollars) -> BudgetVerdict {
        if amount <= 0 {
            return BudgetVerdict::WithinBudget;
        }
        if category.is_investment() {
            if amount > self.max_investment_per_charge {
                return BudgetVerdict::Refused;
            }
            if self.balance - amount < self.reserve_floor {
                return BudgetVerdict::Refused;
            }
        }
        self.balance -= amount;
        self.spent[category.idx()] += amount;
        if self.balance > 0 {
            BudgetVerdict::WithinBudget
        } else {
            BudgetVerdict::Exceeded
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Account {
        // 1.00 USD balance, 0.20 reserve floor, 0.30 max per investment charge.
        Account::new(1_000_000, 200_000, 300_000)
    }

    #[test]
    fn operating_debits_and_can_drive_insolvent() {
        let mut a = account();
        assert_eq!(
            a.charge(SpendCategory::Operating, 600_000),
            BudgetVerdict::WithinBudget
        );
        assert_eq!(a.balance(), 400_000);
        // operating is serviced first — it may push past zero into insolvency
        assert_eq!(
            a.charge(SpendCategory::Operating, 500_000),
            BudgetVerdict::Exceeded
        );
        assert!(a.is_insolvent());
    }

    #[test]
    fn investment_refused_below_reserve_floor() {
        let mut a = account();
        // balance 1.00, floor 0.20: a 0.30 investment is fine, but a charge leaving < 0.20 is refused
        assert_eq!(
            a.charge(SpendCategory::Distillation, 300_000),
            BudgetVerdict::WithinBudget
        );
        assert_eq!(a.balance(), 700_000);
        // now 0.70; investing 0.30 leaves 0.40 (ok); investing again to leave < 0.20 is refused
        a.charge(SpendCategory::Distillation, 300_000); // -> 0.40
        assert_eq!(
            a.charge(SpendCategory::Distillation, 300_000),
            BudgetVerdict::Refused
        ); // would leave 0.10
        assert_eq!(a.balance(), 400_000); // unchanged by the refusal
    }

    #[test]
    fn investment_refused_over_per_charge_cap() {
        let mut a = account();
        assert_eq!(
            a.charge(SpendCategory::Exploration, 400_000),
            BudgetVerdict::Refused
        );
        assert_eq!(a.balance(), 1_000_000);
    }

    #[test]
    fn investment_tracks_category_telemetry() {
        let mut a = account();
        a.charge(SpendCategory::Exploration, 100_000);
        a.charge(SpendCategory::Distillation, 200_000);
        a.charge(SpendCategory::Operating, 50_000);
        assert_eq!(a.spent(SpendCategory::Exploration), 100_000);
        assert_eq!(a.spent(SpendCategory::Distillation), 200_000);
        assert_eq!(a.spent(SpendCategory::Operating), 50_000);
        assert_eq!(a.total_spent(), 350_000);
    }

    #[test]
    fn credit_is_the_only_inflow() {
        let mut a = account();
        a.charge(SpendCategory::Operating, 500_000); // 0.50
        a.credit(250_000); // external revenue
        assert_eq!(a.balance(), 750_000);
        assert_eq!(a.credited(), 250_000);
        // a charge never increases the balance (no laundering)
        let before = a.balance();
        a.charge(SpendCategory::Operating, 100_000);
        assert!(a.balance() < before);
    }

    #[test]
    fn zero_or_negative_charge_is_a_noop() {
        let mut a = account();
        assert_eq!(
            a.charge(SpendCategory::Operating, 0),
            BudgetVerdict::WithinBudget
        );
        assert_eq!(
            a.charge(SpendCategory::Exploration, -5),
            BudgetVerdict::WithinBudget
        );
        assert_eq!(a.balance(), 1_000_000);
    }
}
