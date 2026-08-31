//! Generic Task / assignment / follow-up primitive.
//!
//! Task is a normal [`EntityDef`]: REST, generic UI, search, workflow, activity,
//! audit, notifications, automation, and jobs all go through EntityService.
//! Applications opt related records in with [`EntityDef::with_tasks`].

use crate::app::NavItem;
use crate::automation::{AutomationAction, AutomationDef, AutomationTrigger, NotifyAction};
use crate::entity::EntityDef;
use crate::field::{FieldDef, OnDelete};
use crate::platform::{LinkDef, NotificationDef};
use crate::ui::{
    CalendarViewSpec, CardViewSpec, DashboardCard, DashboardDef, DetailViewSpec, EntityViews,
    FormViewSpec, KanbanCardSpec, KanbanViewSpec, ListColumnSpec, ListViewSpec, SortSpec, UiConfig,
    ViewSectionSpec,
};
use serde_json::json;

pub const TASK_ENTITY: &str = "Task";
pub const TASK_SLUG: &str = "tasks";
pub const TASK_WORKFLOW: &str = "task";
/// Polymorphic related-record type. Same convention as Activity (`entity_type`).
pub const RELATED_TYPE_FIELD: &str = "entity_type";
/// Polymorphic related-record id. Same convention as Activity (`entity_id`).
pub const RELATED_ID_FIELD: &str = "entity_id";

pub const STATUS_OPEN: &str = "Open";
pub const STATUS_IN_PROGRESS: &str = "In Progress";
pub const STATUS_COMPLETED: &str = "Completed";
pub const STATUS_CANCELLED: &str = "Cancelled";

pub const PRIORITY_LOW: &str = "low";
pub const PRIORITY_NORMAL: &str = "normal";
pub const PRIORITY_HIGH: &str = "high";
pub const PRIORITY_URGENT: &str = "urgent";

pub fn task_statuses() -> Vec<&'static str> {
    vec![
        STATUS_OPEN,
        STATUS_IN_PROGRESS,
        STATUS_COMPLETED,
        STATUS_CANCELLED,
    ]
}

pub fn task_priorities() -> Vec<&'static str> {
    vec![
        PRIORITY_LOW,
        PRIORITY_NORMAL,
        PRIORITY_HIGH,
        PRIORITY_URGENT,
    ]
}

/// Inverse one-to-many + Related-panel link so any business entity can show Tasks.
pub fn apply_task_link(entity: &mut EntityDef) -> bool {
    if entity.name == TASK_ENTITY {
        return false;
    }
    let mut added = false;
    if !entity.fields.iter().any(|f| {
        f.relation.as_ref().is_some_and(|rel| {
            rel.target_entity == TASK_ENTITY
                && rel.inverse_field.as_deref() == Some(RELATED_ID_FIELD)
        })
    }) {
        let mut name = "tasks".to_string();
        if entity.fields.iter().any(|f| f.name == name) {
            name = format!("{}_tasks", crate::ident::snake_case(&entity.name));
        }
        entity
            .fields
            .push(FieldDef::one_to_many(name, TASK_ENTITY, RELATED_ID_FIELD).label("Tasks"));
        added = true;
    }
    if !entity
        .links
        .iter()
        .any(|l| l.entity == TASK_ENTITY && l.relation == RELATED_ID_FIELD)
    {
        entity.links.push(
            LinkDef::new("Tasks", TASK_ENTITY, RELATED_ID_FIELD)
                .columns(&["title", "status", "priority", "due_at", "assigned_to"])
                .limit(20)
                .filter(RELATED_TYPE_FIELD, &entity.name),
        );
        added = true;
    }
    if added {
        entity.normalize();
    }
    added
}

