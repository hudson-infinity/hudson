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
    #[arg(long, default_value = "hudson")]
    pub namespace: String,
    #[arg(long, default_value_t = 4318)]
    pub port: u16,
}
