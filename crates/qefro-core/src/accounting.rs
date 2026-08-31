//! Generic double-entry accounting primitives.
//!
//! Account, Journal Entry, Journal Line, and Fiscal Period are normal
//! [`EntityDef`] values. Posting is a business operation on EntityService.
//! There is no second ledger engine, ERP framework, or accounting API.

use crate::app::NavItem;
use crate::automation::{AutomationAction, AutomationDef, AutomationTrigger, NotifyAction};
use crate::document::{DocumentConfig, NamingConfig, PrintFormat, PrintSection, ReportDef};
use crate::entity::EntityDef;
use crate::field::{ChildTableDef, FieldDef, OnDelete};
use crate::platform::NotificationDef;
use crate::ui::{
    DashboardCard, DashboardDef, DetailViewSpec, EntityViews, FormViewSpec, ListColumnSpec,
    ListViewSpec, ViewSectionSpec,
};
use serde_json::json;

pub const ACCOUNT_ENTITY: &str = "Account";
pub const ACCOUNT_SLUG: &str = "accounts";
pub const JOURNAL_ENTITY: &str = "JournalEntry";
pub const JOURNAL_SLUG: &str = "journal-entries";
pub const JOURNAL_LINE_ENTITY: &str = "JournalLine";
pub const JOURNAL_LINE_SLUG: &str = "journal-lines";
pub const PERIOD_ENTITY: &str = "FiscalPeriod";
pub const PERIOD_SLUG: &str = "fiscal-periods";

pub const JOURNAL_WORKFLOW: &str = "journal_entry";
pub const PERIOD_WORKFLOW: &str = "fiscal_period";

pub const ACCOUNT_TYPE_ASSET: &str = "Asset";
pub const ACCOUNT_TYPE_LIABILITY: &str = "Liability";
pub const ACCOUNT_TYPE_EQUITY: &str = "Equity";
pub const ACCOUNT_TYPE_REVENUE: &str = "Revenue";
pub const ACCOUNT_TYPE_EXPENSE: &str = "Expense";

pub const JOURNAL_DRAFT: &str = "Draft";
pub const JOURNAL_POSTED: &str = "Posted";
pub const JOURNAL_REVERSED: &str = "Reversed";

pub const PERIOD_OPEN: &str = "Open";
pub const PERIOD_CLOSED: &str = "Closed";

/// Semantic account keys resolved from tenant business config — never hardcoded IDs.
pub const ACCOUNT_KEY_CASH: &str = "cash";
pub const ACCOUNT_KEY_RECEIVABLE: &str = "receivable";
pub const ACCOUNT_KEY_PAYABLE: &str = "payable";
pub const ACCOUNT_KEY_SALES: &str = "sales";
pub const ACCOUNT_KEY_COGS: &str = "cogs";
pub const ACCOUNT_KEY_INVENTORY: &str = "inventory";

pub fn account_types() -> Vec<&'static str> {
    vec![
        ACCOUNT_TYPE_ASSET,
        ACCOUNT_TYPE_LIABILITY,
        ACCOUNT_TYPE_EQUITY,
        ACCOUNT_TYPE_REVENUE,
        ACCOUNT_TYPE_EXPENSE,
    ]
}

