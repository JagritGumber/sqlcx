import type {
  TableDef,
  QueryDef,
  EnumDef,
  ColumnDef,
  SqlType,
  SqlTypeCategory,
  QueryCommand,
  ParamDef,
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

const ENUM_RE =
  /CREATE\s+TYPE\s+(\w+)\s+AS\s+ENUM\s*\(\s*((?:'[^']*'(?:\s*,\s*'[^']*')*)?)\s*\)/gi;

function parseEnumDefs(sql: string): EnumDef[] {
  const enums: EnumDef[] = [];
  let m: RegExpExecArray | null;
  while ((m = ENUM_RE.exec(sql)) !== null) {
    const name = m[1].toLowerCase();
    const valuesRaw = m[2];
    const values = [...valuesRaw.matchAll(/'([^']*)'/g)].map((v) => v[1]);
    enums.push({ name, values });
  }
  ENUM_RE.lastIndex = 0;
  return enums;
}

// ---------------------------------------------------------------------------
// Schema parsing (regex-based for reliability with custom types)
// ---------------------------------------------------------------------------

const CREATE_TABLE_RE =
  /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s*\(([\s\S]*?)\)\s*;/gi;

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

function parseSchemaDefs(sql: string, enumNames: Set<string>): TableDef[] {
  const tables: TableDef[] = [];
  let m: RegExpExecArray | null;

  while ((m = CREATE_TABLE_RE.exec(sql)) !== null) {
    const tableName = m[1].toLowerCase();
    const body = m[2];

    const columns: ColumnDef[] = [];
    const primaryKey: string[] = [];
    const uniqueConstraints: string[][] = [];

    const lines = splitColumnDefs(body);

    for (const line of lines) {
      const trimmed = line.trim();

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

      columns.push(result.col);
      if (result.isPK) {
        primaryKey.push(result.col.name);
      }
      if (result.isUnique) {
        uniqueConstraints.push([result.col.name]);
      }
    }

    tables.push({ name: tableName, columns, primaryKey, uniqueConstraints });
  }

  CREATE_TABLE_RE.lastIndex = 0;
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

  // WHERE/SET: col op $N
  const wherePatterns =
    /(\w+)\s*(?:=|!=|<>|<=?|>=?|(?:NOT\s+)?(?:I?LIKE|IN|IS))\s*\$(\d+)/gi;
  let m: RegExpExecArray | null;
  while ((m = wherePatterns.exec(sql)) !== null) {
    result.set(parseInt(m[2], 10), m[1].toLowerCase());
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

function resolveReturnColumns(
  sql: string,
  table: TableDef | undefined,
): ColumnDef[] {
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
