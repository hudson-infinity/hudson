use crate::config::Args;
use hudson_core::{
    configured::Configuration,
    evaluation::{self, Case},
    models::Actor,
    storage::Store,
};
use std::io::Read;

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(
        args.evaluate
            .as_ref()
            .ok_or("evaluation file is required")?,
    )?
    .take(1024 * 1024 + 1)
    .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("evaluation suite exceeds 1 MiB".into());
    }
    let cases: Vec<Case> = serde_json::from_slice(&bytes)?;
    evaluation::validate_cases(&cases)?;
    // Validate both configurations before any run can dispatch model/tool work.
    let baseline = Configuration::load(args.config.as_deref().ok_or("config is required")?)?;
    let candidate = args
        .compare_config
        .as_deref()
        .map(Configuration::load)
        .transpose()?;
    let store = match args.database {
        Some(database) => Store::postgres_local("/tmp", &database, &args.namespace)?,
        None => Store::default(),
    };
    let actor = Actor {
        workspace_id: args.workspace_id.clone(),
        id: args.actor_id.clone(),
    };
    let (mut runtime, reference, goal) = baseline.build(store.clone(), &actor)?;
    let candidate = candidate
        .map(|configuration| configuration.build(store, &actor))
        .transpose()?;
    let mut reports = vec![evaluation::evaluate(
        &mut runtime,
        &actor,
        reference,
        goal,
        &cases,
    )?];
    if let Some((mut runtime, reference, goal)) = candidate {
        reports.push(evaluation::evaluate(
            &mut runtime,
            &actor,
            reference,
            goal,
            &cases,
        )?);
    }
    let passed = reports.last().is_some_and(|report| report.passed);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"passed":passed,"reports":reports}))?
    );
    if !passed {
        return Err("evaluation failed; inspect the JSON report".into());
    }
    Ok(())
}
