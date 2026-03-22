---
title: MySQL
description: MySQL 8.0+ support in sqlcx — inline ENUMs, AUTO_INCREMENT, TINYINT(1), and more.
---

sqlcx supports MySQL 8.0+ syntax, including inline `ENUM` column definitions, `AUTO_INCREMENT`, `TINYINT(1)` booleans, generated columns, and backtick-quoted identifiers.

## Configuration

```toml
[sqlcx]
parser = "mysql"
```

## Supported Features

- Inline `ENUM('a', 'b', 'c')` column definitions
- `AUTO_INCREMENT` primary keys
- `TINYINT(1)` mapped to boolean
- `?` positional query parameters
- `GENERATED ALWAYS AS (expr)` computed columns
- Backtick-quoted identifiers (`` `table_name` ``)

## Type Mapping

| SQL Type | Generated Type |
|---|---|
| `INT`, `INTEGER`, `SMALLINT`, `BIGINT`, `TINYINT` | `integer` |
| `TINYINT(1)` | `boolean` |
| `FLOAT`, `DOUBLE`, `DECIMAL`, `NUMERIC` | `float` |
| `VARCHAR`, `TEXT`, `CHAR`, `TINYTEXT`, `MEDIUMTEXT`, `LONGTEXT` | `string` |
| `DATETIME`, `TIMESTAMP` | `datetime` |
| `JSON` | `json` |
| `BLOB`, `BINARY`, `VARBINARY`, `TINYBLOB`, `MEDIUMBLOB`, `LONGBLOB` | `binary` |
| `ENUM('a', 'b', …)` | `enum` |

## Example Schema

```sql
CREATE TABLE orders (
  id         INT AUTO_INCREMENT PRIMARY KEY,
  status     ENUM('pending', 'processing', 'shipped', 'delivered', 'cancelled') NOT NULL DEFAULT 'pending',
  is_paid    TINYINT(1) NOT NULL DEFAULT 0,
  total      DECIMAL(10, 2) NOT NULL,
  notes      TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

sqlcx will generate an inline enum for `status` with variants `Pending`, `Processing`, `Shipped`, `Delivered`, and `Cancelled`, and map `is_paid` to a boolean.

## MySQL-Specific Notes

### Inline ENUMs

Unlike PostgreSQL, MySQL defines `ENUM` values directly in the column definition. sqlcx parses these inline and synthesizes an enum type named after the column (e.g., `orders.status` → `OrdersStatus`).

### TINYINT(1)

MySQL conventionally uses `TINYINT(1)` to represent boolean values (stored as `0` or `1`). sqlcx detects this pattern and maps it to the `boolean` type instead of `integer`.

### ? Parameters

MySQL uses `?` as its parameter placeholder. sqlcx generates parameter lists in positional order matching the `?` occurrences in the query.

### Backtick Quoting

MySQL allows (and sometimes requires) backtick-quoted identifiers. sqlcx strips backticks when generating type and field names, so `` `user_id` `` becomes `userId` (or `user_id` in snake_case targets).
