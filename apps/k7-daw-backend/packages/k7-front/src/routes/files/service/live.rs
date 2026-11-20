//! live.rs — the poll's and the open's answers spelled as node's: the root in node's spelling.
//! What a reader gets wrong: only the `root` field changes; any other answer passes untouched.

use axum::body::Body;
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{Value, json};

use crate::routes::files::json_answer;

/// A drive-letter path spelled with backslashes, as node's win32 `resolve` gives it (`windowsSpelling`, history.ts).
pub(super) fn windows_spelling(p: &str) -> String {
    let b = p.as_bytes();
    let drive =
        b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\');
    if drive {
        p.replace('/', "\\")
    } else {
        p.to_string()
    }
}

/// The poll's or the open's answer as node gives it: its `root` spelled as node's.
pub async fn as_node_answers(r: Response) -> Response {
    if !r.status().is_success() {
        return r;
    }
    let (parts, body) = r.into_parts();
    let Ok(bytes) = body.collect().await.map(|b| b.to_bytes()) else {
        return json_answer(502, &json!({ "error": "file service: the answer was cut" }));
    };
    let Ok(mut v) = serde_json::from_slice::<Value>(&bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    if let Some(Value::String(root)) = v.get_mut("root") {
        *root = windows_spelling(root);
    }
    json_answer(parts.status.as_u16(), &v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_a_drive_letter_path_as_node_does() {
        assert_eq!(
            windows_spelling("C:/Users/a/workspace/songs"),
            "C:\\Users\\a\\workspace\\songs"
        );
        assert_eq!(
            windows_spelling("/home/a/workspace/songs"),
            "/home/a/workspace/songs"
        );
        assert_eq!(windows_spelling("C:"), "C:");
    }
}
