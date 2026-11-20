# The session token

A device has one user and the front listens on its loopback port, so a session does not say who someone is.
It says one thing: the page holds this install's token. That keeps any other website the person has open from
calling the loopback API with the person's own browser.

## The choices

| choice | why |
|---|---|
| one random token per install, 32 bytes as hex, drawn at the first start and kept in `session-token` under the auth folder (`STUDIO_AUTH_DIR`, else `<mediatek>/.auth`), mode 0600 where the OS has modes | a token that outlives a restart keeps the page's cookie valid; the owner alone may read it |
| `K7_SESSION_TOKEN` names the token instead, for a bed or a test | the suites hold the token without reading a file of the instance |
| the page presents it as the cookie `k7.sid`: `HttpOnly`, `SameSite=Strict`, `Path=/`, never `Secure` | no script of the page reads it, a request another site starts does not carry it, and a device's page is http on loopback |
| the token is compared in constant time, and an empty token matches nothing | a front started with no token answers no session |
| `GET /auth/me` hands the token over when the request holds none, and answers `{ user }` either way | the page asks it first (`AuthGate`), so there is no redirect and no host-specific step: the desktop window, a browser, and the phones' views all get the cookie the same way |
| that answer is refused with 403 when the browser says `Sec-Fetch-Site: cross-site`, and every route is behind `web::guard` first: a loopback peer, a loopback `Host`, then the Origin and cross-site refusals | a page on another site, or a rebound name, is given no cookie and reads no answer |
| the one user is `session::local_user()`, `dev@example.com`, whatever the cookie | nothing else names an account on a device |
| there is no sign-out and no revocation | one user on one machine has nothing to end; deleting `session-token` and restarting issues a new token |
| the page asks the file hub's hello only after `/auth/me` answered (`appMain.tsx`) | the hello needs the cookie, which the first `/auth/me` hands over |

## What this does not stop

Any process of the machine that can reach loopback names a loopback `Host`, so it asks `/auth/me` and gets the
token: on Android that is any app. What protects the device is that the front listens on `127.0.0.1` alone
(`standalone.md`), and a web page cannot be that process.
