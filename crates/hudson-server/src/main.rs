mod auth;
mod config;
mod routes;

use clap::Parser;
use config::Args;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let authenticated = args.require_api_token;
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
                workspace_id: args.workspace_id,
                id: args.actor_id,
            };
            let auth_store = store.clone();
            let auth_actor = actor.clone();
            let router = if let Some(task_queue) = args.temporal_task_queue {
                let tree = configuration.build_admission_tree(store, &actor)?;
                routes::scheduled(
                    tree,
                    actor,
                    hudson_core::scheduling::ScheduleTarget {
                        scheduler: format!("temporal:{}", args.namespace),
                        task_queue,
                    },
                )?
            } else {
                let tree = configuration.build_tree(store, &actor)?;
                routes::configured(tree, actor, durable)
            };
            Ok(if authenticated {
                auth::protect(router, auth_store, auth_actor)
            } else {
                router
            })
        };
        build().map_err(|error| error.to_string())
    })
    .join()
    .map_err(|_| "server initialization failed")??;
    // Retain the shared state until the async runtime has finished shutting down.
    // The synchronous PostgreSQL clients must be dropped outside Tokio.
    let retained_router = router.clone();
    let result = {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", args.port)).await?;
            eprintln!(
                "Hudson local API: http://{} ({})",
                listener.local_addr()?,
                if authenticated {
                    "bearer authentication required"
                } else {
                    "single user, no authentication"
                }
            );
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })
    };
    drop(retained_router);
    result
}
