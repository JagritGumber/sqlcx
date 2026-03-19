import type {
  TableDef,
  QueryDef,
  EnumDef,
  ColumnDef,
  SqlType,
  SqlTypeCategory,
  QueryCommand,
  ParamDef,
  JsonShape,
} from "@/ir";
import type { DatabaseParser } from "@/parser/interface";
import { resolveParamNames, type RawParam } from "@/parser/param-naming";

// ---------------------------------------------------------------------------
// SQL type mapping
// ---------------------------------------------------------------------------

const TYPE_CATEGORY_MAP = new Map<string, SqlTypeCategory>([
  ["text", "string"],
  ["varchar", "string"],
  ["char", "string"],
  ["character varying", "string"],
  ["character", "string"],
  ["name", "string"],
  ["integer", "number"],
  ["int", "number"],
  ["int2", "number"],
  ["int4", "number"],
  ["int8", "number"],
  ["smallint", "number"],
  ["bigint", "number"],
  ["serial", "number"],
  ["bigserial", "number"],
  ["real", "number"],
  ["double precision", "number"],
  ["numeric", "number"],
  ["decimal", "number"],
  ["float", "number"],
  ["float4", "number"],
  ["float8", "number"],
  ["boolean", "boolean"],
  ["bool", "boolean"],
  ["timestamp", "date"],
  ["timestamptz", "date"],
  ["date", "date"],
  ["time", "date"],
  ["timetz", "date"],
  ["timestamp without time zone", "date"],
  ["timestamp with time zone", "date"],
  ["json", "json"],
  ["jsonb", "json"],
  ["uuid", "uuid"],
  ["bytea", "binary"],
]);

const SERIAL_TYPES = new Set(["serial", "bigserial"]);

function resolveType(raw: string, enumNames: Set<string>): SqlType {
  const trimmed = raw.trim();

  // Array detection
  if (trimmed.endsWith("[]")) {
    const baseRaw = trimmed.slice(0, -2);
    const elementType = resolveType(baseRaw, enumNames);
    return {
      raw: trimmed,
      normalized: trimmed.toLowerCase(),
      category: elementType.category,
      elementType,
    };
  }

  const normalized = trimmed.toLowerCase();
  const category = TYPE_CATEGORY_MAP.get(normalized);
  if (category) {
    return { raw: trimmed, normalized, category };
  }

  // Check if it's a known enum
  if (enumNames.has(normalized)) {
    return { raw: trimmed, normalized, category: "enum", enumName: normalized };
  }

  return { raw: trimmed, normalized, category: "unknown" };
}

// ---------------------------------------------------------------------------
// Enum parsing (regex-based)
// ---------------------------------------------------------------------------

const ENUM_RE_SOURCE =
  /CREATE\s+TYPE\s+(\w+)\s+AS\s+ENUM\s*\(\s*((?:'[^']*'(?:\s*,\s*'[^']*')*)?)\s*\)/i.source;

function parseEnumDefs(sql: string): EnumDef[] {
  const re = new RegExp(ENUM_RE_SOURCE, "gi");
  const enums: EnumDef[] = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(sql)) !== null) {
    const name = m[1].toLowerCase();
    const valuesRaw = m[2];
    const values = [...valuesRaw.matchAll(/'([^']*)'/g)].map((v) => v[1]);
    enums.push({ name, values });
  }
  return enums;
}

// ---------------------------------------------------------------------------
// Schema parsing (regex-based for reliability with custom types)
// ---------------------------------------------------------------------------

const CREATE_TABLE_RE_SOURCE =
  /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s*\(([\s\S]*?)\)\s*;/i.source;

/**
 * Split the CREATE TABLE body into individual column/constraint definitions.
 * Handles nested parentheses so REFERENCES users(id) doesn't cause a split.
 */
function splitColumnDefs(body: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let current = "";

  for (const ch of body) {
    if (ch === "(") {
      depth++;
      current += ch;
    } else if (ch === ")") {
      depth--;
      current += ch;
    } else if (ch === "," && depth === 0) {
      parts.push(current.trim());
      current = "";
    } else {
      current += ch;
    }
  }
  if (current.trim()) parts.push(current.trim());
  return parts;
}

const MULTI_WORD_TYPES = [
  "character varying",
  "double precision",
  "timestamp without time zone",
  "timestamp with time zone",
];

