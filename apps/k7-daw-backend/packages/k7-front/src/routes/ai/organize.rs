//! organize.rs — `POST /api/ai/organize`: AI organize's whole conversation on the host that holds the key, streamed as NDJSON.
//! The page sends one request and draws the stream; the model's turns, every check and the repair round happen here, in the song crate's rules.
//! Choices: `apps/k7-daw-frontend/docs/aiOrganize.md`.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::response::Response;
use hyper::StatusCode;
use serde_json::{Value, json};
use song::modules::ui::ai_answer::{AiEvent, ai_result_schema, read_ai_answer};
use song::modules::ui::ai_check::{
    AiContext, AiRequest, AiResult, ai_user_prompt, bindable_names, check_ai_result, parse_request,
};
use song::modules::ui::ai_prompt::ai_system_prompt;

use super::claude::Claude;
use super::turn::{HEARTBEAT, Say, Turn, ask, stream};
use super::{AI_BODY_LIMIT, DEFAULT_AI_MODEL, read_for_update};
use crate::AppState;
use crate::js::truthy;
use crate::session::holds;
use crate::web::{error, read_json_within};

/// One proposal and one repair.
pub const ROUNDS: usize = 2;

/// POST `/organize`: the body parsed ahead of every guard, then the session, the key, the request's shape, the stream.
pub async fn post_organize(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let session = holds(req.headers(), &state.settings);
    let body = match read_json_within(req, AI_BODY_LIMIT).await {
        Ok((_, v)) => v,
        Err(r) => return r,
    };
    if !session {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let stored = read_for_update(&state).ok().flatten();
    let field = |k: &str| {
        stored
            .as_ref()
            .and_then(|s| s.get(k))
            .filter(|v| truthy(v))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let Some(key) = field("apiKey") else {
        let message = "no Claude key for this account — set one in the settings panel";
        return error(StatusCode::NOT_FOUND, message);
    };
    let model = field("model").unwrap_or_else(|| DEFAULT_AI_MODEL.into());
    let request = match parse_request(&body) {
        Ok(r) => r,
        Err(m) => return error(StatusCode::BAD_REQUEST, &m),
    };
    answer(
        &state.settings.anthropic_base_url,
        key,
        model,
        request,
        HEARTBEAT,
    )
}

/// The NDJSON stream of one organize run; a closed tab drops the request to Claude.
pub fn answer(
    base: &str,
    key: String,
    model: String,
    req: AiRequest,
    heartbeat: Duration,
) -> Response {
    let claude = Claude::new(base);
    stream(heartbeat, move |say| async move {
        match converse(&claude, &key, &model, &req, &say).await {
            Ok(result) => {
                say.line(&AiEvent::Done { result });
            }
            Err(message) => say.error(&message),
        }
    })
}

/// The turn's text read and checked: the panel when it passes, the model's mistakes when not.
fn judge(text: &str, names: &[String], ctx: &AiContext) -> Result<AiResult, Vec<String>> {
    let Some(answer) = read_ai_answer(text) else {
        return Err(vec!["the answer was not the JSON object asked for".into()]);
    };
    let issues = check_ai_result(&answer, names, ctx).issues;
    if issues.is_empty() {
        Ok(answer)
    } else {
        Err(issues)
    }
}

/// Ask, check, and hand back the mistakes once; a refusal, a failed turn or a second round of mistakes is the error.
async fn converse(
    claude: &Claude,
    key: &str,
    model: &str,
    req: &AiRequest,
    say: &Say,
) -> Result<AiResult, String> {
    let names = bindable_names(&req.params, &req.module_text);
    let ctx = AiContext {
        direction: req.direction.clone(),
        widgets_js: req.widgets_js.clone(),
        mode: req.mode,
    };
    let mut turn = Turn {
        system: ai_system_prompt(),
        schema: ai_result_schema(),
        messages: vec![json!({ "role": "user", "content": ai_user_prompt(req) })],
    };
    let mut issues: Vec<String> = Vec::new();
    for round in 1..=ROUNDS {
        say.status(if round == 1 {
            "Claude is designing the panel…"
        } else {
            "Claude is fixing the panel…"
        });
        let (text, content) = ask(claude, key, model, &turn, say).await?;
        match judge(&text, &names, &ctx) {
            Ok(done) => return Ok(done),
            Err(found) => issues = found,
        }
        let list: Vec<String> = issues.iter().map(|i| format!("- {i}")).collect();
        turn.messages
            .push(json!({ "role": "assistant", "content": content }));
        turn.messages.push(json!({
            "role": "user",
            "content": format!("The panel was refused:\n{}\n\nReturn the whole corrected answer.", list.join("\n")),
        }));
    }
    let mut said = vec!["Claude's panel did not pass the checks".to_owned()];
    said.extend(issues);
    Err(said.join(" — "))
}