pub fn account_entity() -> EntityDef {
    EntityDef::new(ACCOUNT_ENTITY)
        .label("Account")
        .label_plural("Accounts")
        .table_name("accounts")
        .slug_name(ACCOUNT_SLUG)
        .icon("book")
        .description("Tenant chart of accounts. Hierarchical via parent. Not a CRM account.")
        .display_field("name")
        .audit()
        .field(
            FieldDef::string("code")
                .required()
                .unique()
                .searchable()
                .search_weight(10)
                .filterable()
                .max_length(32)
                .section("Account"),
        )
        .field(
            FieldDef::string("name")
                .required()
                .searchable()
                .search_weight(8)
                .filterable()
                .max_length(120)
                .section("Account"),
        )
        .field(
            FieldDef::enum_values(
                "account_type",
                account_types().into_iter().map(|s| s.to_string()).collect(),
            )
            .required()
            .filterable()
            .label("Type")
            .section("Account"),
        )
        .field(
            FieldDef::many_to_one("parent_id", ACCOUNT_ENTITY)
                .nullable()
                .label("Parent")
                .on_delete(OnDelete::SetNull)
                .section("Account"),
        )
        .field(
            FieldDef::boolean("enabled")
                .required()
                .default_value(json!(true))
                .filterable()
                .section("Account"),
        )
        .field(
            FieldDef::string("currency")
                .nullable()
                .default_from("tenant_currency")
                .filterable()
                .section("Account"),
        )
        .field(FieldDef::one_to_many(
            "journal_lines",
            JOURNAL_LINE_ENTITY,
            "account_id",
        ))
        .views(EntityViews {
            default: Some("list".into()),
            list: Some(ListViewSpec {
                columns: vec![
                    ListColumnSpec {
                        field: "code".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "name".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "account_type".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "enabled".into(),
                        width: None,
                        widget: None,
                    },
                ],
                ..Default::default()
            }),
            form: Some(FormViewSpec::sections(vec![ViewSectionSpec::new(
                "Account",
            )
            .fields(&[
                "code",
                "name",
                "account_type",
                "parent_id",
                "enabled",
                "currency",
            ])])),
            detail: Some(DetailViewSpec::sections(vec![ViewSectionSpec::new(
                "Account",
            )
            .fields(&[
                "code",
                "name",
                "account_type",
                "parent_id",
                "enabled",
                "currency",
            ])])),
            ..Default::default()
        })
        .build()
}

pub fn journal_entry_entity() -> EntityDef {
    EntityDef::new(JOURNAL_ENTITY)
        .label("Journal Entry")
        .label_plural("Journal Entries")
        .table_name("journal_entries")
        .slug_name(JOURNAL_SLUG)
        .icon("list")
        .description("Double-entry journal. Posted entries are immutable; reverse to correct.")
        .workflow(JOURNAL_WORKFLOW)
        .display_field("doc_no")
        .audit()
        .document(
            DocumentConfig::new()
                .submit()
                .duplicate()
                .lock_states(&[JOURNAL_POSTED, JOURNAL_REVERSED]),
        )
        .naming(NamingConfig::new("JE-{YYYY}-{#####}"))
        .print_format(
            PrintFormat::new("Journal Entry", JOURNAL_ENTITY)
                .title("Journal Entry")
                .filename_field("doc_no")
                .item_table("lines")
                .total_fields(&["total_debit", "total_credit"])
                .section(PrintSection::kind("header"))
                .section(PrintSection::kind("items").loop_over("lines"))
                .section(PrintSection::kind("totals"))
                .section(PrintSection::kind("footer")),
        )
        .field(
            FieldDef::string("doc_no")
                .nullable()
                .searchable()
                .search_weight(10)
                .filterable()
                .readonly()
                .label("Number")
                .section("Journal"),
        )
        .field(
            FieldDef::date("posting_date")
                .required()
                .default_from("current_date")
                .filterable()
                .indexed()
                .label("Date")
                .section("Journal"),
        )
        .field(
            FieldDef::string("description")
                .required()
                .searchable()
                .search_weight(8)
                .section("Journal"),
        )
        .field(
            FieldDef::string("reference")
                .nullable()
                .searchable()
                .filterable()
                .indexed()
                .section("Journal"),
        )
        .field(
            FieldDef::enum_values(
                "status",
                vec![JOURNAL_DRAFT, JOURNAL_POSTED, JOURNAL_REVERSED],
            )
            .required()
            .default_value(json!(JOURNAL_DRAFT))
            .filterable()
            .readonly()
            .section("Journal"),
        )
        .field(
            FieldDef::string("currency")
                .required()
                .default_from("tenant_currency")
                .filterable()
                .section("Journal"),
        )
        .field(
            FieldDef::many_to_one("period_id", PERIOD_ENTITY)
                .nullable()
                .label("Period")
                .on_delete(OnDelete::Restrict)
                .section("Journal"),
        )
        .field(
            FieldDef::many_to_one("reversed_from_id", JOURNAL_ENTITY)
                .nullable()
                .label("Reverses")
                .readonly()
                .on_delete(OnDelete::SetNull)
                .section("Journal"),
        )
        .child_table(
            ChildTableDef::new("lines", JOURNAL_LINE_ENTITY)
                .parent_field("journal_id")
                .columns(&["account_id", "description", "debit", "credit"]),
        )
        .field(
            FieldDef::currency("total_debit")
                .computed("SUM(lines.debit)")
                .label("Debit")
                .section("Totals"),
        )
        .field(
            FieldDef::currency("total_credit")
                .computed("SUM(lines.credit)")
                .label("Credit")
                .section("Totals"),
        )
        .views(EntityViews {
            default: Some("list".into()),
            list: Some(ListViewSpec {
                columns: vec![
                    ListColumnSpec {
                        field: "doc_no".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "posting_date".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "description".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "status".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "total_debit".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "total_credit".into(),
                        width: None,
                        widget: None,
                    },
                ],
                ..Default::default()
            }),
            form: Some(FormViewSpec::sections(vec![
                ViewSectionSpec::new("Journal").fields(&[
                    "posting_date",
                    "description",
                    "reference",
                    "currency",
                    "period_id",
                    "status",
                ]),
                ViewSectionSpec::new("Lines").fields(&["lines"]),
                ViewSectionSpec::new("Totals").fields(&["total_debit", "total_credit"]),
            ])),
            detail: Some(DetailViewSpec::sections(vec![
                ViewSectionSpec::new("Journal").fields(&[
                    "doc_no",
                    "posting_date",
                    "description",
                    "reference",
                    "currency",
                    "period_id",
                    "status",
                    "reversed_from_id",
                ]),
                ViewSectionSpec::new("Lines").fields(&["lines"]),
                ViewSectionSpec::new("Totals").fields(&["total_debit", "total_credit"]),
            ])),
            ..Default::default()
        })
        .build()
}

