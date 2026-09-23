mod config;
mod routes;

use clap::Parser;
use config::Args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    // Blocking HTTP clients and PostgreSQL are constructed outside Tokio's async context.
    let router = std::thread::spawn(move || -> Result<axum::Router, String> {
        if args.demo {
            return routes::router().map_err(|error| error.to_string());
        }
        let build = || -> Result<axum::Router, Box<dyn std::error::Error>> {
            let configuration =
                hudson_core::configured::Configuration::load(args.config.as_deref().unwrap())?;
            let durable = args.database.is_some();
            let store = match args.database {
                Some(database) => {
                    hudson_core::storage::Store::postgres_local("/tmp", &database, &args.namespace)?
                }
                None => hudson_core::storage::Store::default(),
            };
            let actor = hudson_core::models::Actor {
                workspace_id: "local".into(),
                id: "developer".into(),
            };
            if let Some(task_queue) = args.temporal_task_queue {
                let tree = configuration.build_temporal_tree(store, &actor)?;
                return Ok(routes::scheduled(
                    tree,
                    actor,
                    hudson_core::scheduling::ScheduleTarget {
                        scheduler: format!("temporal:{}", args.namespace),
                        task_queue,
                    },
                )?);
            }
            let tree = configuration.build_tree(store, &actor)?;
            Ok(routes::configured(tree, actor, durable))
        };
        build().map_err(|error| error.to_string())
    })
    .join()
    .map_err(|_| "server initialization failed")??;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", args.port)).await?;
    eprintln!(
        "Hudson local API: http://{} (single user, no authentication)",
        listener.local_addr()?
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
