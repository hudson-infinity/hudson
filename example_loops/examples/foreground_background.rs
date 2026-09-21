//! Same worker, two observation styles. Threads survive a dropped viewer,
//! but NOT process exit; real durability belongs to the future Hudson runtime.
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

#[derive(Clone)]
struct Receipt {
    id: u32,
    events: Arc<Mutex<Vec<&'static str>>>,
}

fn start(id: u32) -> (Receipt, JoinHandle<()>) {
    let receipt = Receipt {
        id,
        events: Arc::new(Mutex::new(Vec::new())),
    };
    let events = Arc::clone(&receipt.events);
    let worker = thread::spawn(move || {
        let mut log = events.lock().unwrap();
        log.push("started");
        log.push("mock tool completed");
        log.push("final result saved");
    });
    (receipt, worker)
}

fn main() {
    let (foreground, worker) = start(1);
    worker.join().unwrap(); // Foreground caller waits immediately.
    println!(
        "foreground {}: {:?}",
        foreground.id,
        foreground.events.lock().unwrap()
    );

    let (background, worker) = start(2);
    println!(
        "background receipt {} returned; caller can do other work",
        background.id
    );
    let viewer = background.clone();
    drop(viewer); // Disconnecting a viewer is not an execution cancellation.
    worker.join().unwrap(); // Demo keeps its process alive, then attaches to the result.
    assert_eq!(
        *foreground.events.lock().unwrap(),
        *background.events.lock().unwrap()
    );
    println!(
        "reattached to {}: {:?}",
        background.id,
        background.events.lock().unwrap()
    );
}
