mod client;
mod commands;

use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    client::execute(commands::Args::parse())
}
