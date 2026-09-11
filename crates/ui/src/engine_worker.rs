use std::{fmt::Display, future::Future, sync::Arc, time::Duration};

use poqi_catalog::CatalogSnapshot;
use poqi_engine::{CrudAction, Engine, QueryResult};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

const CANCELLATION_TIMEOUT: Duration = Duration::from_secs(5);
const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Debug)]
pub(super) enum EngineCommand {
    RunSql {
        request_id: u64,
        sql: String,
        kind: EngineRequestKind,
    },
    Crud {
        request_id: u64,
        action: CrudAction,
        kind: EngineRequestKind,
    },
    RefreshCatalog {
        request_id: u64,
    },
    CancelActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EngineRequestKind {
    RunSql,
    SelectTop,
    UpdateCell,
    DeleteRow,
    RowRefresh,
    SemanticSearch,
    CatalogRefresh,
}

#[derive(Debug)]
pub(super) enum EngineResponse {
    Success {
        request_id: u64,
        kind: EngineRequestKind,
        result: Box<QueryResult>,
    },
    CatalogRefreshed {
        request_id: u64,
        snapshot: CatalogSnapshot,
    },
    Error {
        request_id: u64,
        kind: EngineRequestKind,
        message: String,
    },
    Canceled {
        request_id: u64,
        kind: EngineRequestKind,
        message: String,
    },
}

pub(super) fn spawn_engine_worker(
    engine: Arc<dyn Engine>,
    mut commands: UnboundedReceiver<EngineCommand>,
    responses: UnboundedSender<EngineResponse>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut active: Option<ActiveTask> = None;
        loop {
            if let Some(mut running) = active.take() {
                tokio::select! {
                    command = commands.recv() => {
                        let Some(command) = command else {
                            cancel_active_task(&engine, running, &responses).await;
                            break;
                        };
                        active = handle_command(
                            command,
                            Some(running),
                            &engine,
                            &responses,
                        ).await;
                    }
                    outcome = &mut running.handle => {
                        send_task_outcome(running.request_id, running.kind, outcome, &responses);
                    }
                }
            } else {
                let Some(command) = commands.recv().await else {
                    break;
                };
                active = handle_command(command, None, &engine, &responses).await;
            }
        }
    })
}

struct ActiveTask {
    request_id: u64,
    kind: EngineRequestKind,
    handle: JoinHandle<EngineResponse>,
}

async fn handle_command(
    command: EngineCommand,
    active: Option<ActiveTask>,
    engine: &Arc<dyn Engine>,
    responses: &UnboundedSender<EngineResponse>,
) -> Option<ActiveTask> {
    handle_command_with_timeout(command, active, engine, responses, CANCELLATION_TIMEOUT).await
}

async fn handle_command_with_timeout(
    command: EngineCommand,
    active: Option<ActiveTask>,
    engine: &Arc<dyn Engine>,
    responses: &UnboundedSender<EngineResponse>,
    cancellation_timeout: Duration,
) -> Option<ActiveTask> {
    if let Some(running) = active {
        cancel_active_task_with_timeout(engine, running, responses, cancellation_timeout).await;
    }
    match command {
        EngineCommand::RunSql {
            request_id,
            sql,
            kind,
        } => Some(spawn_sql_task(request_id, sql, kind, Arc::clone(engine))),
        EngineCommand::Crud {
            request_id,
            action,
            kind,
        } => Some(spawn_crud_task(
            request_id,
            action,
            kind,
            Arc::clone(engine),
        )),
        EngineCommand::RefreshCatalog { request_id } => {
            Some(spawn_catalog_task(request_id, Arc::clone(engine)))
        }
        EngineCommand::CancelActive => None,
    }
}

async fn cancel_active_task(
    engine: &Arc<dyn Engine>,
    active: ActiveTask,
    responses: &UnboundedSender<EngineResponse>,
) {
    cancel_active_task_with_timeout(engine, active, responses, CANCELLATION_TIMEOUT).await;
}

async fn cancel_active_task_with_timeout(
    engine: &Arc<dyn Engine>,
    active: ActiveTask,
    responses: &UnboundedSender<EngineResponse>,
    timeout: Duration,
) {
    let cancellation_engine = Arc::clone(engine);
    cancel_active_task_with(active, responses, timeout, move || {
        let engine = Arc::clone(&cancellation_engine);
        async move { engine.cancel_current().await }
    })
    .await;
}

