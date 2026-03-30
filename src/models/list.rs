use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct List {
    pub id: String,
    pub name: String,
    pub task_count: Option<serde_json::Value>,
    pub status: Option<serde_json::Value>,
    pub due_date: Option<String>,
    pub content: Option<String>,
}
