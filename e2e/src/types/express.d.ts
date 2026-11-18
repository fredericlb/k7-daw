import type { SessionUser } from "@studio/oidc";

// Importing @studio/oidc typechecks its source, which reads the Request.user its consumers declare.
declare global {
  namespace Express {
    interface Request {
      user?: SessionUser;
    }
  }
}
