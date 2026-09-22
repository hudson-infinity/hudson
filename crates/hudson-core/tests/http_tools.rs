#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::{
        http_tools::HttpTools,
        tools::{ExecutionError, Invocation, ToolExecutor, ToolRegistry},
    },
    fixtures,
    models::Execution,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
};
#[test]
fn http_tool_sends_stable_operation_id_and_preserves_uncertainty() {
    for status in [200, 500] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/tool", listener.local_addr().unwrap());
        let operation_id = uuid::Uuid::new_v4();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                headers.push(byte[0]);
            }
            let headers = String::from_utf8(headers).unwrap().to_lowercase();
            assert!(headers.contains(&format!("idempotency-key: {operation_id}")));
            assert!(headers.contains("authorization: bearer local-test"));
            let len: usize = headers
                .lines()
                .find_map(|s| s.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; len];
            socket.read_exact(&mut body).unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                serde_json::json!({"a":1})
            );
            let body = "{\"answer\":42}";
            write!(
                socket,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let (_, mut definitions) = fixtures::definitions();
        let mut tool = definitions.remove(0);
        tool.execution = Execution::Http {
            endpoint: endpoint.clone(),
        };
        let mut executor = HttpTools::new(ToolRegistry::new()).unwrap();
        executor.bind(&endpoint, Some("local-test".into())).unwrap();
        let arguments = serde_json::json!({"a":1});
        let result = executor.execute(Invocation {
            operation_id,
            tool: &tool,
            arguments: &arguments,
        });
        if status == 200 {
            assert_eq!(result.unwrap()["answer"], 42);
        } else {
            assert!(matches!(result, Err(ExecutionError::Unknown(_))));
        }
        server.join().unwrap();
    }
}
