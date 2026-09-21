//! Two independent mock reads. Parallel safety is trusted metadata here.
use std::thread;

#[derive(Clone, Copy)]
struct Call {
    id: u32,
    tool: &'static str,
    independent_read: bool,
}

fn batch(calls: &[Call], limit: usize) -> Result<Vec<(u32, String)>, &'static str> {
    // Validate the entire batch before starting any work.
    if calls.len() > limit || calls.iter().any(|c| !c.independent_read) {
        return Err("requires serial execution or exceeds concurrency limit");
    }
    if calls
        .iter()
        .any(|c| !matches!(c.tool, "lookup_order" | "lookup_shipment"))
    {
        return Err("permission denied");
    }
    Ok(thread::scope(|scope| {
        let handles: Vec<_> = calls
            .iter()
            .map(|call| scope.spawn(move || (call.id, format!("mock result from {}", call.tool))))
            .collect();
        // Correlate by call ID and preserve request order, regardless of finish order.
        handles
            .into_iter()
            .map(|h| h.join().expect("mock read panicked"))
            .collect()
    }))
}

fn main() {
    let calls = [
        Call {
            id: 1,
            tool: "lookup_order",
            independent_read: true,
        },
        Call {
            id: 2,
            tool: "lookup_shipment",
            independent_read: true,
        },
    ];
    let results = batch(&calls, 2).unwrap();
    assert_eq!(results.iter().map(|r| r.0).collect::<Vec<_>>(), vec![1, 2]);
    for (id, result) in results {
        println!("call {id}: {result}");
    }
    assert!(batch(&calls, 1).is_err());
    assert!(batch(
        &[Call {
            id: 3,
            tool: "refund",
            independent_read: false
        }],
        2
    )
    .is_err());
    println!("writes and oversized batches are rejected by this parallel-only example");
}
