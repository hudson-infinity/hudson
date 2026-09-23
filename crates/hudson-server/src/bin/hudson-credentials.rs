//! Local installation-operator commands. Database access is the authority boundary.
use clap::{Parser, Subcommand};
use hudson_core::{
    models::{now, Actor},
    storage::Store,
};
use uuid::Uuid;

#[derive(Parser)]
#[command(about = "Issue or revoke Hudson API credentials using trusted local database access.")]
struct Args {
    #[arg(long)]
    database: String,
    #[arg(long, default_value = "hudson")]
    namespace: String,
    #[arg(long, default_value = "local")]
    workspace_id: String,
    #[arg(long, default_value = "developer")]
    actor_id: String,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Print a new bearer token once. Store the output in the calling backend's secrets.
    Issue {
        #[arg(long)]
        label: String,
        #[arg(long, default_value_t = 86400)]
        ttl_seconds: u64,
    },
    Revoke {
        id: Uuid,
    },
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let store = Store::postgres_local("/tmp", &args.database, &args.namespace)?;
    let actor = Actor {
        workspace_id: args.workspace_id,
        id: args.actor_id,
    };
    match args.command {
        Command::Issue { label, ttl_seconds } => {
            let expires = ttl_seconds
                .checked_mul(1000)
                .and_then(|ttl| now().checked_add(ttl))
                .ok_or("invalid token lifetime")?;
            let issued = store.issue_api_token(actor, label, expires)?;
            println!(
                "{}",
                serde_json::json!({"metadata":&issued.metadata, "token":issued.bearer()})
            );
        }
        Command::Revoke { id } => {
            store.revoke_api_token(&actor, id)?;
            println!("{}", serde_json::json!({"revoked":id}));
        }
    }
    Ok(())
}
