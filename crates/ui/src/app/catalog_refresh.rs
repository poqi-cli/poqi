use std::time::{Duration, Instant};

use pg_query::{
    parse,
    protobuf::{DropStmt, ObjectType, RawStmt, RenameStmt},
    NodeEnum,
};

const REFRESH_RETRY_DELAY: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub(crate) struct CatalogRefresh {
    pending: bool,
    cooldown_until: Option<Instant>,
    last_request_id: Option<u64>,
}

impl CatalogRefresh {
    pub(crate) fn new() -> Self {
        Self {
            pending: false,
            cooldown_until: None,
            last_request_id: None,
        }
    }

    pub(crate) fn mark_pending(&mut self) {
        self.pending = true;
    }

    pub(crate) fn on_dispatched(&mut self, request_id: u64) {
        self.last_request_id = Some(request_id);
        self.pending = false;
        self.cooldown_until = None;
    }

    pub(crate) fn on_success(&mut self) {
        self.pending = false;
        self.cooldown_until = None;
        self.last_request_id = None;
    }

    pub(crate) fn on_failure(&mut self, now: Instant) {
        self.pending = true;
        self.cooldown_until = Some(now + REFRESH_RETRY_DELAY);
        self.last_request_id = None;
    }

    pub(crate) fn ready_to_dispatch(&self, now: Instant) -> bool {
        self.pending && self.cooldown_until.is_none_or(|until| now >= until)
    }

    /// Returns true if the given request was a catalog refresh that should be re-queued.
    pub(crate) fn on_canceled(&mut self, request_id: u64) -> bool {
        if self.last_request_id == Some(request_id) {
            self.pending = true;
            self.last_request_id = None;
            self.cooldown_until = None;
            return true;
        }
        false
    }
}

pub(crate) fn mutates_catalog(sql: &str) -> bool {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Ok(parsed) = parse(trimmed) else {
        return false;
    };
    parsed.protobuf.stmts.iter().any(stmt_mutates_catalog)
}

fn stmt_mutates_catalog(stmt: &RawStmt) -> bool {
    stmt.stmt
        .as_ref()
        .and_then(|wrapper| wrapper.node.as_ref())
        .is_some_and(node_mutates_catalog)
}

fn node_mutates_catalog(node: &NodeEnum) -> bool {
    matches!(
        node,
        NodeEnum::CreateStmt(_)
            | NodeEnum::CreateSchemaStmt(_)
            | NodeEnum::CreateTableAsStmt(_)
            | NodeEnum::CreateForeignTableStmt(_)
            | NodeEnum::ViewStmt(_)
            | NodeEnum::AlterTableStmt(_)
            | NodeEnum::AlterObjectSchemaStmt(_)
    ) || matches!(node, NodeEnum::RenameStmt(rename) if rename_targets_catalog(rename))
        || matches!(node, NodeEnum::DropStmt(drop) if drop_targets_catalog(drop))
        || matches!(node, NodeEnum::SelectStmt(select) if select.into_clause.is_some())
}

fn drop_targets_catalog(drop: &DropStmt) -> bool {
    i32_to_object_type(drop.remove_type).is_some_and(affects_relations)
}

fn rename_targets_catalog(rename: &RenameStmt) -> bool {
    i32_to_object_type(rename.rename_type)
        .is_some_and(|kind| kind == ObjectType::ObjectColumn || affects_relations(kind))
}

fn affects_relations(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::ObjectTable
            | ObjectType::ObjectView
            | ObjectType::ObjectMatview
            | ObjectType::ObjectSchema
            | ObjectType::ObjectForeignTable
            | ObjectType::ObjectSequence
    )
}

fn i32_to_object_type(value: i32) -> Option<ObjectType> {
    value.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_catalog_mutation_for_drop_table() {
        assert!(mutates_catalog("DROP TABLE IF EXISTS public.demo;"));
    }

    #[test]
    fn ignores_select_queries() {
        assert!(!mutates_catalog("SELECT * FROM public.demo;"));
    }

    #[test]
    fn detects_column_renames_for_tables_and_views() {
        for sql in [
            "ALTER TABLE public.demo RENAME COLUMN old_name TO new_name;",
            "ALTER VIEW public.demo RENAME COLUMN old_name TO new_name;",
            "ALTER MATERIALIZED VIEW public.demo RENAME COLUMN old_name TO new_name;",
        ] {
            assert!(mutates_catalog(sql), "catalog refresh missing for {sql}");
        }
    }

    #[test]
    fn detects_select_into_but_not_an_ordinary_select() {
        assert!(mutates_catalog("SELECT 1 AS id INTO public.new_table;"));
        assert!(!mutates_catalog("SELECT 1 AS id;"));
    }

    #[test]
    fn canceled_refresh_is_requeued() {
        let mut refresh = CatalogRefresh::new();
        refresh.mark_pending();
        refresh.on_dispatched(42);
        assert!(!refresh.ready_to_dispatch(Instant::now()));
        assert!(refresh.on_canceled(42));
        assert!(refresh.ready_to_dispatch(Instant::now()));
    }
}