/// Framework Task document. Tenant-owned, searchable, workflow-managed.
pub fn task_entity() -> EntityDef {
    EntityDef::new(TASK_ENTITY)
        .label("Task")
        .label_plural("Tasks")
        .table_name("tasks")
        .slug_name(TASK_SLUG)
        .icon("check")
        .description(
            "Generic follow-up or assignment. Optionally relates to any business record. Not a CRM-specific object.",
        )
        .workflow(TASK_WORKFLOW)
        .attachments()
        .display_field("title")
        .field(
            FieldDef::string("title")
                .required()
                .searchable()
                .search_weight(10)
                .max_length(200)
                .filterable()
                .section("Task"),
        )
        .field(
            FieldDef::text("description")
                .nullable()
                .searchable()
                .list(false)
                .section("Task"),
        )
        .field(
            FieldDef::enum_(
                "status",
                task_statuses().into_iter().map(|s| s.to_string()).collect(),
            )
            .required()
            .default_value(json!(STATUS_OPEN))
            .filterable()
            .section("Task"),
        )
        .field(
            FieldDef::enum_(
                "priority",
                task_priorities()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect(),
            )
            .required()
            .default_value(json!(PRIORITY_NORMAL))
            .filterable()
            .section("Task"),
        )
        .field(
            FieldDef::datetime("due_at")
                .nullable()
                .filterable()
                .ui(UiConfig::datetime())
                .label("Due")
                .section("Schedule"),
        )
        .field(
            FieldDef::datetime("completed_at")
                .nullable()
                .list(false)
                .ui(UiConfig::datetime())
                .readonly()
                .label("Completed")
                .section("Schedule"),
        )
        .field(
            FieldDef::assigned_to()
                .default_from("current_user")
                .on_delete(OnDelete::SetNull)
                .section("Assignment"),
        )
        .field(
            FieldDef::string(RELATED_TYPE_FIELD)
                .nullable()
                .filterable()
                .indexed()
                .label("Related type")
                .help("Entity this task is about, e.g. Customer or Order. Prefills from Related → Add Task.")
                .section("Related"),
        )
        .field(
            FieldDef::uuid(RELATED_ID_FIELD)
                .nullable()
                .filterable()
                .indexed()
                .label("Related record")
                .help("Record this task is about. Prefills from Related → Add Task.")
                .section("Related")
                .ui(UiConfig::relation()),
        )
        .views(EntityViews {
            default: Some("list".into()),
            list: Some(ListViewSpec {
                columns: vec![
                    ListColumnSpec {
                        field: "title".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "status".into(),
                        width: None,
                        widget: Some("status".into()),
                    },
                    ListColumnSpec {
                        field: "priority".into(),
                        width: None,
                        widget: None,
                    },
                    ListColumnSpec {
                        field: "due_at".into(),
                        width: None,
                        widget: Some("datetime".into()),
                    },
                    ListColumnSpec {
                        field: "assigned_to".into(),
                        width: None,
                        widget: Some("relation".into()),
                    },
                    ListColumnSpec {
                        field: RELATED_TYPE_FIELD.into(),
                        width: None,
                        widget: None,
                    },
                ],
                default_sort: Some(SortSpec {
                    field: "due_at".into(),
                    direction: Some("asc".into()),
                }),
                ..Default::default()
            }),
            card: Some(CardViewSpec {
                title: Some("title".into()),
                subtitle: Some("due_at".into()),
                fields: vec![
                    "status".into(),
                    "priority".into(),
                    "assigned_to".into(),
                    RELATED_TYPE_FIELD.into(),
                ],
                ..Default::default()
            }),
            form: Some(FormViewSpec::sections(vec![
                ViewSectionSpec::new("Task").fields(&["title", "description", "priority"]),
                ViewSectionSpec::new("Schedule").fields(&["due_at", "assigned_to"]),
                ViewSectionSpec::new("Related").fields(&[RELATED_TYPE_FIELD, RELATED_ID_FIELD]),
            ])),
            detail: Some(DetailViewSpec::sections(vec![
                ViewSectionSpec::new("Task").fields(&["title", "description", "status", "priority"]),
                ViewSectionSpec::new("Schedule").fields(&["due_at", "completed_at", "assigned_to"]),
                ViewSectionSpec::new("Related").fields(&[RELATED_TYPE_FIELD, RELATED_ID_FIELD]),
            ])),
            kanban: Some(KanbanViewSpec {
                group_by: Some("status".into()),
                card: Some(KanbanCardSpec {
                    title: Some("title".into()),
                    subtitle: Some("due_at".into()),
                    fields: vec!["priority".into(), "assigned_to".into()],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            calendar: Some(CalendarViewSpec {
                start: Some("due_at".into()),
                title: Some("title".into()),
                subtitle: Some("status".into()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .validation_rule(crate::validation::ValidationRule::compare(
            "due_at",
            "greater_or_equal",
            "created_at",
        ))
        .build()
}

pub fn task_nav_item() -> NavItem {
    NavItem::new("Tasks", TASK_ENTITY).section("Work")
}

pub fn task_notifications() -> Vec<NotificationDef> {
    vec![
        NotificationDef::new("task_assigned", "task.assigned")
            .channels(&["in_app"])
            .recipients(&["assignee"])
            .title("Task assigned")
            .body("A task was assigned to you."),
        NotificationDef::new("entity_assigned", "entity.assigned")
            .channels(&["in_app"])
            .recipients(&["assignee"])
            .title("Record assigned")
            .body("A record was assigned to you."),
        NotificationDef::new("task_due", "task.due")
            .channels(&["in_app"])
            .recipients(&["assignee", "creator"])
            .title("Task due")
            .body("A task is due."),
    ]
}

pub fn task_automations() -> Vec<AutomationDef> {
    vec![
        AutomationDef::new(
            "task_created_notify",
            AutomationTrigger::event("task.created"),
        )
        .description("Notify the assignee when a task is created with an assignment")
        .conditions(crate::condition::Condition {
            field: Some("assigned_to".into()),
            is_not_empty: Some(true),
            ..Default::default()
        })
        .action(AutomationAction::Notify {
            notify: NotifyAction {
                notification: Some("task_assigned".into()),
                recipients: vec!["assignee".into()],
                title: Some("Task assigned".into()),
                ..Default::default()
            },
        }),
        AutomationDef::new(
            "task_completed_activity",
            AutomationTrigger::event("workflow.transitioned"),
        )
        .description("Record activity when a task is completed")
        .conditions(crate::condition::Condition::all(vec![
            crate::condition::Condition::field_equals("entity", TASK_ENTITY),
            crate::condition::Condition::field_equals("to_state", STATUS_COMPLETED),
        ]))
        .action(AutomationAction::create_activity("Task completed")),
        AutomationDef::new(
            "task_cancelled_activity",
            AutomationTrigger::event("workflow.transitioned"),
        )
        .description("Record activity when a task is cancelled")
        .conditions(crate::condition::Condition::all(vec![
            crate::condition::Condition::field_equals("entity", TASK_ENTITY),
            crate::condition::Condition::field_equals("to_state", STATUS_CANCELLED),
        ]))
        .action(AutomationAction::create_activity("Task cancelled")),
    ]
}

pub fn task_dashboard() -> DashboardDef {
    DashboardDef::new("my-tasks", "Tasks")
        .card(
            DashboardCard::kpi("My open tasks", TASK_ENTITY)
                .filter("status", STATUS_OPEN)
                .filter("assigned_to", "current_user")
                .size("sm"),
        )
        .card(
            DashboardCard::count("Overdue tasks", TASK_ENTITY)
                .filter("due_at.lt", "now")
                .filter("status.neq", STATUS_COMPLETED)
                .filter("status.neq", STATUS_CANCELLED)
                .size("sm"),
        )
        .card(
            DashboardCard::count("Tasks due today", TASK_ENTITY)
                .filter("due_at.gte", "today")
                .filter("due_at.lt", "tomorrow")
                .filter("status.neq", STATUS_COMPLETED)
                .filter("status.neq", STATUS_CANCELLED)
                .size("sm"),
        )
        .card(
            DashboardCard::recent("Open tasks", TASK_ENTITY, 8)
                .filter("status.neq", STATUS_COMPLETED)
                .filter("status.neq", STATUS_CANCELLED),
        )
        .card(DashboardCard::workflow("Tasks by status", TASK_ENTITY).size("md"))
}

pub fn platform_entities() -> Vec<EntityDef> {
    let mut entities = crate::identity::identity_entities();
    entities.push(task_entity());
    entities
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::UI_SCHEMA_VERSION;

    #[test]
    fn task_is_a_normal_tenant_document() {
        let task = task_entity();
        assert!(task.tenant_owned);
        assert!(!task.skip_ddl);
        assert_eq!(task.slug, TASK_SLUG);
        assert_eq!(task.workflow.as_deref(), Some(TASK_WORKFLOW));
        assert_eq!(task.display_field, "title");
        assert!(task.attachments);
        assert!(task.activity);
        assert!(task.comments);
        assert!(task.audit);
        assert!(task.get_field("title").unwrap().searchable);
        assert!(task.get_field("description").unwrap().searchable);
        assert!(task.get_field("assigned_to").is_some());
        assert_eq!(
            task.get_field("assigned_to")
                .unwrap()
                .default_from
                .as_deref(),
            Some("current_user")
        );
        assert_eq!(
            task.get_field("priority").unwrap().default,
            Some(json!(PRIORITY_NORMAL))
        );
        assert_eq!(
            task.get_field("status").unwrap().default,
            Some(json!(STATUS_OPEN))
        );
        assert!(task.get_field(RELATED_TYPE_FIELD).is_some());
        assert!(task.get_field(RELATED_ID_FIELD).is_some());
        assert!(task.get_field("due_at").is_some());
        assert!(task.get_field("completed_at").unwrap().ui.readonly);
        assert_eq!(task.to_ui_meta().schema_version, UI_SCHEMA_VERSION);
        let caps = task.to_ui_meta().capabilities.unwrap();
        assert!(caps.workflow);
        assert!(caps.assignment);
        assert!(caps.activity);
        assert!(task.views.as_ref().unwrap().kanban.is_some());
        assert!(task.views.as_ref().unwrap().card.is_some());
    }

    #[test]
    fn with_tasks_adds_generic_related_link() {
        let mut customer = EntityDef::new("Customer")
            .table_name("customers")
            .slug_name("customers")
            .field(FieldDef::string("name").required())
            .build();
        assert!(apply_task_link(&mut customer));
        assert!(customer.get_field("tasks").is_some());
        let link = customer
            .links
            .iter()
            .find(|l| l.entity == TASK_ENTITY)
            .unwrap();
        assert_eq!(link.relation, RELATED_ID_FIELD);
        assert!(link
            .filters
            .iter()
            .any(|f| f.field == RELATED_TYPE_FIELD && f.value == "Customer"));
        assert!(!apply_task_link(&mut customer));
    }

    #[test]
    fn platform_entities_include_identity_and_task() {
        let names: Vec<_> = platform_entities().into_iter().map(|e| e.name).collect();
        assert!(names.contains(&"Person".into()));
        assert!(names.contains(&"User".into()));
        assert!(names.contains(&TASK_ENTITY.into()));
    }
}