function parseColumnLine(
  line: string,
  enumNames: Set<string>,
): { col: ColumnDef; isPK: boolean; isUnique: boolean } | null {
  line = line.trim();
  if (!line) return null;
  if (/^(PRIMARY\s+KEY|CONSTRAINT|UNIQUE|CHECK|FOREIGN\s+KEY)/i.test(line)) {
    return null;
  }

  // Extract column name (first word)
  const nameMatch = line.match(/^(\w+)\s+/);
  if (!nameMatch) return null;
  const colName = nameMatch[1].toLowerCase();
  const afterName = line.slice(nameMatch[0].length);

  // Determine the type
  let rawType: string | null = null;
  for (const mwt of MULTI_WORD_TYPES) {
    if (afterName.toLowerCase().startsWith(mwt)) {
      rawType = mwt;
      break;
    }
  }
  if (!rawType) {
    const typeMatch = afterName.match(/^(\w+(?:\[\])?)/);
    rawType = typeMatch ? typeMatch[1] : "unknown";
  }

  const restAfterType = afterName.slice(rawType.length).trim();

  const isNotNull = /\bNOT\s+NULL\b/i.test(restAfterType);
  const hasDefaultKeyword = /\bDEFAULT\b/i.test(restAfterType);
  const isSerial = SERIAL_TYPES.has(rawType.toLowerCase());
  const isPK = /\bPRIMARY\s+KEY\b/i.test(restAfterType);
  const isUnique = /\bUNIQUE\b/i.test(restAfterType);

  const type = resolveType(rawType, enumNames);

  return {
    col: {
      name: colName,
      type,
      nullable: !isNotNull,
      hasDefault: hasDefaultKeyword || isSerial,
    },
    isPK,
    isUnique,
  };
}

// ---------------------------------------------------------------------------
// Inline annotation parsing (@enum, @json)
// ---------------------------------------------------------------------------

function parseEnumAnnotation(comment: string): string[] | undefined {
  const match = comment.match(/--\s*@enum\s*\(\s*(.*?)\s*\)/);
  if (!match) return undefined;
  const inner = match[1];
  const values: string[] = [];
  const re = /"([^"]*?)"/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(inner)) !== null) {
    values.push(m[1]);
  }
  return values.length > 0 ? values : undefined;
}

/**
 * Recursive-descent parser for the @json type DSL.
 * Supports: string, number, boolean, { key: type }, type[], type?
 */
class JsonShapeParser {
  private pos = 0;
  constructor(private input: string) {}

  parse(): JsonShape {
    const shape = this.parseType();
    this.skipWs();
    if (this.pos < this.input.length) {
      throw new Error(
        `@json parse error: unexpected trailing content at position ${this.pos}: "${this.input.slice(this.pos, this.pos + 10)}"`,
      );
    }
    return shape;
  }

  private parseType(): JsonShape {
    this.skipWs();
    let shape: JsonShape;

    if (this.peek() === "{") {
      shape = this.parseObject();
    } else {
      shape = this.parsePrimitive();
    }

    // Check for array suffix []
    this.skipWs();
    while (this.lookAhead("[]")) {
      this.pos += 2;
      this.skipWs();
      shape = { kind: "array", element: shape };
    }

    // Check for nullable suffix ?
    if (this.peek() === "?") {
      this.pos++;
      shape = { kind: "nullable", inner: shape };
    }

    return shape;
  }

  private parsePrimitive(): JsonShape {
    this.skipWs();
    if (this.matchWord("string")) return { kind: "string" };
    if (this.matchWord("number")) return { kind: "number" };
    if (this.matchWord("boolean")) return { kind: "boolean" };
    throw new Error(
      `@json parse error: unexpected token at position ${this.pos}: "${this.input.slice(this.pos, this.pos + 10)}"`,
    );
  }

  private parseObject(): JsonShape {
    this.consume("{");
    this.skipWs();
    const fields: Record<string, JsonShape> = {};

    if (this.peek() !== "}") {
      this.parseField(fields);
      while (this.peek() === ",") {
        this.pos++; // consume ','
        this.skipWs();
        if (this.peek() === "}") break; // trailing comma
        this.parseField(fields);
      }
    }

    this.consume("}");
    return { kind: "object", fields };
  }

  private parseField(fields: Record<string, JsonShape>): void {
    this.skipWs();
    const name = this.readIdentifier();
    this.skipWs();
    this.consume(":");
    this.skipWs();
    fields[name] = this.parseType();
    this.skipWs();
  }

  private readIdentifier(): string {
    this.skipWs();
    const start = this.pos;
    while (this.pos < this.input.length && /[\w]/.test(this.input[this.pos])) {
      this.pos++;
    }
    if (this.pos === start) {
      throw new Error(
        `@json parse error: expected identifier at position ${this.pos}`,
      );
    }
    return this.input.slice(start, this.pos);
  }

