//! In-memory state-machine simulation. Cloning is NOT durable persistence.
//! No credentials, real approvals, network operations, or production recovery.

#[derive(Clone, Debug, PartialEq, Eq)]
struct Request {
    operation_id: u64,
    amount_cents: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
    Waiting(Request),
    Dispatched(Request),
    Completed(Request, String),
}

// Supplied by a trusted fixture, never by the mock model.
struct Approval {
    request: Request,
    expires_at: u64,
}

fn advance(
    state: &mut State,
    approval: &Approval,
    now: u64,
    policy_allows: bool,
    dispatches: &mut usize,
) -> Result<&'static str, &'static str> {
    match state {
        State::Completed(_, _) => Ok("reuse completed receipt"),
        State::Dispatched(_) => Err("unknown outcome: reconcile before retry"),
        State::Waiting(request) => {
            if !policy_allows || now >= approval.expires_at || *request != approval.request {
                return Err("approval or current policy rejected");
            }
            // In production this intent must be durably stored BEFORE dispatch.
            *state = State::Dispatched(request.clone());
            *dispatches += 1;
            Ok("dispatched; pretend the acknowledgement was lost")
        }
    }
}

fn main() {
    let request = Request {
        operation_id: 42,
        amount_cents: 3000,
    };
    let checkpoint = State::Waiting(request.clone());
    let approval = Approval {
        request: request.clone(),
        expires_at: 100,
    };
    let mut dispatches = 0;

    let mut changed = State::Waiting(Request {
        amount_cents: 4000,
        ..request.clone()
    });
    assert!(advance(&mut changed, &approval, 10, true, &mut dispatches).is_err());
    let mut expired = checkpoint.clone();
    assert!(advance(&mut expired, &approval, 100, true, &mut dispatches).is_err());
    let mut revoked = checkpoint.clone();
    assert!(advance(&mut revoked, &approval, 10, false, &mut dispatches).is_err());
    assert_eq!(dispatches, 0);

    let mut restored = checkpoint.clone();
    println!(
        "{}",
        advance(&mut restored, &approval, 10, true, &mut dispatches).unwrap()
    );
    let mut after_interruption = restored.clone();
    println!(
        "{}",
        advance(
            &mut after_interruption,
            &approval,
            11,
            true,
            &mut dispatches
        )
        .unwrap_err()
    );
    assert_eq!(
        dispatches, 1,
        "uncertain effects must not be blindly repeated"
    );

    // A trusted mock reconciliation provides a known terminal receipt.
    after_interruption = State::Completed(request, "refund confirmed by destination".into());
    println!(
        "{}",
        advance(
            &mut after_interruption,
            &approval,
            12,
            true,
            &mut dispatches
        )
        .unwrap()
    );
    assert_eq!(dispatches, 1);
}
