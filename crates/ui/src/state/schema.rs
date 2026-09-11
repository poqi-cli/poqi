use poqi_catalog::QualifiedRelation;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum NavigationMode {
    SchemaLevel,
    TableLevel,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SchemaItem {
    Schema {
        name: String,
        expanded: bool,
        table_count: usize,
    },
    Table {
        schema: String,
        name: String,
        is_last_in_schema: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SchemaItemKey {
    Schema(String),
    Table(QualifiedRelation),
}

#[derive(Debug, Clone)]
pub(crate) struct SchemaState {
    schemas: HashMap<String, Vec<String>>,
    expanded: HashSet<String>,
    navigation_mode: NavigationMode,
    pub(crate) items: Vec<SchemaItem>,
    pub(crate) selected: usize,
    pub(crate) last_fetched_table: Option<QualifiedRelation>,
    scroll_top: usize,
    suspend_selection_sync: bool,
}

impl SchemaState {
    pub(crate) fn new(schemas: HashMap<String, Vec<String>>) -> Self {
        let schema_count = schemas.len();
        let mut expanded = HashSet::new();
        let mut desired_selection = None;
        if schema_count == 1 {
            if let Some((schema, tables)) = schemas.iter().next() {
                expanded.insert(schema.clone());
                if let Some(first) = tables.first() {
                    desired_selection = Some(SchemaItemKey::Table(QualifiedRelation::in_schema(
                        schema, first,
                    )));
                }
            }
        }

        let items = Vec::new();
        let navigation_mode = NavigationMode::SchemaLevel;
        Self {
            schemas,
            expanded,
            navigation_mode,
            items,
            selected: 0,
            last_fetched_table: None,
            scroll_top: 0,
            suspend_selection_sync: false,
        }
        .with_rebuilt_items(desired_selection)
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn selected_item(&self) -> Option<&SchemaItem> {
        self.items.get(self.selected)
    }

    pub(crate) fn is_table_level(&self) -> bool {
        self.navigation_mode == NavigationMode::TableLevel
    }

    pub(crate) fn current_schema(&self) -> Option<&str> {
        match self.selected_item() {
            Some(SchemaItem::Table { schema, .. }) => Some(schema.as_str()),
            _ => None,
        }
    }

    pub(crate) fn selected_table_name(&self) -> Option<QualifiedRelation> {
        match self.selected_item() {
            Some(SchemaItem::Table { schema, name, .. }) => {
                Some(QualifiedRelation::in_schema(schema, name))
            }
            _ => None,
        }
    }

    fn item_key(item: &SchemaItem) -> SchemaItemKey {
        match item {
            SchemaItem::Schema { name, .. } => SchemaItemKey::Schema(name.clone()),
            SchemaItem::Table { schema, name, .. } => {
                SchemaItemKey::Table(QualifiedRelation::in_schema(schema, name))
            }
        }
    }

    pub(crate) fn drill_down(&mut self) -> bool {
        if let Some(SchemaItem::Schema {
            name, table_count, ..
        }) = self.selected_item().cloned()
        {
            if table_count == 0 {
                return false;
            }
            if self.expanded.insert(name.clone()) {
                let desired = self
                    .schemas
                    .get(&name)
                    .and_then(|tables| tables.first())
                    .map(|table| SchemaItemKey::Table(QualifiedRelation::in_schema(&name, table)));
                self.rebuild_items(desired);
                return true;
            }
            // If already expanded, collapse and keep selection on the schema row.
            self.expanded.remove(&name);
            self.rebuild_items(Some(SchemaItemKey::Schema(name.clone())));
            return true;
        }
        false
    }

    pub(crate) fn go_back(&mut self) -> bool {
        match self.selected_item().cloned() {
            Some(SchemaItem::Table { schema, .. }) => {
                self.rebuild_items(Some(SchemaItemKey::Schema(schema)));
                self.last_fetched_table = None;
                true
            }
            Some(SchemaItem::Schema { name, .. }) if self.expanded.remove(&name) => {
                self.rebuild_items(Some(SchemaItemKey::Schema(name)));
                true
            }
            Some(SchemaItem::Schema { .. }) | None => false,
        }
    }

    pub(crate) fn should_auto_fetch(&self) -> bool {
        self.selected_table_name().is_some_and(|table| {
            !table.requires_explicit_preview() && self.last_fetched_table.as_ref() != Some(&table)
        })
    }

    pub(crate) fn selected_table_requires_explicit_preview(&self) -> bool {
        self.selected_table_name()
            .is_some_and(|table| table.requires_explicit_preview())
    }

    pub(crate) fn mark_auto_fetch_dispatched(&mut self, table: &QualifiedRelation) {
        self.last_fetched_table = Some(table.clone());
    }

    pub(crate) fn move_up(&mut self) {
        if self.selected > 0 {
            self.select_index(self.selected - 1);
        }
    }

    pub(crate) fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.select_index(self.selected + 1);
        }
    }

    pub(crate) fn page_up(&mut self) {
        self.select_index(self.selected.saturating_sub(5));
    }

    pub(crate) fn page_down(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.select_index((self.selected + 5).min(len - 1));
    }

    pub(crate) fn jump_start(&mut self) {
        self.select_index(0);
    }

    pub(crate) fn jump_end(&mut self) {
        if !self.items.is_empty() {
            self.select_index(self.items.len() - 1);
        }
    }

    pub(crate) fn ensure_selected_visible(&mut self, viewport_rows: usize) {
        if self.items.is_empty() || viewport_rows == 0 {
            self.scroll_top = 0;
            return;
        }
        let viewport = viewport_rows.max(1);
        let max_scroll = self.items.len().saturating_sub(viewport);
        let mut scroll_top = self.scroll_top.min(max_scroll);
        if !self.suspend_selection_sync {
            if self.selected < scroll_top {
                scroll_top = self.selected;
            } else {
                let max_visible = scroll_top.saturating_add(viewport.saturating_sub(1));
                if self.selected > max_visible {
                    scroll_top = self.selected.saturating_add(1).saturating_sub(viewport);
                }
            }
        }
        self.scroll_top = scroll_top.min(max_scroll);
    }

    pub(crate) fn set_manual_scroll_top(&mut self, top: usize, viewport_rows: usize) {
        let viewport = viewport_rows.max(1);
        let max_scroll = self.items.len().saturating_sub(viewport);
        self.scroll_top = top.min(max_scroll);
        self.suspend_selection_sync = true;
    }

    pub(crate) fn scroll_offset(&self) -> usize {
        self.scroll_top
    }

    pub(crate) fn select_index(&mut self, index: usize) {
        if self.items.is_empty() {
            self.selected = 0;
        } else {
            self.selected = index.min(self.items.len() - 1);
        }
        self.navigation_mode = match self.selected_item() {
            Some(SchemaItem::Table { .. }) => NavigationMode::TableLevel,
            _ => NavigationMode::SchemaLevel,
        };
        self.suspend_selection_sync = false;
    }

    fn rebuild_items(&mut self, desired_selection: Option<SchemaItemKey>) {
        let current_label = desired_selection.or_else(|| self.selected_item().map(Self::item_key));

        let mut schema_names: Vec<String> = self.schemas.keys().cloned().collect();
        schema_names.sort();

        let mut items = Vec::new();
        for schema in &schema_names {
            let tables = self.schemas.get(schema).cloned().unwrap_or_default();
            let expanded = self.expanded.contains(schema);
            let table_count = tables.len();
            items.push(SchemaItem::Schema {
                name: schema.clone(),
                expanded,
                table_count,
            });
            if expanded && !tables.is_empty() {
                for (idx, table) in tables.iter().enumerate() {
                    let is_last_in_schema = idx + 1 == tables.len();
                    items.push(SchemaItem::Table {
                        schema: schema.clone(),
                        name: table.clone(),
                        is_last_in_schema,
                    });
                }
            }
        }

        let selected = current_label
            .and_then(|label| items.iter().position(|item| Self::item_key(item) == label))
            .unwrap_or(0);

        self.items = items;
        self.select_index(selected);
    }

    fn with_rebuilt_items(mut self, desired_selection: Option<SchemaItemKey>) -> Self {
        self.rebuild_items(desired_selection);
        self
    }

    pub(crate) fn replace(&mut self, schemas: HashMap<String, Vec<String>>) {
        let current_label = self.selected_item().map(Self::item_key);
        self.schemas = schemas;
        self.expanded
            .retain(|schema| self.schemas.contains_key(schema));
        self.last_fetched_table = None;

        let desired_selection = if self.schemas.len() == 1 && self.expanded.is_empty() {
            self.schemas
                .iter()
                .next()
                .and_then(|(schema, tables)| tables.first().map(|table| (schema, table)))
                .map(|(schema, table)| {
                    SchemaItemKey::Table(QualifiedRelation::in_schema(schema, table))
                })
                .or(current_label)
        } else {
            current_label
        };

        self.rebuild_items(desired_selection);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deceptive_relation_is_selectable_but_never_auto_fetched() {
        let relation_name = "audit\u{202E}cod";
        let mut schemas = HashMap::new();
        schemas.insert("public".to_string(), vec![relation_name.to_string()]);
        let state = SchemaState::new(schemas);

        let selected = state.selected_table_name().expect("selected relation");
        assert_eq!(
            selected,
            QualifiedRelation::in_schema("public", relation_name)
        );
        assert_eq!(selected.quoted(), "\"public\".\"audit\u{202E}cod\"");
        assert!(state.selected_table_requires_explicit_preview());
        assert!(!state.should_auto_fetch());
        assert!(state.last_fetched_table.is_none());
    }

    #[test]
    fn deceptive_schema_also_disables_automatic_preview() {
        let mut schemas = HashMap::new();
        schemas.insert("hidden\nline".to_string(), vec!["events".to_string()]);
        let state = SchemaState::new(schemas);

        assert!(state.selected_table_requires_explicit_preview());
        assert!(!state.should_auto_fetch());
    }

    #[test]
    fn ordinary_relation_keeps_existing_automatic_preview() {
        let mut schemas = HashMap::new();
        schemas.insert("public".to_string(), vec!["event\\archive".to_string()]);
        let state = SchemaState::new(schemas);

        assert!(!state.selected_table_requires_explicit_preview());
        assert!(state.should_auto_fetch());
    }
}
