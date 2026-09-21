//! Scripted candidate generation and deterministic verification; no model judge.
fn solve(candidates: &[i32], max_attempts: usize) -> Result<i32, &'static str> {
    for (index, candidate) in candidates.iter().take(max_attempts).enumerate() {
        let verified = *candidate == 2 + 2;
        println!(
            "attempt {}: candidate={candidate}, verified={verified}",
            index + 1
        );
        if verified {
            return Ok(*candidate);
        }
        println!("feedback to next mock model turn: expected the sum of 2 and 2");
    }
    Err("verification failed within the attempt budget")
}

fn main() {
    assert_eq!(solve(&[5, 4], 2), Ok(4));
    assert!(solve(&[5, 6, 4], 2).is_err());
    println!("model completion and verified success are separate outcomes");
}
