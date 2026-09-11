use pg_query::{
    parse,
    protobuf::{
        ColumnRef, Node, RangeVar, ResTarget, SelectStmt, String as PgString, TypeCast, TypeName,
    },
    NodeEnum,
};
use poqi_catalog::{quote_identifier, QualifiedRelation};

use crate::constants::PRIMARY_KEY_ALIAS_PREFIX;
use crate::{EngineError, ResultOrigin, RunSqlRefresh, CTID_ALIAS, TABLEOID_ALIAS, XMIN_ALIAS};

#[derive(Debug)]
pub(crate) struct RunSqlPlan {
    pub editable_sql: Option<String>,
    pub source_columns: Option<SourceProjection>,
    pub origin: ResultOrigin,
    pub source_table: Option<QualifiedRelation>,
}

#[derive(Debug)]
pub(crate) enum SourceProjection {
    AllColumns,
    Named(Vec<String>),
}

impl SourceProjection {
    pub(crate) fn resolve(self, result_columns: &[String]) -> Vec<Option<String>> {
        match self {
            Self::AllColumns => result_columns.iter().cloned().map(Some).collect(),
            Self::Named(columns) if columns.len() == result_columns.len() => {
                columns.into_iter().map(Some).collect()
            }
            Self::Named(_) => vec![None; result_columns.len()],
        }
    }
}

pub(crate) fn plan_run_sql(sql: &str) -> Result<RunSqlPlan, EngineError> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Ok(unknown_plan());
    }

    let Ok(parse_result) = parse(trimmed) else {
        return Ok(unknown_plan());
    };
    let statement_count = parse_result.protobuf.stmts.len();
    if statement_count != 1 {
        return Err(EngineError::UnsupportedSqlBatch(statement_count));
    }

    let Some(node) = parse_result
        .protobuf
        .stmts
        .first()
        .and_then(|stmt| stmt.stmt.as_ref())
        .and_then(|node| node.node.as_ref())
    else {
        return Ok(unknown_plan());
    };
    let NodeEnum::SelectStmt(select) = node else {
        return Ok(unknown_plan());
    };

    Ok(analyze_single_table_select(trimmed, select).unwrap_or_else(unknown_plan))
}

fn unknown_plan() -> RunSqlPlan {
    RunSqlPlan {
        editable_sql: None,
        source_columns: None,
        origin: ResultOrigin::Unknown,
        source_table: None,
    }
}

fn analyze_single_table_select(trimmed: &str, select: &SelectStmt) -> Option<RunSqlPlan> {
    if select.into_clause.is_some()
        || select.with_clause.is_some()
        || !select.values_lists.is_empty()
        || has_set_operation(select)
    {
        return None;
    }

    let range = extract_single_range(select)?;
    let table = QualifiedRelation {
        schema: (!range.schemaname.is_empty()).then(|| range.schemaname.clone()),
        name: range.relname.clone(),
    };
    let refresh = if is_simple_select(select) {
        RunSqlRefresh::SimpleSingleTable {
            table: table.clone(),
        }
    } else {
        RunSqlRefresh::ComplexSingleTable {
            table: table.clone(),
        }
    };
    let source_columns = editable_projection(select, range);
    let editable_sql = source_columns
        .as_ref()
        .and_then(|_| rewrite_with_row_identity(select, range, &[]));

    Some(RunSqlPlan {
        editable_sql,
        source_columns,
        origin: ResultOrigin::RunSql {
            sql: trimmed.to_string(),
            refresh,
        },
        source_table: Some(table),
    })
}

fn has_set_operation(select: &SelectStmt) -> bool {
    select.op != pg_query::protobuf::SetOperation::SetopNone as i32
        || select.larg.is_some()
        || select.rarg.is_some()
}

fn extract_single_range(select: &SelectStmt) -> Option<&RangeVar> {
    let mut from_iter = select.from_clause.iter();
    let first = from_iter.next()?;
    if from_iter.next().is_some() {
        return None;
    }
    let NodeEnum::RangeVar(range) = first.node.as_ref()? else {
        return None;
    };
    (!range.relname.is_empty()).then_some(range)
}

