// aiLoop.ts — AI organize's conversation, run in the browser: check the request, send turns to the host (which
// holds the key), check each answer against the loader's rules, and hand the mistakes back once. The host makes
// one model turn per call and decides nothing — docs/panelRules.md, "The loop".

import type { AiOrganizeResult } from "./aiAnswer";
import { AI_RESULT_SCHEMA, readAiAnswer } from "./aiAnswer";
import { aiUserPrompt, bindableNames, checkAiResult } from "./aiOrganize";
import { aiSystemPrompt } from "./aiPrompt";
import { requestSchema } from "./aiRequest";

/** One proposal and one repair. */
export const ROUNDS = 2;

/** What one model turn is asked: the constant prompt, the answer's schema, the conversation so far. */
export interface AiTurnRequest {
  system: string;
  schema: unknown;
  messages: { role: "user" | "assistant"; content: unknown }[];
}

/** What a turn answers: the model's text, and its content blocks to hand back on a repair. */
export interface AiTurnReply {
  text: string;
  content: unknown;
}

/** The host's half: one model turn, its progress through `onStatus`. */
export type AiTurnCall = (
  turn: AiTurnRequest,
  onStatus: (text: string) => void,
) => Promise<AiTurnReply>;

/** The request as zod parsed it (unknown keys dropped), or the message of why it is refused. */
export function parseRequest(body: unknown) {
  const parsed = requestSchema.safeParse(body);
  return parsed.success
    ? { req: parsed.data }
    : { invalid: parsed.error.message };
}

/** The turn's text read and checked: the panel when it passes, the model's mistakes when not. */
function judge(
  text: unknown,
  names: string[],
  req: Parameters<typeof checkAiResult>[2],
): { done: AiOrganizeResult } | { issues: string[] } {
  if (typeof text !== "string")
    return { issues: ["the answer was not the JSON object asked for"] };
  const answer = readAiAnswer(text);
  if (!answer)
    return { issues: ["the answer was not the JSON object asked for"] };
  const { issues } = checkAiResult(answer, names, req);
  return issues.length ? { issues } : { done: answer };
}

/** Ask, check, and hand back the mistakes once; a refusal, a failed turn or a second round of mistakes throws. */
export async function organizeLoop(
  body: unknown,
  turn: AiTurnCall,
  onStatus: (text: string) => void,
): Promise<AiOrganizeResult> {
  const start = parseRequest(body);
  if (!start.req) throw new Error(start.invalid);
  const { req } = start;
  const names = bindableNames(req);
  const system = aiSystemPrompt();
  const messages: AiTurnRequest["messages"] = [
    { role: "user", content: aiUserPrompt(req) },
  ];
  let issues: string[] = [];
  for (let round = 1; round <= ROUNDS; round++) {
    onStatus(
      round === 1
        ? "Claude is designing the panel…"
        : "Claude is fixing the panel…",
    );
    const reply = await turn(
      { system, schema: AI_RESULT_SCHEMA, messages: [...messages] },
      onStatus,
    );
    const verdict = judge(reply.text, names, req);
    if ("done" in verdict) return verdict.done;
    issues = verdict.issues;
    messages.push({ role: "assistant", content: reply.content });
    messages.push({
      role: "user",
      content: `The panel was refused:\n${issues.map((i) => `- ${i}`).join("\n")}\n\nReturn the whole corrected answer.`,
    });
  }
  throw new Error(
    ["Claude's panel did not pass the checks", ...issues].join(" — "),
  );
}