  private skipWs(): void {
    while (this.pos < this.input.length && /\s/.test(this.input[this.pos])) {
      this.pos++;
    }
  }

  private peek(): string | undefined {
    this.skipWs();
    return this.input[this.pos];
  }

  private lookAhead(s: string): boolean {
    return this.input.startsWith(s, this.pos);
  }

  private matchWord(word: string): boolean {
    if (this.input.startsWith(word, this.pos)) {
      const afterPos = this.pos + word.length;
      if (afterPos >= this.input.length || !/\w/.test(this.input[afterPos])) {
        this.pos = afterPos;
        return true;
      }
    }
    return false;
  }

  private consume(ch: string): void {
    this.skipWs();
    if (this.input[this.pos] !== ch) {
      throw new Error(
        `@json parse error: expected '${ch}' at position ${this.pos}, got '${this.input[this.pos]}'`,
      );
    }
    this.pos++;
  }
}

function parseJsonAnnotation(comment: string): JsonShape | undefined {
  const match = comment.match(/--\s*@json\s*\(\s*([\s\S]+)\s*\)\s*$/);
  if (!match) return undefined;
  const body = match[1].trim();
  try {
    const parser = new JsonShapeParser(body);
    return parser.parse();
  } catch {
    return undefined;
  }
}

function parseSchemaDefs(sql: string, enumNames: Set<string>): TableDef[] {
  const re = new RegExp(CREATE_TABLE_RE_SOURCE, "gi");
  const tables: TableDef[] = [];
  let m: RegExpExecArray | null;

  while ((m = re.exec(sql)) !== null) {
    const tableName = m[1].toLowerCase();
    const body = m[2];

    const columns: ColumnDef[] = [];
    const primaryKey: string[] = [];
    const uniqueConstraints: string[][] = [];

    // Split body into raw lines, then group comments with the column that follows
    const rawLines = body.split("\n");
    let pendingComment = "";
    // First pass: associate comment lines with column defs
    // We accumulate lines into defs using splitColumnDefs on non-comment text
    let nonCommentBuffer = "";
    const commentMap = new Map<number, string>(); // defIndex -> comment

    for (const rawLine of rawLines) {
      const trimmedLine = rawLine.trim();
      if (trimmedLine.startsWith("--")) {
        // Accumulate comment lines — annotations can be on any line above the column
        pendingComment += (pendingComment ? "\n" : "") + trimmedLine;
      } else {
        // Non-comment content; track how many defs this adds
        const beforeDefs = splitColumnDefs(nonCommentBuffer).filter(
          (d) => d.trim().length > 0,
        ).length;
        nonCommentBuffer += (nonCommentBuffer ? "\n" : "") + rawLine;
        const afterDefs = splitColumnDefs(nonCommentBuffer).filter(
          (d) => d.trim().length > 0,
        ).length;

        if (afterDefs > beforeDefs && pendingComment) {
          commentMap.set(beforeDefs, pendingComment);
          pendingComment = "";
        } else if (afterDefs === beforeDefs) {
          // Still accumulating same def, keep comment pending
        } else {
          pendingComment = "";
        }
      }
    }

    const lines = splitColumnDefs(nonCommentBuffer);

    for (let i = 0; i < lines.length; i++) {
      const trimmed = lines[i].trim();

      // Table-level PRIMARY KEY constraint
      const pkMatch = trimmed.match(
        /^PRIMARY\s+KEY\s*\(\s*([\w\s,]+)\s*\)/i,
      );
      if (pkMatch) {
        primaryKey.push(
          ...pkMatch[1].split(",").map((s) => s.trim().toLowerCase()),
        );
        continue;
      }

      const result = parseColumnLine(trimmed, enumNames);
      if (!result) continue;

      // Apply inline annotations from the comment above this column
      const comment = commentMap.get(i);
      if (comment) {
        const enumValues = parseEnumAnnotation(comment);
        if (enumValues) {
          result.col.type = {
            ...result.col.type,
            category: "enum",
            enumValues,
          };
        }

        const jsonShape = parseJsonAnnotation(comment);
        if (jsonShape) {
          result.col.type = {
            ...result.col.type,
            jsonShape,
          };
        }
      }

      columns.push(result.col);
      if (result.isPK) {
        primaryKey.push(result.col.name);
      }
      if (result.isUnique) {
        uniqueConstraints.push([result.col.name]);
      }
    }

    // PK columns are implicitly NOT NULL — fix nullable for table-level PKs
    for (const col of columns) {
      if (primaryKey.includes(col.name)) {
        col.nullable = false;
        col.hasDefault = col.hasDefault || SERIAL_TYPES.has(col.type.normalized);
      }
    }

    tables.push({ name: tableName, columns, primaryKey, uniqueConstraints });
  }

  return tables;
}

