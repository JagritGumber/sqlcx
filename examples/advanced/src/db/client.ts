export interface DatabaseClient {
  query<T>(sql: string, params: unknown[]): Promise<T[]>;
  queryOne<T>(sql: string, params: unknown[]): Promise<T | null>;
  execute(sql: string, params: unknown[]): Promise<{ rowsAffected: number }>;
}

interface BunSqlDriver {
  unsafe(query: string, values?: unknown[]): Promise<any[] & { count: number }>;
}

export class BunSqlClient implements DatabaseClient {
  private sql: BunSqlDriver;

  constructor(sql: BunSqlDriver) {
    this.sql = sql;
  }

  async query<T>(text: string, values?: unknown[]): Promise<T[]> {
    const result = await this.sql.unsafe(text, values);
    return [...result] as T[];
  }

  async queryOne<T>(text: string, values?: unknown[]): Promise<T | null> {
    const rows = await this.query<T>(text, values);
    return rows[0] ?? null;
  }

  async execute(text: string, values?: unknown[]): Promise<{ rowsAffected: number }> {
    const result = await this.sql.unsafe(text, values);
    return { rowsAffected: result.count };
  }
}
