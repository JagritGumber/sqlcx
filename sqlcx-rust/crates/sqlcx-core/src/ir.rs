use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Enums ────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SqlTypeCategory {
    String,
    Number,
    Boolean,
    Date,
    Json,
    Uuid,
    #[serde(rename = "binary")]
    Binary,
    Enum,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QueryCommand {
    One,
    Many,
    Exec,
    #[serde(rename = "execresult")]
    ExecResult,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum JsonShape {
    String,
    Number,
    Boolean,
    Object {
        fields: HashMap<std::string::String, JsonShape>,
    },
    Array {
        element: Box<JsonShape>,
    },
    Nullable {
        inner: Box<JsonShape>,
    },
}

// ── Structs ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SqlType {
    pub raw: String,
    pub normalized: String,
    pub category: SqlTypeCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_type: Option<Box<SqlType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_shape: Option<JsonShape>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_table: Option<String>,
    #[serde(rename = "type")]
    pub sql_type: SqlType,
    pub nullable: bool,
    pub has_default: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub primary_key: Vec<String>,
    pub unique_constraints: Vec<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    pub index: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub sql_type: SqlType,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryDef {
    pub name: String,
    pub command: QueryCommand,
    pub sql: String,
    pub params: Vec<ParamDef>,
    pub returns: Vec<ColumnDef>,
    pub source_file: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EnumDef {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SqlcxIR {
    pub tables: Vec<TableDef>,
    pub queries: Vec<QueryDef>,
    pub enums: Vec<EnumDef>,
}

pub type Overrides = HashMap<String, String>;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn make_sql_type(raw: &str, category: SqlTypeCategory) -> SqlType {
        SqlType {
            raw: raw.to_string(),
            normalized: raw.to_lowercase(),
            category,
            element_type: None,
            enum_name: None,
            enum_values: None,
            json_shape: None,
        }
    }

    fn make_column(name: &str, has_default: bool) -> ColumnDef {
        ColumnDef {
            name: name.to_string(),
            alias: None,
            source_table: None,
            sql_type: make_sql_type("text", SqlTypeCategory::String),
            nullable: false,
            has_default,
        }
    }

    #[test]
    fn ir_round_trip_json() {
        let ir = SqlcxIR {
            tables: vec![TableDef {
                name: "users".to_string(),
                columns: vec![
                    make_column("id", false),
                    make_column("email", false),
                ],
                primary_key: vec!["id".to_string()],
                unique_constraints: vec![vec!["email".to_string()]],
            }],
            queries: vec![],
            enums: vec![],
        };

        let json = serde_json::to_string(&ir).expect("serialize");
        let restored: SqlcxIR = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.tables.len(), 1);
        assert_eq!(restored.tables[0].name, "users");
        assert_eq!(restored.tables[0].columns.len(), 2);
        assert_eq!(restored.tables[0].columns[0].name, "id");
        assert_eq!(restored.tables[0].primary_key, vec!["id"]);
        assert_eq!(
            restored.tables[0].unique_constraints,
            vec![vec!["email".to_string()]]
        );
    }

    #[test]
    fn sql_type_category_serializes_lowercase() {
        let s = serde_json::to_string(&SqlTypeCategory::String).unwrap();
        assert_eq!(s, r#""string""#);

        let b = serde_json::to_string(&SqlTypeCategory::Binary).unwrap();
        assert_eq!(b, r#""binary""#);

        let n = serde_json::to_string(&SqlTypeCategory::Number).unwrap();
        assert_eq!(n, r#""number""#);
    }

    #[test]
    fn json_shape_serializes_with_kind_tag() {
        let mut fields = HashMap::new();
        fields.insert("foo".to_string(), JsonShape::String);

        let shape = JsonShape::Object { fields };
        let v: Value = serde_json::to_value(&shape).unwrap();

        assert_eq!(v["kind"], "object");
        assert!(v["fields"].is_object());
        assert_eq!(v["fields"]["foo"]["kind"], "string");
    }

    #[test]
    fn query_command_serializes_lowercase() {
        let exec_result = serde_json::to_string(&QueryCommand::ExecResult).unwrap();
        assert_eq!(exec_result, r#""execresult""#);

        let one = serde_json::to_string(&QueryCommand::One).unwrap();
        assert_eq!(one, r#""one""#);

        let many = serde_json::to_string(&QueryCommand::Many).unwrap();
        assert_eq!(many, r#""many""#);
    }

    #[test]
    fn camel_case_json_keys() {
        let table = TableDef {
            name: "items".to_string(),
            columns: vec![make_column("price", true)],
            primary_key: vec!["id".to_string()],
            unique_constraints: vec![],
        };

        let v: Value = serde_json::to_value(&table).unwrap();

        // primaryKey not primary_key
        assert!(v.get("primaryKey").is_some(), "expected 'primaryKey' key");
        assert!(v.get("primary_key").is_none(), "unexpected 'primary_key' key");

        // uniqueConstraints not unique_constraints
        assert!(
            v.get("uniqueConstraints").is_some(),
            "expected 'uniqueConstraints' key"
        );

        // hasDefault not has_default
        let col = &v["columns"][0];
        assert!(col.get("hasDefault").is_some(), "expected 'hasDefault' key");
        assert!(col.get("has_default").is_none(), "unexpected 'has_default' key");
    }
}
