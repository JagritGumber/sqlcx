use std::collections::{HashMap, HashSet};

pub struct RawParam {
    pub index: u32,
    pub column: Option<String>,
    pub r#override: Option<String>,
}

pub fn resolve_param_names(params: &[RawParam]) -> Vec<String> {
    // Pass 1: count column frequency for params without an override
    let mut freq: HashMap<&str, u32> = HashMap::new();
    for p in params {
        if p.r#override.is_none()
            && let Some(col) = &p.column
        {
            *freq.entry(col.as_str()).or_insert(0) += 1;
        }
    }

    // Pass 2: assign names, then dedup any collisions
    let mut counters: HashMap<&str, u32> = HashMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut result: Vec<String> = Vec::with_capacity(params.len());

    for p in params {
        let mut name = if let Some(ov) = &p.r#override {
            ov.clone()
        } else if let Some(col) = &p.column {
            if freq.get(col.as_str()).copied().unwrap_or(0) > 1 {
                let n = counters.entry(col.as_str()).or_insert(0);
                *n += 1;
                format!("{}_{}", col, n)
            } else {
                col.clone()
            }
        } else {
            format!("param_{}", p.index)
        };

        // Dedup: resolve any remaining collisions
        let base = name.clone();
        let mut suffix = 1u32;
        while seen.contains(&name) {
            name = format!("{}_{}", base, suffix);
            suffix += 1;
        }

        seen.insert(name.clone());
        result.push(name);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_column_name() {
        let params = vec![RawParam {
            index: 1,
            column: Some("id".to_string()),
            r#override: None,
        }];
        assert_eq!(resolve_param_names(&params), vec!["id"]);
    }

    #[test]
    fn collision_adds_suffix() {
        let params = vec![
            RawParam {
                index: 1,
                column: Some("created_at".to_string()),
                r#override: None,
            },
            RawParam {
                index: 2,
                column: Some("created_at".to_string()),
                r#override: None,
            },
        ];
        assert_eq!(
            resolve_param_names(&params),
            vec!["created_at_1", "created_at_2"]
        );
    }

    #[test]
    fn null_column_falls_back() {
        let params = vec![RawParam {
            index: 1,
            column: None,
            r#override: None,
        }];
        assert_eq!(resolve_param_names(&params), vec!["param_1"]);
    }

    #[test]
    fn override_takes_precedence() {
        let params = vec![RawParam {
            index: 1,
            column: Some("created_at".to_string()),
            r#override: Some("start_date".to_string()),
        }];
        assert_eq!(resolve_param_names(&params), vec!["start_date"]);
    }

    #[test]
    fn dedup_override_vs_inferred() {
        let params = vec![
            RawParam {
                index: 1,
                column: Some("id".to_string()),
                r#override: Some("id".to_string()),
            },
            RawParam {
                index: 2,
                column: Some("id".to_string()),
                r#override: None,
            },
        ];
        let result = resolve_param_names(&params);
        assert_eq!(result[0], "id");
        assert_eq!(result[1], "id_1");
    }
}
