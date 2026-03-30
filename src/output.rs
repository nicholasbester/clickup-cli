use comfy_table::{Table, ContentArrangement};

pub struct OutputConfig {
    pub mode: String,
    pub fields: Option<Vec<String>>,
    pub no_header: bool,
    pub quiet: bool,
}

impl OutputConfig {
    pub fn from_cli(mode: &str, fields: &Option<String>, no_header: bool, quiet: bool) -> Self {
        Self {
            mode: mode.to_string(),
            fields: fields.as_ref().map(|f| f.split(',').map(|s| s.trim().to_string()).collect()),
            no_header,
            quiet,
        }
    }

    pub fn print_items(&self, items: &[serde_json::Value], default_fields: &[&str], id_field: &str) {
        if self.quiet {
            for item in items {
                if let Some(id) = item.get(id_field).and_then(|v| v.as_str()) {
                    println!("{}", id);
                }
            }
            return;
        }

        let fields: Vec<&str> = match &self.fields {
            Some(f) => f.iter().map(|s| s.as_str()).collect(),
            None => default_fields.to_vec(),
        };

        match self.mode.as_str() {
            "json" => {
                println!("{}", serde_json::to_string_pretty(items).unwrap());
            }
            "json-compact" => {
                let filtered: Vec<serde_json::Value> = items
                    .iter()
                    .map(|item| {
                        let mut obj = serde_json::Map::new();
                        for &field in &fields {
                            if let Some(val) = item.get(field) {
                                obj.insert(field.to_string(), val.clone());
                            }
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&filtered).unwrap());
            }
            "csv" => {
                if !self.no_header {
                    println!("{}", fields.join(","));
                }
                for item in items {
                    let row: Vec<String> = fields
                        .iter()
                        .map(|&f| flatten_value(item.get(f)))
                        .collect();
                    println!("{}", row.join(","));
                }
            }
            _ => {
                // table (default)
                let mut table = Table::new();
                table.set_content_arrangement(ContentArrangement::Dynamic);
                if !self.no_header {
                    table.set_header(fields.iter().map(|f| f.to_string()).collect::<Vec<_>>());
                }
                for item in items {
                    let row: Vec<String> = fields
                        .iter()
                        .map(|&f| flatten_value(item.get(f)))
                        .collect();
                    table.add_row(row);
                }
                println!("{}", table);
            }
        }
    }

    pub fn print_single(&self, item: &serde_json::Value, default_fields: &[&str], id_field: &str) {
        self.print_items(&[item.clone()], default_fields, id_field);
    }

    pub fn print_message(&self, message: &str) {
        if self.mode == "json" {
            println!("{}", serde_json::json!({ "message": message }));
        } else {
            println!("{}", message);
        }
    }
}

pub fn flatten_value(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => "-".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        Some(serde_json::Value::Array(arr)) => {
            // For arrays of objects with "username" field (assignees)
            let items: Vec<String> = arr
                .iter()
                .map(|v| {
                    if let Some(username) = v.get("username").and_then(|u| u.as_str()) {
                        username.to_string()
                    } else if let Some(s) = v.as_str() {
                        s.to_string()
                    } else {
                        v.to_string()
                    }
                })
                .collect();
            if items.is_empty() { "-".to_string() } else { items.join(", ") }
        }
        Some(serde_json::Value::Object(obj)) => {
            // Flatten nested objects: status.status, priority.priority
            if let Some(inner) = obj.get("status").and_then(|v| v.as_str()) {
                inner.to_string()
            } else if let Some(inner) = obj.get("priority").and_then(|v| v.as_str()) {
                inner.to_string()
            } else if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                name.to_string()
            } else if let Some(username) = obj.get("username").and_then(|v| v.as_str()) {
                username.to_string()
            } else {
                serde_json::to_string(&serde_json::Value::Object(obj.clone())).unwrap()
            }
        }
    }
}
