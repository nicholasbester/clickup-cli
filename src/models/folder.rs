use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_folder: Option<String>,
    pub task_count: Option<String>,
    pub lists: Option<Vec<serde_json::Value>>,
}
