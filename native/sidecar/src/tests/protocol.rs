use serde_json::{Map, json};

use super::{Response, handle, parse};
use now_playing_service::MediaService;

#[test]
fn malformed_requests_are_answered_without_an_id() {
    for line in [
        &b"not json"[..],
        b"{}",
        b"{\"id\":-1,\"method\":\"x\"}",
        b"{\"id\":1,\"method\":\"x\",\"extra\":1}",
    ] {
        let response = parse(line).unwrap_err();
        assert_eq!(response.id, None);
        assert_eq!(response.error.unwrap().code, "INVALID_ARGUMENT");
    }
    assert_eq!(parse(br#"{"id":7,"method":"current"}"#).unwrap().id, 7);
}

#[tokio::test(flavor = "current_thread")]
async fn methods_and_parameters_are_checked_before_reaching_the_os() {
    let service = MediaService::new();
    let hello = handle(&service, 1, "hello", &Map::new()).await;
    assert_eq!(hello.value.unwrap()["protocol"], json!(1));

    for (method, params) in [
        ("delete", json!({})),
        ("control", json!({})),
        ("control", json!({ "command": "explode" })),
        ("control", json!({ "command": "seek", "positionMs": -1 })),
        ("control", json!({ "command": "play", "sessionId": 3 })),
    ] {
        let response: Response = handle(&service, 2, method, params.as_object().unwrap()).await;
        assert_eq!(response.id, Some(2));
        assert_eq!(
            response.error.unwrap().code,
            "INVALID_ARGUMENT",
            "{method} {params}"
        );
    }
}
