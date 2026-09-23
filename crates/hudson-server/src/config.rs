use clap::Parser;
#[derive(Parser)]
#[command(about = "Loopback-only, single-user Hudson agent API.")]
#[command(group(clap::ArgGroup::new("mode").required(true).args(["demo", "config"])))]
pub struct Args {
    #[arg(long)]
    pub demo: bool,
    #[arg(long)]
    pub config: Option<std::path::PathBuf>,
    #[arg(long, requires = "config")]
    pub database: Option<String>,
    /// Persist submissions for a separate Temporal worker; requires durable storage.
    #[arg(long, requires = "database")]
    pub temporal_task_queue: Option<String>,
    /// Require persisted bearer credentials on every API route.
    #[arg(long, requires = "database")]
    pub require_api_token: bool,
    #[arg(long, default_value = "local", requires = "config")]
    pub workspace_id: String,
    #[arg(long, default_value = "developer", requires = "config")]
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
