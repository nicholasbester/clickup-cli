use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: Option<serde_json::Value>,
    pub priority: Option<serde_json::Value>,
    pub assignees: Option<Vec<serde_json::Value>>,
    pub due_date: Option<String>,
    pub description: Option<String>,
    pub custom_id: Option<String>,
}
