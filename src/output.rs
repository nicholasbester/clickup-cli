use chrono::DateTime;
use comfy_table::{ContentArrangement, Table};

#[derive(Clone)]
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
            fields: fields
                .as_ref()
                .map(|f| f.split(',').map(|s| s.trim().to_string()).collect()),
            no_header,
            quiet,
        }
    }

    pub fn print_items(
        &self,
        items: &[serde_json::Value],
        default_fields: &[&str],
        id_field: &str,
    ) {
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
                let filtered = compact_items(items, &fields);
                println!("{}", serde_json::to_string_pretty(&filtered).unwrap());
            }
            "csv" => {
                // Tabular output always renders every column ("-" when absent), so
                // the optional-omission `?` marker doesn't apply here — just strip
                // it from the header/lookup name.
                let names: Vec<&str> = fields
                    .iter()
                    .map(|&f| f.strip_suffix('?').unwrap_or(f))
                    .collect();
                if !self.no_header {
                    println!("{}", names.join(","));
                }
                for item in items {
                    let row: Vec<String> =
                        names.iter().map(|&f| flatten_value(item.get(f))).collect();
                    println!("{}", row.join(","));
                }
            }
            _ => {
                // table (default). Same `?`-stripping as csv, for the same reason.
                let names: Vec<&str> = fields
                    .iter()
                    .map(|&f| f.strip_suffix('?').unwrap_or(f))
                    .collect();
                let mut table = Table::new();
                table.set_content_arrangement(ContentArrangement::Dynamic);
                if !self.no_header {
                    table.set_header(names.iter().map(|f| f.to_string()).collect::<Vec<_>>());
                }
                for item in items {
                    let row: Vec<String> =
                        names.iter().map(|&f| flatten_value(item.get(f))).collect();
                    table.add_row(row);
                }
                println!("{}", table);
            }
        }
    }

    pub fn print_single(&self, item: &serde_json::Value, default_fields: &[&str], id_field: &str) {
        self.print_items(std::slice::from_ref(item), default_fields, id_field);
    }

    pub fn print_message(&self, message: &str) {
        if self.mode == "json" {
            println!("{}", serde_json::json!({ "message": message }));
        } else {
            println!("{}", message);
        }
    }
}

/// Flatten a list of items to only include the specified fields with flattened values.
/// Returns a JSON array. Used by MCP server for token-efficient responses. Also
/// reached by the CLI's `--output json-compact` mode with user-supplied `--fields`,
/// so the trailing-`?` marker described below applies there too.
///
/// A field name with a trailing `?` (e.g. `"custom_id?"`) is optional: it is
/// emitted under the name without the marker, and only when the source value
/// is present and non-null. All other fields are always emitted, with `"-"`
/// as the placeholder for missing/null values.
///
/// The trailing `?` is always interpreted as this optional marker, so a field
/// literally named with a trailing `?` cannot be projected verbatim.
pub fn compact_items(items: &[serde_json::Value], fields: &[&str]) -> serde_json::Value {
    let compacted: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let mut obj = serde_json::Map::new();
            for &field in fields {
                if let Some(key) = field.strip_suffix('?') {
                    match item.get(key) {
                        None | Some(serde_json::Value::Null) => {}
                        Some(v) => {
                            let val = flatten_value(Some(v));
                            obj.insert(key.to_string(), serde_json::Value::String(val));
                        }
                    }
                } else {
                    let val = flatten_value(item.get(field));
                    obj.insert(field.to_string(), serde_json::Value::String(val));
                }
            }
            serde_json::Value::Object(obj)
        })
        .collect();
    serde_json::Value::Array(compacted)
}

pub fn flatten_value(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => "-".to_string(),
        Some(serde_json::Value::String(s)) => {
            // Try to parse as Unix millisecond timestamp
            if let Ok(ms) = s.parse::<i64>() {
                if ms > 1_000_000_000_000 && ms < 10_000_000_000_000 {
                    if let Some(dt) = DateTime::from_timestamp_millis(ms) {
                        return dt.format("%Y-%m-%d").to_string();
                    }
                }
            }
            s.clone()
        }
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
            if items.is_empty() {
                "-".to_string()
            } else {
                items.join(", ")
            }
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