async fn cancel_active_task_with<F, Fut, E>(
    mut active: ActiveTask,
    responses: &UnboundedSender<EngineResponse>,
    timeout: Duration,
    mut cancel_current: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<bool, E>>,
    E: Display,
{
    let deadline = tokio::time::Instant::now() + timeout;
    let mut cancellation_ready = true;
    loop {
        tokio::select! {
            biased;
            outcome = &mut active.handle => {
                send_task_outcome(active.request_id, active.kind, outcome, responses);
                return;
            }
            () = tokio::time::sleep_until(deadline) => {
                abort_with_unknown_outcome(
                    active,
                    "the cancellation attempt did not finish before its deadline".to_string(),
                    responses,
                    false,
                )
                .await;
                return;
            }
            () = tokio::time::sleep(CANCELLATION_POLL_INTERVAL), if !cancellation_ready => {
                cancellation_ready = true;
            }
            cancellation = cancel_current(), if cancellation_ready => match cancellation {
                Ok(true) => {
                    settle_after_cancellation_delivery_until(active, responses, deadline).await;
                    return;
                }
                Ok(false) => cancellation_ready = false,
                Err(err) => {
                    abort_with_unknown_outcome(
                        active,
                        format!("server cancellation failed: {err}"),
                        responses,
                        false,
                    )
                    .await;
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
async fn settle_after_cancellation_delivery(
    active: ActiveTask,
    responses: &UnboundedSender<EngineResponse>,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    settle_after_cancellation_delivery_until(active, responses, deadline).await;
}

async fn settle_after_cancellation_delivery_until(
    mut active: ActiveTask,
    responses: &UnboundedSender<EngineResponse>,
    deadline: tokio::time::Instant,
) {
    match tokio::time::timeout_at(deadline, &mut active.handle).await {
        Ok(outcome) => {
            send_cancellation_outcome(active.request_id, active.kind, outcome, responses);
        }
        Err(_) => {
            abort_with_unknown_outcome(
                active,
                "the operation did not finish after server cancellation was requested".to_string(),
                responses,
                true,
            )
            .await;
        }
    }
}

async fn abort_with_unknown_outcome(
    active: ActiveTask,
    reason: String,
    responses: &UnboundedSender<EngineResponse>,
    cancellation_delivered: bool,
) {
    active.handle.abort();
    match active.handle.await {
        Ok(response) => {
            // A task that completed concurrently with abort still owns the authoritative
            // database result, including a successful commit.
            if cancellation_delivered {
                send_cancellation_outcome(active.request_id, active.kind, Ok(response), responses);
            } else {
                send_task_outcome(active.request_id, active.kind, Ok(response), responses);
            }
        }
        Err(err) if err.is_cancelled() => {
            let _ = responses.send(EngineResponse::Error {
                request_id: active.request_id,
                kind: active.kind,
                message: format!("{reason}; the database operation outcome is unknown"),
            });
        }
        Err(err) => send_task_outcome(active.request_id, active.kind, Err(err), responses),
    }
}

fn send_task_outcome(
    request_id: u64,
    kind: EngineRequestKind,
    outcome: Result<EngineResponse, tokio::task::JoinError>,
    responses: &UnboundedSender<EngineResponse>,
) {
    let response = match outcome {
        Ok(EngineResponse::Canceled { request_id, kind, message }) => EngineResponse::Error {
            request_id,
            kind,
            message,
        },
        Ok(response) => response,
        Err(err) if err.is_panic() => EngineResponse::Error {
            request_id,
            kind,
            message:
                "engine worker panicked while processing the request; the database operation outcome is unknown"
                    .to_string(),
        },
        Err(err) => EngineResponse::Error {
            request_id,
            kind,
            message: format!(
                "engine worker stopped unexpectedly: {err}; the database operation outcome is unknown"
            ),
        },
    };
    let _ = responses.send(response);
}

fn send_cancellation_outcome(
    request_id: u64,
    kind: EngineRequestKind,
    outcome: Result<EngineResponse, tokio::task::JoinError>,
    responses: &UnboundedSender<EngineResponse>,
) {
    // SQLSTATE identifies an aborted statement independently of server language.
    // Only a delivered cancellation permits normal cancellation handling. If a
    // server timeout races that delivery, PostgreSQL does not expose a separate
    // locale-independent reason; the statement is nevertheless confirmed aborted.
    match outcome {
        Ok(response @ EngineResponse::Canceled { .. }) => {
            let _ = responses.send(response);
        }
        other => send_task_outcome(request_id, kind, other, responses),
    }
}

fn operation_error(
    request_id: u64,
    kind: EngineRequestKind,
    error: &anyhow::Error,
) -> EngineResponse {
    let message = error.to_string();
    if poqi_engine::is_query_canceled(error) {
        EngineResponse::Canceled {
            request_id,
            kind,
            message,
        }
    } else {
        EngineResponse::Error {
            request_id,
            kind,
            message,
        }
    }
}

fn spawn_sql_task(
    request_id: u64,
    sql: String,
    kind: EngineRequestKind,
    engine: Arc<dyn Engine>,
) -> ActiveTask {
    let handle = tokio::spawn(async move {
        match engine.execute(&sql).await {
            Ok(result) => EngineResponse::Success {
                request_id,
                kind,
                result: Box::new(result),
            },
            Err(err) => operation_error(request_id, kind, &err),
        }
    });
    ActiveTask {
        request_id,
        kind,
        handle,
    }
}

fn spawn_crud_task(
    request_id: u64,
    action: CrudAction,
    kind: EngineRequestKind,
    engine: Arc<dyn Engine>,
) -> ActiveTask {
    let handle = tokio::spawn(async move {
        match engine.crud(action).await {
            Ok(result) => EngineResponse::Success {
                request_id,
                kind,
                result: Box::new(result),
            },
            Err(err) => operation_error(request_id, kind, &err),
        }
    });
    ActiveTask {
        request_id,
        kind,
        handle,
    }
}

fn spawn_catalog_task(request_id: u64, engine: Arc<dyn Engine>) -> ActiveTask {
    let kind = EngineRequestKind::CatalogRefresh;
    let handle = tokio::spawn(async move {
        match engine.refresh_catalog().await {
            Ok(snapshot) => EngineResponse::CatalogRefreshed {
                request_id,
                snapshot,
            },
            Err(err) => operation_error(request_id, kind, &err),
        }
    });
    ActiveTask {
        request_id,
        kind,
        handle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poqi_engine::StubEngine;
    use std::sync::atomic::{AtomicBool, Ordering};

    const TEST_CANCELLATION_TIMEOUT: Duration = Duration::from_millis(25);

    fn success_response(request_id: u64, kind: EngineRequestKind) -> EngineResponse {
        EngineResponse::Success {
            request_id,
            kind,
            result: Box::new(QueryResult::empty()),
        }
    }

    fn panic_response() -> EngineResponse {
        panic!("boom")
    }

    #[tokio::test]
    async fn requested_server_cancellation_is_a_normal_outcome() {
        let kind = EngineRequestKind::RunSql;
        let active = ActiveTask {
            request_id: 12,
            kind,
            handle: tokio::spawn(async move {
                EngineResponse::Canceled {
                    request_id: 12,
                    kind,
                    message: "kysely peruutettu".into(),
                }
            }),
        };
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        settle_after_cancellation_delivery(active, &responses, TEST_CANCELLATION_TIMEOUT).await;
        assert!(matches!(
            response_rx.recv().await,
            Some(EngineResponse::Canceled { request_id: 12, .. })
        ));
    }

    #[test]
    fn unsolicited_server_cancellation_remains_an_error() {
        let kind = EngineRequestKind::RunSql;
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        send_task_outcome(
            13,
            kind,
            Ok(EngineResponse::Canceled {
                request_id: 13,
                kind,
                message: "canceling statement due to user request".into(),
            }),
            &responses,
        );
        assert!(matches!(
            response_rx.try_recv(),
            Ok(EngineResponse::Error { request_id: 13, .. })
        ));
    }

    #[tokio::test]
    async fn cancellation_before_delivery_does_not_hide_statement_timeout() {
        let kind = EngineRequestKind::RunSql;
        let handle = tokio::spawn(async move {
            EngineResponse::Canceled {
                request_id: 14,
                kind,
                message: "canceling statement due to statement timeout".into(),
            }
        });
        while !handle.is_finished() {
            tokio::task::yield_now().await;
        }
        let active = ActiveTask {
            request_id: 14,
            kind,
            handle,
        };
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        let cancellation_called = AtomicBool::new(false);
        cancel_active_task_with(active, &responses, TEST_CANCELLATION_TIMEOUT, || async {
            cancellation_called.store(true, Ordering::Release);
            Ok::<bool, &'static str>(true)
        })
        .await;
        assert!(!cancellation_called.load(Ordering::Acquire));
        assert!(matches!(
            response_rx.recv().await,
            Some(EngineResponse::Error { request_id: 14, .. })
        ));
    }

    #[tokio::test]
    async fn task_panic_is_reported_for_the_active_request() {
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        let outcome = tokio::spawn(async { panic_response() }).await;

        send_task_outcome(17, EngineRequestKind::RunSql, outcome, &responses);

        match response_rx.recv().await.expect("panic response") {
            EngineResponse::Error {
                request_id,
                kind,
                message,
            } => {
                assert_eq!(request_id, 17);
                assert_eq!(kind, EngineRequestKind::RunSql);
                assert!(message.contains("panicked"));
                assert!(message.contains("outcome is unknown"));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancellation_waits_when_operation_has_not_registered_yet() {
        let engine: Arc<dyn Engine> = Arc::new(StubEngine);
        let completed = Arc::new(AtomicBool::new(false));
        let task_completed = Arc::clone(&completed);
        let handle = tokio::spawn(async move {
            for _ in 0..10 {
                tokio::task::yield_now().await;
            }
            task_completed.store(true, Ordering::Release);
            success_response(23, EngineRequestKind::SelectTop)
        });
        let active = ActiveTask {
            request_id: 23,
            kind: EngineRequestKind::SelectTop,
            handle,
        };
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();

        cancel_active_task(&engine, active, &responses).await;

        assert!(completed.load(Ordering::Acquire));
        assert!(matches!(
            response_rx.recv().await,
            Some(EngineResponse::Success {
                request_id: 23,
                kind: EngineRequestKind::SelectTop,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn cancellation_delivery_does_not_hide_successful_write_commit() {
        let handle = tokio::spawn(async {
            tokio::task::yield_now().await;
            success_response(41, EngineRequestKind::UpdateCell)
        });
        let active = ActiveTask {
            request_id: 41,
            kind: EngineRequestKind::UpdateCell,
            handle,
        };
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();

        settle_after_cancellation_delivery(active, &responses, TEST_CANCELLATION_TIMEOUT).await;

        assert!(matches!(
            response_rx.recv().await,
            Some(EngineResponse::Success {
                request_id: 41,
                kind: EngineRequestKind::UpdateCell,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn cancellation_delivery_preserves_unknown_operation_error() {
        let expected =
            "database connection was lost; the operation outcome is unknown and was not retried";
        let handle = tokio::spawn(async move {
            EngineResponse::Error {
                request_id: 43,
                kind: EngineRequestKind::DeleteRow,
                message: expected.to_string(),
            }
        });
        let active = ActiveTask {
            request_id: 43,
            kind: EngineRequestKind::DeleteRow,
            handle,
        };
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();

        settle_after_cancellation_delivery(active, &responses, TEST_CANCELLATION_TIMEOUT).await;

        match response_rx.recv().await.expect("operation outcome") {
            EngineResponse::Error {
                request_id,
                kind,
                message,
            } => {
                assert_eq!(request_id, 43);
                assert_eq!(kind, EngineRequestKind::DeleteRow);
                assert_eq!(message, expected);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancellation_settle_timeout_reports_unknown_outcome() {
        let handle = tokio::spawn(async {
            std::future::pending::<()>().await;
            success_response(47, EngineRequestKind::RunSql)
        });
        let active = ActiveTask {
            request_id: 47,
            kind: EngineRequestKind::RunSql,
            handle,
        };
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();

        settle_after_cancellation_delivery(active, &responses, TEST_CANCELLATION_TIMEOUT).await;

        match response_rx.recv().await.expect("timeout response") {
            EngineResponse::Error {
                request_id,
                kind,
                message,
            } => {
                assert_eq!(request_id, 47);
                assert_eq!(kind, EngineRequestKind::RunSql);
                assert!(message.contains("outcome is unknown"));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn permanently_unregistered_operation_does_not_block_replacement_or_shutdown() {
        let engine: Arc<dyn Engine> = Arc::new(StubEngine);
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        let active = ActiveTask {
            request_id: 51,
            kind: EngineRequestKind::RunSql,
            handle: tokio::spawn(async {
                std::future::pending::<()>().await;
                success_response(51, EngineRequestKind::RunSql)
            }),
        };

        let replacement = tokio::time::timeout(
            Duration::from_secs(1),
            handle_command_with_timeout(
                EngineCommand::RunSql {
                    request_id: 52,
                    sql: "SELECT 1".to_string(),
                    kind: EngineRequestKind::RunSql,
                },
                Some(active),
                &engine,
                &responses,
                TEST_CANCELLATION_TIMEOUT,
            ),
        )
        .await
        .expect("replacement must not wait indefinitely")
        .expect("replacement task");

        match response_rx.recv().await.expect("superseded response") {
            EngineResponse::Error {
                request_id,
                kind,
                message,
            } => {
                assert_eq!(request_id, 51);
                assert_eq!(kind, EngineRequestKind::RunSql);
                assert!(message.contains("outcome is unknown"));
            }
            other => panic!("unexpected response: {other:?}"),
        }
        assert!(matches!(
            replacement.handle.await.expect("replacement response"),
            EngineResponse::Error {
                request_id: 52,
                kind: EngineRequestKind::RunSql,
                ..
            }
        ));

        let shutdown_active = ActiveTask {
            request_id: 53,
            kind: EngineRequestKind::CatalogRefresh,
            handle: tokio::spawn(async {
                std::future::pending::<()>().await;
                EngineResponse::CatalogRefreshed {
                    request_id: 53,
                    snapshot: CatalogSnapshot::default(),
                }
            }),
        };
        tokio::time::timeout(
            Duration::from_secs(1),
            cancel_active_task_with_timeout(
                &engine,
                shutdown_active,
                &responses,
                TEST_CANCELLATION_TIMEOUT,
            ),
        )
        .await
        .expect("shutdown must not wait indefinitely");
        assert!(matches!(
            response_rx.recv().await,
            Some(EngineResponse::Error {
                request_id: 53,
                kind: EngineRequestKind::CatalogRefresh,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn hanging_cancellation_delivery_is_bounded() {
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        let active = ActiveTask {
            request_id: 57,
            kind: EngineRequestKind::DeleteRow,
            handle: tokio::spawn(async {
                std::future::pending::<()>().await;
                success_response(57, EngineRequestKind::DeleteRow)
            }),
        };

        tokio::time::timeout(
            Duration::from_secs(1),
            cancel_active_task_with(active, &responses, TEST_CANCELLATION_TIMEOUT, || {
                std::future::pending::<Result<bool, &'static str>>()
            }),
        )
        .await
        .expect("hanging cancellation delivery must respect the deadline");

        match response_rx
            .recv()
            .await
            .expect("bounded cancellation response")
        {
            EngineResponse::Error {
                request_id,
                kind,
                message,
            } => {
                assert_eq!(request_id, 57);
                assert_eq!(kind, EngineRequestKind::DeleteRow);
                assert!(message.contains("deadline"));
                assert!(message.contains("outcome is unknown"));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn local_abort_preserves_an_already_completed_error() {
        let expected = "known database failure";
        let handle = tokio::spawn(async move {
            EngineResponse::Error {
                request_id: 59,
                kind: EngineRequestKind::DeleteRow,
                message: expected.to_string(),
            }
        });
        while !handle.is_finished() {
            tokio::task::yield_now().await;
        }
        let active = ActiveTask {
            request_id: 59,
            kind: EngineRequestKind::DeleteRow,
            handle,
        };
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();

        abort_with_unknown_outcome(active, "local timeout".to_string(), &responses, false).await;

        match response_rx.recv().await.expect("known response") {
            EngineResponse::Error {
                request_id,
                kind,
                message,
            } => {
                assert_eq!(request_id, 59);
                assert_eq!(kind, EngineRequestKind::DeleteRow);
                assert_eq!(message, expected);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }
}
