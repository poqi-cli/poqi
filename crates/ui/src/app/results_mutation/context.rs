use poqi_catalog::QualifiedRelation;
use poqi_engine::RowIdentity;

#[derive(Debug, Clone)]
pub(crate) struct MutationContext {
    pub(crate) table: QualifiedRelation,
    pub(crate) row_identity: Option<RowIdentity>,
    pub(crate) row: Option<usize>,
    pub(crate) column: Option<usize>,
}

impl MutationContext {
    pub(super) fn pending_focus(&self) -> Option<PendingFocus> {
        let row_identity = self.row_identity.clone()?;
        let column = self.column.unwrap_or(0);
        Some(PendingFocus {
            table: self.table.clone(),
            row_identity,
            column,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PendingFocus {
    pub(super) table: QualifiedRelation,
    pub(super) row_identity: RowIdentity,
    pub(super) column: usize,
}
