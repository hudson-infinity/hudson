//! Control messages are handled between bounded steps. No running process is killed.
use std::collections::VecDeque;

enum Control {
    Steer(&'static str),
    Cancel,
}

fn main() {
    let mut controls = VecDeque::from([Control::Steer("focus on delivery"), Control::Cancel]);
    let mut model_context = vec!["original customer task"];
    let mut tool_dispatches = 0;
    let mut cancelled = false;
    for _ in 0..4 {
        match controls.pop_front() {
            Some(Control::Steer(message)) => {
                model_context.push(message);
                println!("steering recorded: {message}");
            }
            Some(Control::Cancel) => {
                cancelled = true;
                println!("cancellation recorded; no further tool dispatch");
                break;
            }
            None => {}
        }
        tool_dispatches += 1;
        println!("one authorized mock read finished");
    }
    assert!(cancelled);
    assert_eq!(tool_dispatches, 1);
    assert_eq!(model_context.len(), 2);
    println!("real cancellation also requires acknowledgement from active executors");
}
