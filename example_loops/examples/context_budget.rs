//! Character counts stand in for token accounting. Whole tool exchanges are kept.
#[derive(Clone)]
struct Exchange {
    call_id: u32,
    request: String,
    result: String,
}

fn pack(history: &[Exchange], pending: &str, cap: usize) -> Result<Vec<Exchange>, &'static str> {
    let policy = "Trusted policy is supplied separately.";
    let mut used = policy.len() + pending.len();
    if used > cap {
        return Err("required context exceeds budget");
    }
    let mut kept = Vec::new();
    for exchange in history.iter().rev() {
        let size = exchange.request.len() + exchange.result.len();
        if used + size > cap {
            break;
        }
        kept.push(exchange.clone());
        used += size;
    }
    kept.reverse();
    Ok(kept)
}

fn main() {
    let history = vec![
        Exchange {
            call_id: 1,
            request: "search".into(),
            result: "large old output ".repeat(20),
        },
        Exchange {
            call_id: 2,
            request: "lookup_order".into(),
            result: "shipped".into(),
        },
    ];
    let packed = pack(&history, "Answer the delivery question", 100).unwrap();
    assert_eq!(
        packed.iter().map(|e| e.call_id).collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(
        history.len(),
        2,
        "model context trimming leaves the evidence store intact"
    );
    assert!(pack(&history, "pending request", 1).is_err());
    println!(
        "kept complete exchange {}; old evidence remains in full history",
        packed[0].call_id
    );
    println!("real compaction also needs token counting, summary provenance, and provider rules");
}
