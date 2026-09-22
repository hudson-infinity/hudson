use crate::commands::{Args, Command, Start};
use serde_json::{json, Value};
use std::io::Read;

fn segment(id: &str) -> Result<&str, Box<dyn std::error::Error>> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err("invalid ID".into());
    }
    Ok(id)
}
pub fn execute(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let base = args.url.trim_end_matches('/');
    let response = match args.command {
        Command::Health => client.get(format!("{base}/health")).send()?,
        Command::Start(start) => {
            let body = submission(start)?;
            client.post(format!("{base}/runs")).json(&body).send()?
        }
        Command::Reply {
            run_id,
            question_id,
            text,
            request_key,
        } => {
            let body = json!({"input":text,"request_key":request_key,"question_id":question_id});
            if serde_json::to_vec(&body)?.len() > 64 * 1024 {
                return Err("reply exceeds 64 KiB".into());
            }
            client
                .post(format!("{base}/runs/{}/input", segment(&run_id)?))
                .json(&body)
                .send()?
        }
        Command::Resume { run_id } => client
            .post(format!("{base}/runs/{}/resume", segment(&run_id)?))
            .send()?,
        Command::Get { run_id } => client
            .get(format!("{base}/runs/{}", segment(&run_id)?))
            .send()?,
        Command::Children { run_id } => client
            .get(format!("{base}/runs/{}/children", segment(&run_id)?))
            .send()?,
        Command::Operation { operation_id } => client
            .get(format!("{base}/operations/{}", segment(&operation_id)?))
            .send()?,
        Command::Events { run_id, after } => client
            .get(format!("{base}/runs/{}/events", segment(&run_id)?))
            .query(&[("after", after)])
            .send()?,
        Command::Approve { operation_id, deny } => client
            .post(format!(
                "{base}/operations/{}/approval",
                segment(&operation_id)?
            ))
            .json(&json!({"approved":!deny}))
            .send()?,
        Command::Cancel { run_id } => client
            .post(format!("{base}/runs/{}/cancel", segment(&run_id)?))
            .send()?,
    };
    let status = response.status();
    let body: Value = response.json()?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    if !status.is_success() {
        return Err(format!("Hudson returned {status}").into());
    }
    Ok(())
}

fn submission(start: Start) -> Result<Value, Box<dyn std::error::Error>> {
    const MAX_BYTES: u64 = 64 * 1024;
    let input = if let Some(task) = start.task {
        json!(task)
    } else if let Some(path) = start.input_file {
        let reader: Box<dyn Read> = if path == std::path::Path::new("-") {
            Box::new(std::io::stdin())
        } else {
            Box::new(std::fs::File::open(path)?)
        };
        let mut bytes = Vec::new();
        reader.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("JSON input exceeds 64 KiB".into());
        }
        serde_json::from_slice(&bytes)?
    } else if let Some(order) = start.order {
        json!({"order_id":order,"action":if start.refund {"refund"} else {"lookup"}})
    } else {
        return Err("provide --task, --input-file, or fixture --order".into());
    };
    let body = json!({"input":input,"request_key":start.request_key});
    if serde_json::to_vec(&body)?.len() as u64 > MAX_BYTES {
        return Err("submission exceeds the API's 64 KiB body limit".into());
    }
    Ok(body)
}
