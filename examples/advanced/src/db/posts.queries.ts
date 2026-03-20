import type { DatabaseClient } from "./client";

export interface GetPostRow {
  id: number;
  user_id: number;
  title: string;
  slug: string;
  body: string;
  status: string;
  stats: unknown;
  published_at: Date | null;
  created_at: Date;
}

export interface GetPostParams {
  id: number;
}

export const getPostSql = "SELECT * FROM posts WHERE id = $1";

export async function getPost(client: DatabaseClient, params: GetPostParams): Promise<GetPostRow | null> {
  return client.queryOne<GetPostRow>(getPostSql, [params.id]);
}

export interface GetPostBySlugRow {
  id: number;
  user_id: number;
  title: string;
  slug: string;
  body: string;
  status: string;
  stats: unknown;
  published_at: Date | null;
  created_at: Date;
}

export interface GetPostBySlugParams {
  slug: string;
}

export const getPostBySlugSql = "SELECT * FROM posts WHERE slug = $1";

export async function getPostBySlug(client: DatabaseClient, params: GetPostBySlugParams): Promise<GetPostBySlugRow | null> {
  return client.queryOne<GetPostBySlugRow>(getPostBySlugSql, [params.slug]);
}

export interface ListPostsByUserRow {
  id: number;
  user_id: number;
  title: string;
  slug: string;
  body: string;
  status: string;
  stats: unknown;
  published_at: Date | null;
  created_at: Date;
}

export interface ListPostsByUserParams {
  user_id: number;
}

export const listPostsByUserSql = "SELECT * FROM posts WHERE user_id = $1 ORDER BY created_at DESC";

export async function listPostsByUser(client: DatabaseClient, params: ListPostsByUserParams): Promise<ListPostsByUserRow[]> {
  return client.query<ListPostsByUserRow>(listPostsByUserSql, [params.user_id]);
}

export interface ListPublishedPostsRow {
  id: number;
  user_id: number;
  title: string;
  slug: string;
  body: string;
  status: string;
  stats: unknown;
  published_at: Date | null;
  created_at: Date;
}

export const listPublishedPostsSql = "SELECT * FROM posts WHERE status = 'published' ORDER BY published_at DESC";

export async function listPublishedPosts(client: DatabaseClient): Promise<ListPublishedPostsRow[]> {
  return client.query<ListPublishedPostsRow>(listPublishedPostsSql, []);
}

export interface CreatePostRow {
  id: number;
  user_id: number;
  title: string;
  slug: string;
  body: string;
  status: string;
  stats: unknown;
  published_at: Date | null;
  created_at: Date;
}

export interface CreatePostParams {
  user_id: number;
  title: string;
  slug: string;
  body: string;
  stats: unknown;
}

export const createPostSql = "INSERT INTO posts (user_id, title, slug, body, stats) VALUES ($1, $2, $3, $4, $5) RETURNING *";

export async function createPost(client: DatabaseClient, params: CreatePostParams): Promise<CreatePostRow | null> {
  return client.queryOne<CreatePostRow>(createPostSql, [params.user_id, params.title, params.slug, params.body, params.stats]);
}

export interface PublishPostParams {
  id: number;
}

export const publishPostSql = "UPDATE posts SET status = 'published', published_at = NOW() WHERE id = $1";

export async function publishPost(client: DatabaseClient, params: PublishPostParams): Promise<void> {
  await client.execute(publishPostSql, [params.id]);
}

export interface DeletePostParams {
  id: number;
}

export const deletePostSql = "DELETE FROM posts WHERE id = $1";

export async function deletePost(client: DatabaseClient, params: DeletePostParams): Promise<{ rowsAffected: number }> {
  return client.execute(deletePostSql, [params.id]);
}

export interface ListPostsByDateRangeRow {
  id: number;
  user_id: number;
  title: string;
  slug: string;
  body: string;
  status: string;
  stats: unknown;
  published_at: Date | null;
  created_at: Date;
}

export interface ListPostsByDateRangeParams {
  start_date: Date;
  end_date: Date;
}

export const listPostsByDateRangeSql = "SELECT * FROM posts WHERE published_at > $1 AND published_at < $2";

export async function listPostsByDateRange(client: DatabaseClient, params: ListPostsByDateRangeParams): Promise<ListPostsByDateRangeRow[]> {
  return client.query<ListPostsByDateRangeRow>(listPostsByDateRangeSql, [params.start_date, params.end_date]);
}
