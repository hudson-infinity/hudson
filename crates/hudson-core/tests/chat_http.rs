use hudson_core::adapters::{chat::ChatModel, models::ModelExecutor};
use hudson_harness::ModelRequest;
use std::{
    io::{Read, Write},
    net::TcpListener,
};

#[test]
fn sends_bounded_authenticated_request_and_decodes_response() {
    for legacy in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break bytes.len();
                }
            };
            let headers = String::from_utf8(bytes).unwrap().to_lowercase();
            assert!(headers.contains("authorization: bearer test-key"));
            assert!(headers.starts_with("post /v1/chat/completions "));
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            assert!(header_end < 8192);
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                request[if legacy {
                    "max_tokens"
                } else {
                    "max_completion_tokens"
                }],
                64
            );
            assert_eq!(request["model"], "configured-model");
            let response = r#"{"choices":[{"finish_reason":"stop","message":{"content":"hello"}}],"usage":{"prompt_tokens":12,"completion_tokens":3}}"#;
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        });
        let mut model = ChatModel::new(
            &format!("http://{address}/v1/chat/completions"),
            Some("test-key".into()),
            64,
        )
        .unwrap();
        model = model.with_legacy_token_limit(legacy);
        let response = model
            .call(&ModelRequest {
                instructions: "help".into(),
                model: "configured-model".into(),
                messages: vec![],
                tools: vec![],
            })
            .unwrap();
        assert!(matches!(
            response,
            hudson_harness::ModelResponse::Final { .. }
        ));
        assert_eq!(
            model.take_usage(),
            Some(hudson_core::models::TokenUsage {
                input_tokens: 12,
                output_tokens: 3
            })
        );
        assert!(model.take_usage().is_none());
        server.join().unwrap();
    }
}
