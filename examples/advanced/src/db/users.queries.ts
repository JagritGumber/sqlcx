import type { DatabaseClient } from "./client";

export interface GetUserByIdRow {
  id: number;
  username: string;
  email: string;
  bio: string | null;
  role: string;
  preferences: unknown | null;
  tags: string[] | null;
  created_at: Date;
  updated_at: Date;
}

export interface GetUserByIdParams {
  id: number;
}

export const getUserByIdSql = "SELECT * FROM users WHERE id = $1";

export async function getUserById(client: DatabaseClient, params: GetUserByIdParams): Promise<GetUserByIdRow | null> {
  return client.queryOne<GetUserByIdRow>(getUserByIdSql, [params.id]);
}

export interface GetUserByUsernameRow {
  id: number;
  username: string;
  email: string;
  bio: string | null;
  role: string;
  preferences: unknown | null;
  tags: string[] | null;
  created_at: Date;
  updated_at: Date;
}

export interface GetUserByUsernameParams {
  username: string;
}

export const getUserByUsernameSql = "SELECT * FROM users WHERE username = $1";

export async function getUserByUsername(client: DatabaseClient, params: GetUserByUsernameParams): Promise<GetUserByUsernameRow | null> {
  return client.queryOne<GetUserByUsernameRow>(getUserByUsernameSql, [params.username]);
}

export interface ListUsersRow {
  id: number;
  username: string;
  email: string;
  role: string;
}

export const listUsersSql = "SELECT id, username, email, role FROM users ORDER BY created_at DESC";

export async function listUsers(client: DatabaseClient): Promise<ListUsersRow[]> {
  return client.query<ListUsersRow>(listUsersSql, []);
}

export interface CreateUserRow {
  id: number;
  username: string;
  email: string;
  bio: string | null;
  role: string;
  preferences: unknown | null;
  tags: string[] | null;
  created_at: Date;
  updated_at: Date;
}

export interface CreateUserParams {
  username: string;
  email: string;
  role: string;
}

export const createUserSql = "INSERT INTO users (username, email, role) VALUES ($1, $2, $3) RETURNING *";

export async function createUser(client: DatabaseClient, params: CreateUserParams): Promise<CreateUserRow | null> {
  return client.queryOne<CreateUserRow>(createUserSql, [params.username, params.email, params.role]);
}

export interface UpdateUserBioParams {
  bio: string;
  id: number;
}

export const updateUserBioSql = "UPDATE users SET bio = $1, updated_at = NOW() WHERE id = $2";

export async function updateUserBio(client: DatabaseClient, params: UpdateUserBioParams): Promise<void> {
  await client.execute(updateUserBioSql, [params.bio, params.id]);
}

export interface SearchUsersRow {
  id: number;
  username: string;
  email: string;
  bio: string | null;
  role: string;
  preferences: unknown | null;
  tags: string[] | null;
  created_at: Date;
  updated_at: Date;
}

export interface SearchUsersParams {
  email: string;
}

export const searchUsersSql = "SELECT * FROM users WHERE username ILIKE $1 OR email ILIKE $1";

export async function searchUsers(client: DatabaseClient, params: SearchUsersParams): Promise<SearchUsersRow[]> {
  return client.query<SearchUsersRow>(searchUsersSql, [params.email]);
}
