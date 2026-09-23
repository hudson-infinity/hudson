use clap::{Parser, Subcommand};
use hudson_core::{configured::Configuration, models::Actor, storage::Store};
use hudson_temporal::{RunActivities, RunWorkflow};
use std::path::PathBuf;
use temporalio_client::{
    envconfig::LoadClientConfigProfileOptions, Client, ClientOptions, Connection,
    WorkflowGetResultOptions, WorkflowIdConflictPolicy, WorkflowIdReusePolicy,
    WorkflowStartOptions,
};
use temporalio_sdk::{Runtime, Worker, WorkerOptions};
use uuid::Uuid;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    database: String,
    #[arg(long, default_value = "local")]
    namespace: String,
    #[arg(long, default_value = "hudson")]
    task_queue: String,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Worker,
    Run {
        #[arg(long, conflicts_with_all = ["resume", "input_file"], required_unless_present_any = ["resume", "input_file"])]
        task: Option<String>,
        #[arg(long, conflicts_with_all = ["resume", "task"])]
        input_file: Option<PathBuf>,
        #[arg(long)]
        resume: Option<Uuid>,
        #[arg(long, conflicts_with = "resume")]
        request_key: Option<String>,
        #[arg(long)]
        background: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let actor = Actor {
        workspace_id: "local".into(),
        id: "developer".into(),
    };
    let store = Store::postgres_local("/tmp", &args.database, &args.namespace)?;
    let tree = Configuration::load(&args.config)?.build_temporal_tree(store, &actor)?;
    // Blocking providers and PostgreSQL are constructed outside the Tokio runtime.
    let run_id = match &args.command {
        Command::Worker => None,
        Command::Run {
            task,
            input_file,
            resume,
            request_key,
            ..
        } => {
            Some(match resume {
                Some(id) => {
                    let run = tree.runtime.store.inspect(&actor, *id)?;
                    if run.parent_operation.is_some() {
                        return Err("child runs are scheduled by their parent workflow; resume the root run".into());
                    }
                    let bindings = tree.bindings();
                    let goal = bindings
                        .get(&run.agent_ref)
                        .ok_or("agent is not configured")?;
                    tree.runtime
                        .store
                        .validate_resume(&actor, *id, &run.agent_ref, goal)?;
                    tree.runtime.store.validate_model_budget(
                        &actor,
                        *id,
                        &tree.runtime.model_budget_binding(),
                    )?;
                    *id
                }
                None => tree.runtime.submit_with_goal(
                    &actor,
                    tree.reference.clone(),
                    read_input(task.as_deref(), input_file.as_deref())?,
                    request_key.clone(),
                    tree.goal.clone(),
                )?,
            })
        }
    };
    if let Some(id) = run_id {
        eprintln!("Run: {id}");
    }
    let activities = RunActivities::new(tree.into_scheduled(), actor);
    tokio::runtime::Runtime::new()?.block_on(async move {
        let runtime = Runtime::from_current_tokio(Default::default())?;
        let (connection, options) =
            ClientOptions::load_from_config(LoadClientConfigProfileOptions::default())?;
        let client = Client::new(Connection::connect(connection).await?, options)?;
        match args.command {
            Command::Worker => {
                let options = WorkerOptions::new(args.task_queue)
                    .register_workflow::<RunWorkflow>()?
                    .register_activities(activities)
                    .build();
                Worker::new(&runtime, client, options)?.run().await?;
            }
            Command::Run { background, .. } => {
                drop(activities);
                let id = run_id.ok_or("missing run id")?;
                let workflow_id = format!("hudson:{}:{id}", args.namespace);
                // Stable IDs let resubmission repair a crash between the Postgres
                // commit and Temporal start without starting a second active workflow.
                let started = client
                    .start_workflow(
                        RunWorkflow::run,
                        id,
                        WorkflowStartOptions::new(args.task_queue, workflow_id.clone())
                            .id_conflict_policy(WorkflowIdConflictPolicy::UseExisting)
                            .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
                            .build(),
                    )
                    .await;
                let handle = match started {
                    Ok(handle) => handle,
                    Err(temporalio_client::errors::WorkflowStartError::AlreadyStarted {
                        ..
                    }) => client.get_workflow_handle::<hudson_temporal::RunWorkflowDefinition>(
                        workflow_id.clone(),
                    ),
                    Err(error) => return Err(error.into()),
                };
                println!(
                    "{}",
                    serde_json::json!({"run_id":id,"workflow_id":workflow_id})
                );
                if !background {
                    let view = handle
                        .get_result(WorkflowGetResultOptions::default())
                        .await?;
                    println!("{}", serde_json::to_string_pretty(&view)?);
                    if view.status == hudson_core::models::RunStatus::Failed {
                        return Err("agent run failed".into());
                    }
                }
            }
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}

fn read_input(
    task: Option<&str>,
    file: Option<&std::path::Path>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    use std::io::Read;
    if let Some(file) = file {
        let reader: Box<dyn Read> = if file == std::path::Path::new("-") {
            Box::new(std::io::stdin())
        } else {
            Box::new(std::fs::File::open(file)?)
        };
        let mut bytes = Vec::new();
        reader.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err("input exceeds 1 MiB".into());
        }
        Ok(serde_json::from_slice(&bytes)?)
    } else {
        Ok(serde_json::Value::String(
            task.ok_or("task input required")?.into(),
        ))
    }
}