// ---------------------------------------------------------------------------
// Query parsing
// ---------------------------------------------------------------------------

const QUERY_ANNOTATION_RE =
  /--\s*name:\s*(\w+)\s+:(one|many|execresult|exec)/;
const PARAM_OVERRIDE_RE = /--\s*@param\s+\$(\d+)\s+(\w+)/g;
const DOLLAR_PARAM_RE = /\$(\d+)/g;

interface QueryBlock {
  name: string;
  command: QueryCommand;
  sql: string;
  comments: string;
}

function splitQueryBlocks(sql: string): QueryBlock[] {
  const lines = sql.split("\n");
  const blocks: QueryBlock[] = [];
  let current: QueryBlock | null = null;
  let commentBuffer = "";

  for (const line of lines) {
    const trimmed = line.trim();
    const annotationMatch = trimmed.match(QUERY_ANNOTATION_RE);

    if (annotationMatch) {
      if (current) blocks.push(current);
      current = {
        name: annotationMatch[1],
        command: annotationMatch[2] as QueryCommand,
        sql: "",
        comments: commentBuffer + trimmed + "\n",
      };
      commentBuffer = "";
    } else if (trimmed.startsWith("--")) {
      if (current) {
        current.comments += trimmed + "\n";
      } else {
        commentBuffer += trimmed + "\n";
      }
    } else if (current && trimmed) {
      current.sql += (current.sql ? " " : "") + trimmed;
    }
  }

  if (current) blocks.push(current);
  return blocks;
}

function extractParamOverrides(comments: string): Map<number, string> {
  const overrides = new Map<number, string>();
  let m: RegExpExecArray | null;
  const re = new RegExp(PARAM_OVERRIDE_RE.source, "g");
  while ((m = re.exec(comments)) !== null) {
    overrides.set(parseInt(m[1], 10), m[2]);
  }
  return overrides;
}

function extractParamIndices(sql: string): number[] {
  const indices = new Set<number>();
  let m: RegExpExecArray | null;
  const re = new RegExp(DOLLAR_PARAM_RE.source, "g");
  while ((m = re.exec(sql)) !== null) {
    indices.add(parseInt(m[1], 10));
  }
  return [...indices].sort((a, b) => a - b);
}

/**
 * Try to infer what column a $N param corresponds to from the SQL text.
 * Handles WHERE col = $1, col ILIKE $1, and INSERT positional mapping.
 */
function inferParamColumns(sql: string): Map<number, string> {
  const result = new Map<number, string>();

  // INSERT: columns list maps positionally to VALUES params
  const insertMatch = sql.match(
    /INSERT\s+INTO\s+\w+\s*\(\s*([\w\s,]+)\s*\)\s*VALUES\s*\(\s*([\$\d\s,]+)\s*\)/i,
  );
  if (insertMatch) {
    const cols = insertMatch[1].split(",").map((s) => s.trim().toLowerCase());
    const params = [...insertMatch[2].matchAll(/\$(\d+)/g)].map((m) =>
      parseInt(m[1], 10),
    );
    for (let i = 0; i < Math.min(cols.length, params.length); i++) {
      result.set(params[i], cols[i]);
    }
    return result;
  }

  // SQL keywords that can appear before operators but aren't column names
  const SQL_KEYWORDS = new Set([
    "not", "and", "or", "where", "set", "when", "then", "else", "case",
    "between", "exists", "any", "all", "some", "having",
  ]);

  // WHERE/SET: col op $N — also try to extract column from FUNC(col) op $N
  const wherePatterns =
    /(?:(\w+)\s*\(\s*(\w+)\s*\)|(\w+))\s*(?:=|!=|<>|<=?|>=?|(?:NOT\s+)?(?:I?LIKE|IN|IS))\s*\$(\d+)/gi;
  let m: RegExpExecArray | null;
  while ((m = wherePatterns.exec(sql)) !== null) {
    const paramIdx = parseInt(m[4], 10);
    if (m[1] && m[2]) {
      // FUNC(col) pattern — use the inner column name
      result.set(paramIdx, m[2].toLowerCase());
    } else if (m[3]) {
      const word = m[3].toLowerCase();
      if (!SQL_KEYWORDS.has(word)) {
        result.set(paramIdx, word);
      }
    }
  }

  return result;
}

