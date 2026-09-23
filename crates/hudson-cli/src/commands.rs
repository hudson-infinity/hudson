use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Client for the local Hudson agent API")]
pub struct Args {
    #[arg(long, default_value = "http://127.0.0.1:4318")]
    pub url: String,
    /// Read the bearer token from this environment variable, never from an argument.
    #[arg(long)]
    pub api_token_env: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    Health,
    Capabilities,
    /// Publish a JSON request containing request_key and tool.
    PublishTool {
        file: std::path::PathBuf,
    },
    /// Publish a JSON request containing request_key and agent.
    PublishAgent {
        file: std::path::PathBuf,
    },
    Tool {
        name: String,
        version: u32,
    },
    Agent {
        name: String,
        version: u32,
    },
    Start(Start),
    /// Answer a waiting agent; reuse the key when retrying the same reply.
    Reply {
        run_id: String,
        #[arg(long)]
        question_id: u64,
        #[arg(long)]
        text: String,
        #[arg(long)]
        request_key: String,
    },
    Resume {
        run_id: String,
    },
    Get {
        run_id: String,
    },
    Children {
        run_id: String,
    },
    Operation {
        operation_id: String,
    },
    Events {
        run_id: String,
        #[arg(long, default_value_t = 0)]
        after: u64,
    },
    Approve {
        operation_id: String,
        #[arg(long)]
        deny: bool,
    },
    Cancel {
        run_id: String,
    },
}

#[derive(clap::Args)]
#[command(group(clap::ArgGroup::new("input").required(true).args(["task", "input_file", "order"])))]
pub struct Start {
    /// Select an immutable published agent; omit for startup-configured hosts.
    #[arg(long, requires_all = ["agent_version", "request_key"], conflicts_with = "order")]
    pub agent: Option<String>,
    #[arg(long, requires = "agent")]
    pub agent_version: Option<u32>,
    /// Submit a plain-text task to the configured agent.
    #[arg(long)]
    pub task: Option<String>,
    /// Submit arbitrary JSON from a file; use - to read standard input.
    #[arg(long)]
    pub input_file: Option<std::path::PathBuf>,
    /// Explicit input for the offline order fixture.
    #[arg(long)]
    pub order: Option<String>,
    #[arg(long, requires = "order")]
    pub refund: bool,
    #[arg(long)]
    pub request_key: Option<String>,
}
