import type { DatabaseClient } from "./client";

export interface GetUserRow {
  id: number;
  name: string;
  email: string;
  created_at: Date;
}

export interface GetUserParams {
  id: number;
}

export const getUserSql = "SELECT * FROM users WHERE id = $1";

export async function getUser(client: DatabaseClient, params: GetUserParams): Promise<GetUserRow | null> {
  return client.queryOne<GetUserRow>(getUserSql, [params.id]);
}

export interface ListUsersRow {
  id: number;
  name: string;
  email: string;
  created_at: Date;
}

export const listUsersSql = "SELECT * FROM users ORDER BY created_at DESC";

export async function listUsers(client: DatabaseClient): Promise<ListUsersRow[]> {
  return client.query<ListUsersRow>(listUsersSql, []);
}

export interface CreateUserParams {
  name: string;
  email: string;
}

export const createUserSql = "INSERT INTO users (name, email) VALUES ($1, $2)";

export async function createUser(client: DatabaseClient, params: CreateUserParams): Promise<void> {
  await client.execute(createUserSql, [params.name, params.email]);
}

export interface DeleteUserParams {
  id: number;
}

export const deleteUserSql = "DELETE FROM users WHERE id = $1";

export async function deleteUser(client: DatabaseClient, params: DeleteUserParams): Promise<{ rowsAffected: number }> {
  return client.execute(deleteUserSql, [params.id]);
}
