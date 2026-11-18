// aiRequest.ts — what the builder's AI organize request must look like, as one zod schema: the loop
// (aiLoop.ts) checks the request it is handed before a minute of the account's Claude is spent on it.

import { z } from "zod";
import type { AiOrganizeRequest } from "./aiOrganize";

const paramSchema = z.object({
  name: z.string().min(1),
  meta: z
    .object({
      min: z.number(),
      max: z.number(),
      def: z.number().optional(),
      states: z
        .array(z.object({ label: z.string(), value: z.number() }))
        .optional(),
    })
    .optional(),
  value: z.string().optional(),
});

export const requestSchema = z
  .object({
    module: z.string().min(1),
    moduleText: z.string(),
    params: z.array(paramSchema).max(2000),
    uiText: z.string(),
    widgetsJs: z.string(),
    mode: z.enum(["update", "reset"]),
    direction: z.string().max(2000),
  })
  // An update with nothing to change would pay a minute of the account's Claude for the panel it has.
  .refine((r) => r.mode !== "update" || r.direction.trim() !== "", {
    message: "an update says what should change",
    path: ["direction"],
  }) satisfies z.ZodType<AiOrganizeRequest>;
