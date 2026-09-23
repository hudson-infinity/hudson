use clap::Parser;
#[derive(Parser)]
#[command(about = "Loopback-only, single-user Hudson agent API.")]
#[command(group(clap::ArgGroup::new("mode").required(true).args(["demo", "config", "catalog"])))]
pub struct Args {
    #[arg(long)]
    pub demo: bool,
    #[arg(long)]
    pub config: Option<std::path::PathBuf>,
    /// Trusted capability catalog; enables authenticated customer publication.
    #[arg(long, requires_all = ["database", "require_api_token", "temporal_task_queue"])]
    pub catalog: Option<std::path::PathBuf>,
    #[arg(long, conflicts_with = "demo")]
    pub database: Option<String>,
    /// Persist submissions for a separate Temporal worker; requires durable storage.
    #[arg(long, requires = "database", conflicts_with = "demo")]
    pub temporal_task_queue: Option<String>,
    /// Require persisted bearer credentials on every API route.
    #[arg(long, requires = "database", conflicts_with = "demo")]
    pub require_api_token: bool,
    #[arg(long, default_value = "local", conflicts_with = "demo")]
    pub workspace_id: String,
    #[arg(long, default_value = "developer", conflicts_with = "demo")]
    pub actor_id: String,
    #[arg(long, default_value = "hudson")]
    pub namespace: String,
    #[arg(long, default_value_t = 4318)]
    pub port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_remains_available_and_authentication_requires_durable_configuration() {
        assert!(Args::try_parse_from(["server", "--demo"]).is_ok());
        for args in [
            vec!["server", "--catalog", "catalog.json"],
            vec![
                "server",
                "--catalog",
                "catalog.json",
                "--database",
                "test",
                "--temporal-task-queue",
                "q",
            ],
            vec![
                "server",
                "--catalog",
                "catalog.json",
                "--database",
                "test",
                "--require-api-token",
            ],
        ] {
            assert!(Args::try_parse_from(args).is_err());
        }
        assert!(Args::try_parse_from([
            "server",
            "--catalog",
            "catalog.json",
            "--database",
            "test",
            "--require-api-token",
            "--temporal-task-queue",
            "q"
        ])
        .is_ok());
        assert!(Args::try_parse_from(["server", "--demo", "--require-api-token"]).is_err());
        assert!(
            Args::try_parse_from(["server", "--config", "agent.json", "--require-api-token"])
                .is_err()
        );
        assert!(Args::try_parse_from([
            "server",
            "--config",
            "agent.json",
            "--database",
            "test",
            "--require-api-token"
        ])
        .is_ok());
    }
}
