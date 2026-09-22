use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use hudson_core::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, ToolRegistry},
    },
    configured::ConfiguredTree,
    fixtures::{self, DemoRuntime},
    models::*,
    runtime::Runtime,
    storage::Store,
    Error,
};
use hudson_harness::{AgentLoop, ModelRequest, ModelResponse};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

struct NoModel(Option<hudson_core::budgets::ModelBudgetBinding>);
impl ModelExecutor for NoModel {
    fn budget_binding(&self) -> Option<hudson_core::budgets::ModelBudgetBinding> {
        self.0.clone()
    }
    fn call(&mut self, _: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        Err(ExecutionError::Failed("control cannot call a model".into()))
    }
}
type Control = Runtime<AgentLoop, NoModel, ToolRegistry>;
enum Driver {
    Demo(DemoRuntime),
    Configured(ConfiguredTree),
}
impl Driver {
    fn tick(&mut self, actor: &Actor, id: Uuid) -> Result<RunView, Error> {
        match self {
            Self::Demo(runtime) => runtime.tick(actor, id),
            Self::Configured(runtime) => runtime.tick(actor, id),
        }
    }
}
#[derive(Clone)]
struct App {
    driver: Arc<Mutex<Driver>>,
    control: Arc<Mutex<Control>>,
    store: Store,
    active: Arc<Mutex<BTreeMap<Uuid, bool>>>,
    actor: Actor,
    agent: VersionRef,
    goal: Option<Goal>,
    bindings: BTreeMap<VersionRef, Option<Goal>>,
    model_budget: Option<hudson_core::budgets::ModelBudgetBinding>,
    fixture: bool,
}
struct ApiError(Error);
impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self(e)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::Denied => StatusCode::FORBIDDEN,
            Error::Conflict(_) => StatusCode::CONFLICT,
            Error::Invalid(_) | Error::Unsupported(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({"error":self.0.to_string()}))).into_response()
    }
}
fn unavailable() -> ApiError {
    ApiError(Error::Conflict("runtime unavailable".into()))
}
impl App {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Control>, ApiError> {
        self.control.lock().map_err(|_| unavailable())
    }
    fn validate_binding(&self, id: Uuid) -> Result<(), ApiError> {
        let run = self.store.inspect(&self.actor, id)?;
        let goal = self.bindings.get(&run.agent_ref).ok_or_else(|| {
            ApiError(Error::Conflict(
                "agent version is not configured in this host".into(),
            ))
        })?;
        self.store
            .validate_resume(&self.actor, id, &run.agent_ref, goal)?;
        self.store
            .validate_model_budget(&self.actor, id, &self.model_budget)?;
        Ok(())
    }
    fn drive(&self, id: Uuid) -> Result<(), ApiError> {
        self.validate_binding(id)?;
        // Duplicate submissions/resumes never enqueue another copy of an active run.
        {
            let mut active = self.active.lock().map_err(|_| unavailable())?;
            if let Some(wake) = active.get_mut(&id) {
                *wake = true;
                return Ok(());
            }
            active.insert(id, false);
        }
        let app = self.clone();
        tokio::task::spawn_blocking(move || {
            let result = (|| -> Result<(), ApiError> {
                let mut driver = app.driver.lock().map_err(|_| unavailable())?;
                loop {
                    let view = driver.tick(&app.actor, id)?;
                    if view.status.terminal()
                        || matches!(view.status, RunStatus::Waiting | RunStatus::Cancelling)
                    {
                        break;
                    }
                    // A previous executor may still own this operation. Never replay it.
                    if app
                        .store
                        .operations(&app.actor, id)?
                        .iter()
                        .any(|op| op.status == OperationStatus::Running)
                    {
                        break;
                    }
                }
                Ok(())
            })();
            let wake = app
                .active
                .lock()
                .ok()
                .and_then(|mut active| active.remove(&id))
                .unwrap_or(false);
            if wake {
                let _ = app.drive(id);
            }
            if let Err(error) = result {
                eprintln!("worker error: {}", error.0);
            }
        });
        Ok(())
    }
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| unavailable())?
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submission {
    input: Value,
    request_key: Option<String>,
}
async fn submit(
    State(app): State<App>,
    Json(body): Json<Submission>,
) -> Result<impl IntoResponse, ApiError> {
    blocking(move || {
        let id = if app.fixture {
            // Submission pins the backend checkpoint format, even before its first tick.
            Runtime::new(
                app.store.clone(),
                fixtures::FixtureBackend,
                NoModel(None),
                ToolRegistry::new(),
            )
            .submit(&app.actor, app.agent.clone(), body.input, body.request_key)?
        } else {
            app.lock()?.submit_with_goal(
                &app.actor,
                app.agent.clone(),
                body.input,
                body.request_key,
                app.goal.clone(),
            )?
        };
        app.drive(id)?;
        Ok((StatusCode::ACCEPTED, Json(json!({"run_id":id}))))
    })
    .await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserReply {
    question_id: u64,
    input: Value,
    request_key: String,
}
async fn reply(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Json(body): Json<UserReply>,
) -> Result<Json<Value>, ApiError> {
    blocking(move || {
        app.validate_binding(id)?;
        app.lock()?.provide_input(
            &app.actor,
            id,
            body.question_id,
            &body.request_key,
            body.input,
        )?;
        app.drive(id)?;
        Ok(Json(json!({"run_id":id})))
    })
    .await
}
async fn resume(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    blocking(move || {
        app.validate_binding(id)?;
        app.drive(id)?;
        Ok(Json(json!({"run_id":id})))
    })
    .await
}
async fn inspect(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    blocking(move || {
        Ok(Json(
            serde_json::to_value(app.store.inspect(&app.actor, id)?).map_err(Error::from)?,
        ))
    })
    .await
}
async fn children(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    blocking(move || {
        Ok(Json(
            serde_json::to_value(app.store.children(&app.actor, id)?).map_err(Error::from)?,
        ))
    })
    .await
}
async fn inspect_operation(
    State(app): State<App>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    blocking(move || {
        Ok(Json(
            serde_json::to_value(app.store.inspect_operation(&app.actor, id)?)
                .map_err(Error::from)?,
        ))
    })
    .await
}
#[derive(Deserialize)]
struct Cursor {
    #[serde(default)]
    after: u64,
}
async fn events(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Query(cursor): Query<Cursor>,
) -> Result<Json<Value>, ApiError> {
    blocking(move || {
        Ok(Json(
            serde_json::to_value(app.store.events(&app.actor, id, cursor.after)?)
                .map_err(Error::from)?,
        ))
    })
    .await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    approved: bool,
}
async fn approve(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Json(body): Json<Decision>,
) -> Result<Json<Value>, ApiError> {
    blocking(move || {
        let run_id = {
            let runtime = app.lock()?;
            let run_id = runtime.store.operation_run(&app.actor, id)?;
            app.validate_binding(run_id)?;
            runtime.approve(&app.actor, id, body.approved, now(), now() + 60_000)?;
            run_id
        };
        app.drive(run_id)?;
        Ok(Json(json!({"run_id":run_id})))
    })
    .await
}
async fn cancel(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    blocking(move || {
        app.lock()?.cancel(&app.actor, id)?;
        Ok(Json(json!({"run_id":id})))
    })
    .await
}
fn assemble(
    driver: Driver,
    store: Store,
    actor: Actor,
    agent: VersionRef,
    goal: Option<Goal>,
    durable: bool,
    mode: &'static str,
) -> Router {
    let bindings = match &driver {
        Driver::Configured(tree) => tree.bindings(),
        Driver::Demo(_) => [(agent.clone(), goal.clone())].into(),
    };
    let model_budget = match &driver {
        Driver::Configured(tree) => tree.runtime.model_budget_binding(),
        Driver::Demo(runtime) => runtime.model_budget_binding(),
    };
    let app = App {
        model_budget: model_budget.clone(),
        bindings,
        driver: Arc::new(Mutex::new(driver)),
        control: Arc::new(Mutex::new(Runtime::new(
            store.clone(),
            AgentLoop,
            NoModel(model_budget),
            ToolRegistry::new(),
        ))),
        store,
        active: Default::default(),
        actor,
        agent,
        goal,
        fixture: mode == "fixture",
    };
    Router::new()
        .route(
            "/openapi.json",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    include_str!("../../../docs/openapi.json"),
                )
            }),
        )
        .route(
            "/health",
            get(move || async move { Json(json!({"mode":mode,"durable":durable})) }),
        )
        .route("/runs", post(submit))
        .route("/runs/{id}", get(inspect))
        .route("/runs/{id}/events", get(events))
        .route("/runs/{id}/children", get(children))
        .route("/runs/{id}/resume", post(resume))
        .route("/runs/{id}/input", post(reply))
        .route("/runs/{id}/cancel", post(cancel))
        .route("/operations/{id}", get(inspect_operation))
        .route("/operations/{id}/approval", post(approve))
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .with_state(app)
}
pub fn router() -> Result<Router, Error> {
    let runtime = fixtures::runtime()?;
    let store = runtime.store.clone();
    Ok(assemble(
        Driver::Demo(runtime),
        store,
        fixtures::actor(),
        fixtures::agent_ref(),
        None,
        false,
        "fixture",
    ))
}
pub fn configured(tree: ConfiguredTree, actor: Actor, durable: bool) -> Router {
    let store = tree.runtime.store.clone();
    let agent = tree.reference.clone();
    let goal = tree.goal.clone();
    assemble(
        Driver::Configured(tree),
        store,
        actor,
        agent,
        goal,
        durable,
        "configured",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;

    async fn request(app: &Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
        let value = serde_json::from_slice(&bytes).unwrap();
        let specification: Value =
            serde_json::from_str(include_str!("../../../docs/openapi.json")).unwrap();
        let raw_path = path.split('?').next().unwrap();
        let mut segments = raw_path.split('/').collect::<Vec<_>>();
        if segments.len() >= 3 && matches!(segments[1], "runs" | "operations") {
            segments[2] = "{id}";
        }
        let responses =
            &specification["paths"][segments.join("/")][method.to_lowercase()]["responses"];
        let response = responses
            .get(status.as_str())
            .unwrap_or(&responses["default"]);
        let schema = &response["content"]["application/json"]["schema"];
        assert!(
            schema.is_object(),
            "missing response contract for {method} {path} {status}"
        );
        let validator = jsonschema::validator_for(
            &json!({"allOf":[schema], "components":specification["components"]}),
        )
        .unwrap();
        assert!(
            validator.is_valid(&value),
            "response violates contract: {method} {path}: {value}"
        );
        (status, value)
    }
    async fn until(app: &Router, id: &str, status: &str) -> Value {
        for _ in 0..100 {
            let (_, view) = request(app, "GET", &format!("/runs/{id}"), Value::Null).await;
            if view["status"] == status {
                return view;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("run did not reach {status}");
    }

    #[tokio::test]
    async fn serves_openapi_and_compiles_all_public_schemas() {
        let app = router().unwrap();
        let (status, spec) = request(&app, "GET", "/openapi.json", Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(spec["openapi"], "3.1.0");
        for name in spec["components"]["schemas"].as_object().unwrap().keys() {
            jsonschema::validator_for(&json!({"$ref":format!("#/components/schemas/{name}"), "components":spec["components"]})).unwrap();
        }
        let (_, health) = request(&app, "GET", "/health", Value::Null).await;
        assert_eq!(health["mode"], "fixture");
        let (_, start) = request(
            &app,
            "POST",
            "/runs",
            json!({"input":{"order_id":"123","action":"lookup"}}),
        )
        .await;
        let id = start["run_id"].as_str().unwrap();
        until(&app, id, "completed").await;
        let (_, children) =
            request(&app, "GET", &format!("/runs/{id}/children"), Value::Null).await;
        assert_eq!(children, json!([]));
        request(&app, "POST", &format!("/runs/{id}/resume"), Value::Null).await;
        let (status, _) = request(
            &app,
            "POST",
            &format!("/runs/{id}/input"),
            json!({"input":"late", "request_key":"answer", "question_id":1}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn preview_api_shows_exact_approval_and_resumes_the_same_run() {
        let app = router().unwrap();
        let (status, start) = request(
            &app,
            "POST",
            "/runs",
            json!({
                "input":{"order_id":"123","action":"refund"}, "request_key":"refund-1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let id = start["run_id"].as_str().unwrap();
        let waiting = until(&app, id, "waiting").await;
        assert!(waiting.get("state").is_none());
        let operation_id = waiting["wait"]["operation_id"].as_str().unwrap();
        let (status, operation) = request(
            &app,
            "GET",
            &format!("/operations/{operation_id}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(operation["tool_name"], "refund_order");
        assert_eq!(
            operation["arguments"],
            json!({"order_id":"123","amount_cents":3000})
        );
        assert!(operation.get("request").is_none());
        assert!(operation.get("attempts").is_none());
        let (status, _) = request(
            &app,
            "POST",
            &format!("/operations/{operation_id}/approval"),
            json!({"approved":true}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let completed = until(&app, id, "completed").await;
        assert_eq!(completed["assessment"]["passed"], true);
        let (_, events) = request(
            &app,
            "GET",
            &format!("/runs/{id}/events?after=2"),
            Value::Null,
        )
        .await;
        assert!(events
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["sequence"].as_u64().unwrap() > 2));
        let (_, duplicate) = request(
            &app,
            "POST",
            "/runs",
            json!({
                "input":{"order_id":"123","action":"refund"}, "request_key":"refund-1"
            }),
        )
        .await;
        assert_eq!(start, duplicate);
    }

    #[tokio::test]
    async fn preview_api_rejects_changed_submission_and_unknown_run() {
        let app = router().unwrap();
        let input = json!({"input":{"order_id":"123","action":"lookup"},"request_key":"same"});
        assert_eq!(
            request(&app, "POST", "/runs", input).await.0,
            StatusCode::ACCEPTED
        );
        assert_eq!(
            request(
                &app,
                "POST",
                "/runs",
                json!({
                    "input":{"order_id":"456","action":"lookup"},"request_key":"same"
                })
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            request(
                &app,
                "GET",
                &format!("/runs/{}", Uuid::new_v4()),
                Value::Null
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
}
