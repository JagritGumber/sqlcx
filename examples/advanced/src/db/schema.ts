import { Type, type Static } from "@sinclair/typebox";

// Requires @sinclair/typebox >= 0.31.0 (for Type.Date and Type.Uint8Array)

type Prettify<T> = { [K in keyof T]: T[K] } & {};

export const PostStatus = Type.Union([Type.Literal("draft"), Type.Literal("published"), Type.Literal("archived")]);

export const SelectUsers = Type.Object({
  "id": Type.Number(),
  "username": Type.String(),
  "email": Type.String(),
  "bio": Type.Union([Type.String(), Type.Null()]),
  "role": Type.Union([Type.Literal("admin"), Type.Literal("editor"), Type.Literal("viewer")]),
  "preferences": Type.Union([Type.Object({ "theme": Type.String(), "language": Type.String(), "notifications": Type.Boolean() }), Type.Null()]),
  "tags": Type.Union([Type.Array(Type.String()), Type.Null()]),
  "created_at": Type.Date(),
  "updated_at": Type.Date()
});

export const InsertUsers = Type.Object({
  "id": Type.Optional(Type.Number()),
  "username": Type.String(),
  "email": Type.String(),
  "bio": Type.Optional(Type.Union([Type.String(), Type.Null()])),
  "role": Type.Optional(Type.Union([Type.Literal("admin"), Type.Literal("editor"), Type.Literal("viewer")])),
  "preferences": Type.Optional(Type.Union([Type.Object({ "theme": Type.String(), "language": Type.String(), "notifications": Type.Boolean() }), Type.Null()])),
  "tags": Type.Optional(Type.Union([Type.Array(Type.String()), Type.Null()])),
  "created_at": Type.Optional(Type.Date()),
  "updated_at": Type.Optional(Type.Date())
});

export const SelectPosts = Type.Object({
  "id": Type.Number(),
  "user_id": Type.Number(),
  "title": Type.String(),
  "slug": Type.String(),
  "body": Type.String(),
  "status": PostStatus,
  "stats": Type.Object({ "views": Type.Number(), "likes": Type.Number(), "shares": Type.Number() }),
  "published_at": Type.Union([Type.Date(), Type.Null()]),
  "created_at": Type.Date()
});

export const InsertPosts = Type.Object({
  "id": Type.Optional(Type.Number()),
  "user_id": Type.Number(),
  "title": Type.String(),
  "slug": Type.String(),
  "body": Type.String(),
  "status": Type.Optional(PostStatus),
  "stats": Type.Object({ "views": Type.Number(), "likes": Type.Number(), "shares": Type.Number() }),
  "published_at": Type.Optional(Type.Union([Type.Date(), Type.Null()])),
  "created_at": Type.Optional(Type.Date())
});

export const SelectComments = Type.Object({
  "id": Type.Number(),
  "post_id": Type.Number(),
  "user_id": Type.Number(),
  "body": Type.String(),
  "created_at": Type.Date()
});

export const InsertComments = Type.Object({
  "id": Type.Optional(Type.Number()),
  "post_id": Type.Number(),
  "user_id": Type.Number(),
  "body": Type.String(),
  "created_at": Type.Optional(Type.Date())
});

export type SelectUsers = Prettify<Static<typeof SelectUsers>>;

export type InsertUsers = Prettify<Static<typeof InsertUsers>>;

export type SelectPosts = Prettify<Static<typeof SelectPosts>>;

export type InsertPosts = Prettify<Static<typeof InsertPosts>>;

export type SelectComments = Prettify<Static<typeof SelectComments>>;

export type InsertComments = Prettify<Static<typeof InsertComments>>;

export type PostStatus = Prettify<Static<typeof PostStatus>>;
