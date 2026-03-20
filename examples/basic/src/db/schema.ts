import { Type, type Static } from "@sinclair/typebox";

// Requires @sinclair/typebox >= 0.31.0 (for Type.Date and Type.Uint8Array)

type Prettify<T> = { [K in keyof T]: T[K] } & {};

export const SelectUsers = Type.Object({
  "id": Type.Number(),
  "name": Type.String(),
  "email": Type.String(),
  "created_at": Type.Date()
});

export const InsertUsers = Type.Object({
  "id": Type.Optional(Type.Number()),
  "name": Type.String(),
  "email": Type.String(),
  "created_at": Type.Optional(Type.Date())
});

export type SelectUsers = Prettify<Static<typeof SelectUsers>>;

export type InsertUsers = Prettify<Static<typeof InsertUsers>>;
