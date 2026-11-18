// express.d.ts — `req.user` is @studio/http's AuthUser, which owns the Express augmentation;
// this adds the session's `groups` claim to it, since a second `Request.user` declaration would conflict.

import "@studio/http";

declare module "@studio/http" {
  interface AuthUser {
    groups?: string[];
  }
}