pub fn journal_line_entity() -> EntityDef {
    EntityDef::new(JOURNAL_LINE_ENTITY)
        .label("Journal Line")
        .label_plural("Journal Lines")
        .table_name("journal_lines")
        .slug_name(JOURNAL_LINE_SLUG)
        .child_of(JOURNAL_ENTITY, "lines")
        .display_field("description")
        .no_activity()
        .no_comments()
        .field(
            FieldDef::many_to_one("journal_id", JOURNAL_ENTITY)
                .required()
                .hidden()
                .indexed()
                .on_delete(OnDelete::Cascade),
        )
        .field(
            FieldDef::many_to_one("account_id", ACCOUNT_ENTITY)
                .required()
                .label("Account")
                .indexed()
                .search_related(),
        )
        .field(FieldDef::string("description").nullable().searchable())
        .field(
            FieldDef::currency("debit")
                .required()
                .min(0.0)
                .default_value(json!(0)),
        )
        .field(
            FieldDef::currency("credit")
                .required()
                .min(0.0)
                .default_value(json!(0)),
        )
        .field(
            FieldDef::date("posting_date")
                .nullable()
                .indexed()
                .filterable()
                .server_managed()
                .label("Date"),
        )
        .field(
            FieldDef::string("journal_no")
                .nullable()
                .indexed()
                .searchable()
                .server_managed()
                .label("Reference"),
        )
        .field(
            FieldDef::boolean("posted")
                .default_value(json!(false))
                .filterable()
                .indexed()
                .hidden()
                .server_managed(),
        )
        .views(EntityViews {
            default: Some("list".into()),
            list: Some(ListViewSpec {
                columns: vec![
                    ListColumnSpec {
                        field: "posting_date".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "journal_no".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "account_id".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "description".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "debit".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "credit".into(),
                        width: None,
                        widget: None,
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        })
        .build()
}

pub fn fiscal_period_entity() -> EntityDef {
    EntityDef::new(PERIOD_ENTITY)
        .label("Fiscal Period")
        .label_plural("Fiscal Periods")
        .table_name("fiscal_periods")
        .slug_name(PERIOD_SLUG)
        .icon("calendar")
        .description("Open or closed posting window. Closed periods reject new postings.")
        .workflow(PERIOD_WORKFLOW)
        .display_field("name")
        .audit()
        .field(
            FieldDef::string("code")
                .required()
                .unique()
                .searchable()
                .search_weight(10)
                .filterable()
                .max_length(32)
                .help("For example 2026-01 or FY2026")
                .section("Period"),
        )
        .field(
            FieldDef::string("name")
                .required()
                .searchable()
                .section("Period"),
        )
        .field(
            FieldDef::date("start_date")
                .required()
                .filterable()
                .indexed()
                .section("Period"),
        )
        .field(
            FieldDef::date("end_date")
                .required()
                .filterable()
                .indexed()
                .section("Period"),
        )
        .field(
            FieldDef::enum_values("status", vec![PERIOD_OPEN, PERIOD_CLOSED])
                .required()
                .default_value(json!(PERIOD_OPEN))
                .filterable()
                .readonly()
                .section("Period"),
        )
        .validation_rule(crate::validation::ValidationRule::compare(
            "end_date",
            "greater_or_equal",
            "start_date",
        ))
        .views(EntityViews {
            default: Some("list".into()),
            list: Some(ListViewSpec {
                columns: vec![
                    ListColumnSpec {
                        field: "code".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "name".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "start_date".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "end_date".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "status".into(),
                        width: None,
                        widget: None,
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        })
        .build()
}

pub fn accounting_entities() -> Vec<EntityDef> {
    vec![
        account_entity(),
        journal_entry_entity(),
        journal_line_entity(),
        fiscal_period_entity(),
    ]
}

pub fn accounting_nav_items() -> Vec<NavItem> {
    vec![
        NavItem::new("Accounts", ACCOUNT_ENTITY).section("Finance"),
        NavItem::new("Journal Entries", JOURNAL_ENTITY).section("Finance"),
        NavItem::new("Fiscal Periods", PERIOD_ENTITY).section("Finance"),
    ]
}

pub fn accounting_reports() -> Vec<ReportDef> {
    vec![
        ReportDef::new("trial-balance", JOURNAL_LINE_ENTITY)
            .label("Trial Balance")
            .fields(&["account_id", "debit", "credit"])
            .group_by(&["account_id"])
            .sum("debit")
            .sum("credit")
            .filter_eq("posted", json!(true))
            .chart("bar"),
        ReportDef::new("general-ledger", JOURNAL_LINE_ENTITY)
            .label("General Ledger")
            .fields(&["account_id", "posting_date", "debit", "credit"])
            .group_by(&["account_id", "posting_date"])
            .sum("debit")
            .sum("credit")
            .filter_eq("posted", json!(true)),
        ReportDef::new("account-balance", JOURNAL_LINE_ENTITY)
            .label("Account Balance")
            .fields(&["account_id", "debit", "credit"])
            .group_by(&["account_id"])
            .sum("debit")
            .sum("credit")
            .filter_eq("posted", json!(true)),
    ]
}

pub fn accounting_dashboard() -> DashboardDef {
    DashboardDef::new("accounting", "Accounting")
        .card(
            DashboardCard::count("Posted journals", JOURNAL_ENTITY)
                .filter("status", JOURNAL_POSTED)
                .size("sm"),
        )
        .card(
            DashboardCard::count("Draft journals", JOURNAL_ENTITY)
                .filter("status", JOURNAL_DRAFT)
                .size("sm"),
        )
        .card(DashboardCard::recent("Recent journals", JOURNAL_ENTITY, 8))
}

pub fn accounting_notifications() -> Vec<NotificationDef> {
    vec![NotificationDef::new("journal_posted", "journal.posted")
        .channels(&["in_app"])
        .recipients(&["Manager", "Admin"])
        .title("Journal posted")
        .body("A journal entry was posted to the ledger.")]
}

pub fn accounting_automations() -> Vec<AutomationDef> {
    vec![AutomationDef::new(
        "journal_posted_notify",
        AutomationTrigger::event("journal.posted"),
    )
    .description("Notify managers when a journal is posted")
    .action(AutomationAction::Notify {
        notify: NotifyAction {
            notification: Some("journal_posted".into()),
            recipients: vec!["Manager".into()],
            title: Some("Journal posted".into()),
            ..Default::default()
        },
    })]
}

/// Resolve a semantic account key (`cash`, `sales`, …) from tenant business config.
pub fn tenant_account_code(
    business: &crate::ui::TenantBusinessConfig,
    key: &str,
) -> Option<String> {
    let value = match key {
        ACCOUNT_KEY_CASH => business.cash_account.as_deref(),
        ACCOUNT_KEY_RECEIVABLE => business.receivable_account.as_deref(),
        ACCOUNT_KEY_PAYABLE => business.payable_account.as_deref(),
        ACCOUNT_KEY_SALES => business.sales_account.as_deref(),
        ACCOUNT_KEY_COGS => business.cogs_account.as_deref(),
        ACCOUNT_KEY_INVENTORY => business.inventory_account.as_deref(),
        _ => None,
    }?;
    let t = value.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Semantic ledger posting for operations (sales, payments, future inventory).
#[derive(Debug, Clone)]
pub struct LedgerLineSpec {
    pub account_key: String,
    pub debit: serde_json::Value,
    pub credit: serde_json::Value,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LedgerPosting {
    pub description: String,
    pub reference: String,
    pub posting_date: Option<String>,
    pub lines: Vec<LedgerLineSpec>,
}

impl LedgerPosting {
    pub fn new(description: impl Into<String>, reference: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            reference: reference.into(),
            posting_date: None,
            lines: Vec::new(),
        }
    }

    pub fn date(mut self, date: impl Into<String>) -> Self {
        self.posting_date = Some(date.into());
        self
    }

    pub fn debit(
        mut self,
        account_key: impl Into<String>,
        amount: impl Into<serde_json::Value>,
    ) -> Self {
        self.lines.push(LedgerLineSpec {
            account_key: account_key.into(),
            debit: amount.into(),
            credit: serde_json::json!(0),
            description: None,
        });
        self
    }

    pub fn credit(
        mut self,
        account_key: impl Into<String>,
        amount: impl Into<serde_json::Value>,
    ) -> Self {
        self.lines.push(LedgerLineSpec {
            account_key: account_key.into(),
            debit: serde_json::json!(0),
            credit: amount.into(),
            description: None,
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::UI_SCHEMA_VERSION;

    #[test]
    fn accounting_entities_are_tenant_documents() {
        for entity in accounting_entities() {
            entity.validate_idents().unwrap();
            assert!(entity.tenant_owned, "{}", entity.name);
            assert_eq!(entity.to_ui_meta().schema_version, UI_SCHEMA_VERSION);
        }
        let journal = journal_entry_entity();
        assert_eq!(journal.workflow.as_deref(), Some(JOURNAL_WORKFLOW));
        assert!(journal.document.as_ref().unwrap().is_locked(JOURNAL_POSTED));
        assert!(journal.get_field("lines").unwrap().is_child_table());
        let account = account_entity();
        assert!(account.get_field("code").unwrap().unique);
        assert!(account.get_field("parent_id").is_some());
    }

    #[test]
    fn semantic_keys_do_not_hardcode_ids() {
        let mut cfg = crate::ui::TenantBusinessConfig::default();
        assert!(tenant_account_code(&cfg, ACCOUNT_KEY_CASH).is_none());
        cfg.cash_account = Some("1100".into());
        assert_eq!(
            tenant_account_code(&cfg, ACCOUNT_KEY_CASH).as_deref(),
            Some("1100")
        );
    }
}
