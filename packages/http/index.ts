// @studio/http — the Express layer shared by k7's backends: route
// wrapping/validation and per-request origin derivation. Importing this
// also declares `req.user`.

export { type AuthUser, sub } from "./express.ts";
export { publicOrigin, redirectUriFor } from "./origin.ts";
export { HttpError, validate, wrap } from "./route.ts";