function findFromTable(
  sql: string,
  tables: TableDef[],
): TableDef | undefined {
  const fromMatch = sql.match(/(?:FROM|INTO|UPDATE)\s+(\w+)/i);
  if (!fromMatch) return undefined;
  const tableName = fromMatch[1].toLowerCase();
  return tables.find((t) => t.name === tableName);
}

function resolveReturningColumns(
  sql: string,
  table: TableDef | undefined,
): ColumnDef[] | null {
  const returningMatch = sql.match(/\bRETURNING\s+([\s\S]+?)(?:;?\s*)$/i);
  if (!returningMatch) return null;

  const colsPart = returningMatch[1].trim();
  if (colsPart === "*") {
    return table ? [...table.columns] : [];
  }
  if (!table) return [];

  return colsPart.split(",").map((s) => {
    const name = s.trim().toLowerCase();
    const tableCol = table.columns.find((c) => c.name === name);
    return tableCol
      ? { ...tableCol }
      : { name, type: { raw: "unknown", normalized: "unknown", category: "unknown" }, nullable: true, hasDefault: false };
  });
}

function resolveReturnColumns(
  sql: string,
  table: TableDef | undefined,
): ColumnDef[] {
  // Check RETURNING clause first (INSERT/UPDATE/DELETE ... RETURNING)
  const returning = resolveReturningColumns(sql, table);
  if (returning) return returning;

  if (!/^\s*SELECT\b/i.test(sql)) return [];

  const selectMatch = sql.match(/SELECT\s+([\s\S]+?)\s+FROM\b/i);
  if (!selectMatch) return [];

  const colsPart = selectMatch[1].trim();

  if (colsPart === "*") {
    return table ? [...table.columns] : [];
  }

  if (!table) return [];
  const colNames = colsPart.split(",").map((s) => s.trim().toLowerCase());
  const resolved: ColumnDef[] = [];

  for (const colExpr of colNames) {
    const aliasMatch = colExpr.match(/^(\w+)\s+as\s+(\w+)$/i);
    const actualName = aliasMatch ? aliasMatch[1] : colExpr;
    const tableCol = table.columns.find((c) => c.name === actualName);

    if (tableCol) {
      resolved.push(
        aliasMatch
          ? { ...tableCol, alias: aliasMatch[2].toLowerCase() }
          : { ...tableCol },
      );
    } else {
      resolved.push({
        name: actualName,
        type: { raw: "unknown", normalized: "unknown", category: "unknown" },
        nullable: true,
        hasDefault: false,
      });
    }
  }
  return resolved;
}

function buildParams(
  sql: string,
  comments: string,
  table: TableDef | undefined,
): ParamDef[] {
  const paramIndices = extractParamIndices(sql);
  if (paramIndices.length === 0) return [];

  const overrides = extractParamOverrides(comments);
  const inferredCols = inferParamColumns(sql);

  const rawParams: RawParam[] = paramIndices.map((idx) => ({
    index: idx,
    column: inferredCols.get(idx) ?? null,
    override: overrides.get(idx),
  }));

  const names = resolveParamNames(rawParams);

  return paramIndices.map((idx, i) => {
    const colName = inferredCols.get(idx);
    let type: SqlType = {
      raw: "unknown",
      normalized: "unknown",
      category: "unknown",
    };

    if (table && colName) {
      const tableCol = table.columns.find((c) => c.name === colName);
      if (tableCol) {
        type = tableCol.type;
      }
    }

    return { index: idx, name: names[i], type };
  });
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export function createPostgresParser(): DatabaseParser {
  return {
    dialect: "postgresql",

    parseEnums(sql: string): EnumDef[] {
      return parseEnumDefs(sql);
    },

    parseSchema(sql: string): TableDef[] {
      const enums = parseEnumDefs(sql);
      const enumNames = new Set(enums.map((e) => e.name));
      return parseSchemaDefs(sql, enumNames);
    },

    parseQueries(sql: string, tables: TableDef[]): QueryDef[] {
      const blocks = splitQueryBlocks(sql);
      return blocks.map((block) => {
        const table = findFromTable(block.sql, tables);
        const params = buildParams(block.sql, block.comments, table);
        const returns = resolveReturnColumns(block.sql, table);

        return {
          name: block.name,
          command: block.command,
          sql: block.sql.replace(/;\s*$/, ""),
          params,
          returns,
          sourceFile: "",
        };
      });
    },
  };
}