fn editable_projection(select: &SelectStmt, range: &RangeVar) -> Option<SourceProjection> {
    if !select.distinct_clause.is_empty()
        || select.where_clause.is_some()
        || !select.group_clause.is_empty()
        || select.group_distinct
        || select.having_clause.is_some()
        || !select.window_clause.is_empty()
        || !select.sort_clause.is_empty()
        || select.limit_offset.is_some()
        || select.limit_count.is_some()
        || !select.locking_clause.is_empty()
        || select.target_list.is_empty()
        || range
            .alias
            .as_ref()
            .is_some_and(|alias| !alias.colnames.is_empty())
    {
        return None;
    }

    if select.target_list.len() == 1 && res_target_is_star(&select.target_list[0], range) {
        return Some(SourceProjection::AllColumns);
    }

    select
        .target_list
        .iter()
        .map(|target| direct_source_column(target, range))
        .collect::<Option<Vec<_>>>()
        .map(SourceProjection::Named)
}

pub(crate) fn quote_type_name(input: &str) -> Option<String> {
    let parse_result = parse(&format!("SELECT NULL::{input}")).ok()?;
    let statement = parse_result.protobuf.stmts.as_slice();
    let [statement] = statement else {
        return None;
    };
    let NodeEnum::SelectStmt(select) = statement.stmt.as_ref()?.node.as_ref()? else {
        return None;
    };
    if !plain_type_name_select(select) {
        return None;
    }
    let [target] = select.target_list.as_slice() else {
        return None;
    };
    let NodeEnum::ResTarget(target) = target.node.as_ref()? else {
        return None;
    };
    if !target.name.is_empty() || !target.indirection.is_empty() {
        return None;
    }
    let NodeEnum::TypeCast(type_cast) = target.val.as_ref()?.node.as_ref()? else {
        return None;
    };
    let NodeEnum::AConst(null) = type_cast.arg.as_ref()?.node.as_ref()? else {
        return None;
    };
    if !null.isnull || null.val.is_some() {
        return None;
    }
    let type_name = type_cast.type_name.as_ref()?;
    if type_name.setof
        || type_name.pct_type
        || !type_name.typmods.is_empty()
        || !type_name.array_bounds.is_empty()
    {
        return None;
    }
    let components = type_name
        .names
        .iter()
        .map(|name| match name.node.as_ref() {
            Some(NodeEnum::String(value)) if !value.sval.is_empty() => Some(value.sval.as_str()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if !(1..=2).contains(&components.len()) {
        return None;
    }
    Some(
        components
            .into_iter()
            .map(quote_identifier)
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn plain_type_name_select(select: &SelectStmt) -> bool {
    select.distinct_clause.is_empty()
        && select.into_clause.is_none()
        && select.from_clause.is_empty()
        && select.where_clause.is_none()
        && select.group_clause.is_empty()
        && !select.group_distinct
        && select.having_clause.is_none()
        && select.window_clause.is_empty()
        && select.values_lists.is_empty()
        && select.sort_clause.is_empty()
        && select.limit_offset.is_none()
        && select.limit_count.is_none()
        && select.locking_clause.is_empty()
        && select.with_clause.is_none()
        && !has_set_operation(select)
}

fn direct_source_column(target: &Node, range: &RangeVar) -> Option<String> {
    let NodeEnum::ResTarget(target) = target.node.as_ref()? else {
        return None;
    };
    if !target.indirection.is_empty() {
        return None;
    }
    let NodeEnum::ColumnRef(column) = target.val.as_ref()?.node.as_ref()? else {
        return None;
    };
    let fields = column
        .fields
        .iter()
        .map(|field| match field.node.as_ref() {
            Some(NodeEnum::String(value)) => Some(value.sval.as_str()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let column_name = match fields.as_slice() {
        [column] => *column,
        [qualifier, column] if qualifier_matches_range(qualifier, range) => *column,
        _ => return None,
    };
    (!is_system_column(column_name)).then(|| column_name.to_string())
}

fn qualifier_matches_range(qualifier: &str, range: &RangeVar) -> bool {
    range
        .alias
        .as_ref()
        .map_or(qualifier == range.relname, |alias| {
            qualifier == alias.aliasname
        })
}

fn is_system_column(column: &str) -> bool {
    matches!(
        column,
        "tableoid" | "xmin" | "cmin" | "xmax" | "cmax" | "ctid"
    )
}

pub(crate) fn rewrite_with_primary_key_identity(
    sql: &str,
    primary_key_columns: &[String],
) -> Option<String> {
    let parse_result = parse(sql).ok()?;
    let select = parse_result
        .protobuf
        .stmts
        .first()?
        .stmt
        .as_ref()?
        .node
        .as_ref()?;
    let NodeEnum::SelectStmt(select) = select else {
        return None;
    };
    let range = extract_single_range(select)?;
    rewrite_with_row_identity(select, range, primary_key_columns)
}

fn rewrite_with_row_identity(
    select: &SelectStmt,
    range: &RangeVar,
    primary_key_columns: &[String],
) -> Option<String> {
    let qualifier = range
        .alias
        .as_ref()
        .map_or_else(|| range.relname.clone(), |alias| alias.aliasname.clone());
    let mut cloned = select.clone();
    cloned.target_list.push(build_identity_res_target(
        &qualifier,
        "tableoid",
        "oid",
        TABLEOID_ALIAS,
    ));
    cloned.target_list.push(build_identity_res_target(
        &qualifier, "ctid", "text", CTID_ALIAS,
    ));
    cloned.target_list.push(build_identity_res_target(
        &qualifier, "xmin", "text", XMIN_ALIAS,
    ));
    for (index, column) in primary_key_columns.iter().enumerate() {
        cloned.target_list.push(build_column_res_target(
            &qualifier,
            column,
            &format!("{PRIMARY_KEY_ALIAS_PREFIX}{index}"),
        ));
    }
    NodeEnum::SelectStmt(Box::new(cloned)).deparse().ok()
}

fn build_column_res_target(qualifier: &str, column: &str, alias: &str) -> Node {
    Node {
        node: Some(NodeEnum::ResTarget(Box::new(ResTarget {
            name: alias.to_string(),
            indirection: Vec::new(),
            val: Some(Box::new(Node {
                node: Some(NodeEnum::ColumnRef(ColumnRef {
                    fields: vec![pg_string_node(qualifier), pg_string_node(column)],
                    location: -1,
                })),
            })),
            location: -1,
        }))),
    }
}

fn build_identity_res_target(qualifier: &str, column: &str, cast_type: &str, alias: &str) -> Node {
    let column_ref = Node {
        node: Some(NodeEnum::ColumnRef(ColumnRef {
            fields: vec![pg_string_node(qualifier), pg_string_node(column)],
            location: -1,
        })),
    };
    let type_name = TypeName {
        names: vec![pg_string_node("pg_catalog"), pg_string_node(cast_type)],
        type_oid: 0,
        setof: false,
        pct_type: false,
        typmods: Vec::new(),
        typemod: -1,
        array_bounds: Vec::new(),
        location: -1,
    };
    let type_cast = TypeCast {
        arg: Some(Box::new(column_ref)),
        type_name: Some(type_name),
        location: -1,
    };
    Node {
        node: Some(NodeEnum::ResTarget(Box::new(ResTarget {
            name: alias.to_string(),
            indirection: Vec::new(),
            val: Some(Box::new(Node {
                node: Some(NodeEnum::TypeCast(Box::new(type_cast))),
            })),
            location: -1,
        }))),
    }
}

fn pg_string_node(value: &str) -> Node {
    Node {
        node: Some(NodeEnum::String(PgString {
            sval: value.to_string(),
        })),
    }
}

fn is_simple_select(select: &SelectStmt) -> bool {
    projection_is_star_only(select)
        && select.where_clause.is_none()
        && select.group_clause.is_empty()
        && select.having_clause.is_none()
        && select.sort_clause.is_empty()
        && select.window_clause.is_empty()
        && select.distinct_clause.is_empty()
        && select.with_clause.is_none()
        && select.values_lists.is_empty()
}

fn projection_is_star_only(select: &SelectStmt) -> bool {
    !select.target_list.is_empty()
        && select.target_list.iter().all(|target| {
            let Some(NodeEnum::ResTarget(target)) = target.node.as_ref() else {
                return false;
            };
            let Some(NodeEnum::ColumnRef(column)) =
                target.val.as_ref().and_then(|value| value.node.as_ref())
            else {
                return false;
            };
            column
                .fields
                .last()
                .is_some_and(|field| matches!(field.node.as_ref(), Some(NodeEnum::AStar(_))))
        })
}

fn res_target_is_star(node: &Node, range: &RangeVar) -> bool {
    let Some(NodeEnum::ResTarget(target)) = node.node.as_ref() else {
        return false;
    };
    let Some(NodeEnum::ColumnRef(column)) = target.val.as_ref().and_then(|val| val.node.as_ref())
    else {
        return false;
    };
    match column.fields.as_slice() {
        [star] => matches!(star.node.as_ref(), Some(NodeEnum::AStar(_))),
        [qualifier, star] => {
            matches!(star.node.as_ref(), Some(NodeEnum::AStar(_)))
                && matches!(qualifier.node.as_ref(), Some(NodeEnum::String(value)) if qualifier_matches_range(&value.sval, range))
        }
        _ => false,
    }
}
