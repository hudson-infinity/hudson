//! Original offline illustration; see ../patterns.md. No model or real tools.

enum ModelReply {
    Tool { call_id: u32, name: &'static str },
    Final(String),
}

fn mock_model(tool: &'static str, observation: Option<&str>) -> ModelReply {
    match observation {
        None => ModelReply::Tool {
            call_id: 1,
            name: tool,
        },
        Some(value) => ModelReply::Final(format!("Your order is {value}.")),
    }
}

fn run(tool: &'static str, executions: &mut usize) -> Result<String, &'static str> {
    let mut observation = None;
    for turn in 1..=4 {
        println!("model turn {turn}");
        match mock_model(tool, observation.as_deref()) {
            ModelReply::Tool { call_id, name } => {
                // The model names a tool; the driver authorizes its use.
                if name != "lookup_order" {
                    return Err("permission denied before dispatch");
                }
                *executions += 1;
                println!("tool result for call {call_id}: shipped");
                observation = Some("shipped".to_owned());
            }
            ModelReply::Final(answer) => return Ok(answer),
        }
    }
    Err("turn limit reached")
}

fn main() {
    let mut executions = 0;
    println!("{}", run("lookup_order", &mut executions).unwrap());
    assert_eq!(executions, 1);
    assert_eq!(
        run("erase_orders", &mut executions),
        Err("permission denied before dispatch")
    );
    assert_eq!(executions, 1, "denied requests never reach the mock tool");
    println!("unauthorized tool blocked; total tool executions: {executions}");
}
