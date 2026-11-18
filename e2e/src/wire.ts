// wire.ts — requests a client library would tidy away: an upgrade read as its bare status, WebSocket frames written
// by hand (a ping, a header declaring a payload past the cap), a path sent with its dot segments, a Host of our choosing.
// What a front can break and a fetch hides (e2e/docs/conformance.md).
import { randomBytes } from "node:crypto";
import { request as httpRequest, type IncomingHttpHeaders } from "node:http";
import { request as httpsRequest } from "node:https";
import type { Duplex } from "node:stream";
import { base } from "./instance.ts";

const sender = (url: URL) =>
  url.protocol === "https:" ? httpsRequest : httpRequest;

/** An upgrade to `path`: its status, and the socket when it was taken (101). */
export function upgrade(
  path: string,
  headers: Record<string, string> = {},
): Promise<{ status: number; socket?: Duplex }> {
  const url = new URL(path, base);
  return new Promise((resolve, reject) => {
    const req = sender(url)(url, {
      rejectUnauthorized: false,
      headers: {
        connection: "Upgrade",
        upgrade: "websocket",
        "sec-websocket-version": "13",
        "sec-websocket-key": randomBytes(16).toString("base64"),
        ...headers,
      },
    });
    req.on("response", (res) => {
      res.resume();
      resolve({ status: res.statusCode ?? 0 });
    });
    req.on("upgrade", (_res, socket) => resolve({ status: 101, socket }));
    req.on("error", reject);
    req.end();
  });
}

/** One client frame, masked as a client must; `declared` lies about the length, to test a cap without sending it. */
export function clientFrame(
  opcode: number,
  payload: Buffer,
  declared = payload.length,
): Buffer {
  let head: Buffer;
  if (declared < 126) head = Buffer.from([0x80 | opcode, 0x80 | declared]);
  else if (declared < 65536) {
    head = Buffer.from([0x80 | opcode, 0x80 | 126, 0, 0]);
    head.writeUInt16BE(declared, 2);
  } else {
    head = Buffer.alloc(10);
    head[0] = 0x80 | opcode;
    head[1] = 0x80 | 127;
    head.writeBigUInt64BE(BigInt(declared), 2);
  }
  const mask = randomBytes(4);
  const body = Buffer.from(payload.map((b, i) => b ^ mask[i % 4]));
  return Buffer.concat([head, mask, body]);
}

/** The first server frame of `opcode` the socket receives within `ms`: its payload, unmasked as a server sends it. */
export function serverFrame(
  socket: Duplex,
  opcode: number,
  ms: number,
): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    let buf = Buffer.alloc(0);
    const timer = setTimeout(() => {
      socket.off("data", onData);
      reject(new Error(`no frame ${opcode} within ${ms} ms`));
    }, ms);
    const onData = (chunk: Buffer) => {
      buf = Buffer.concat([buf, chunk]);
      while (buf.length >= 2) {
        let len = buf[1] & 0x7f;
        let at = 2;
        if (len === 126) {
          if (buf.length < 4) return;
          len = buf.readUInt16BE(2);
          at = 4;
        } else if (len === 127) {
          if (buf.length < 10) return;
          len = Number(buf.readBigUInt64BE(2));
          at = 10;
        }
        if (buf.length < at + len) return;
        const op = buf[0] & 0x0f;
        const payload = buf.subarray(at, at + len);
        buf = buf.subarray(at + len);
        if (op === opcode) {
          clearTimeout(timer);
          socket.off("data", onData);
          resolve(Buffer.from(payload));
          return;
        }
      }
    };
    socket.on("data", onData);
  });
}

/** A request sent with `path` exactly as written and `headers` as given, Host included. */
export function rawRequest(
  path: string,
  headers: Record<string, string> = {},
  method = "GET",
): Promise<{ status: number; headers: IncomingHttpHeaders; body: string }> {
  const url = new URL(base);
  return new Promise((resolve, reject) => {
    const req = sender(url)(
      {
        host: url.hostname,
        port: url.port,
        path,
        method,
        headers,
        rejectUnauthorized: false,
      },
      (res) => {
        let body = "";
        res.setEncoding("utf8");
        res.on("data", (d) => {
          body += d;
        });
        res.on("end", () =>
          resolve({ status: res.statusCode ?? 0, headers: res.headers, body }),
        );
      },
    );
    req.on("error", reject);
    req.end();
  });
}
