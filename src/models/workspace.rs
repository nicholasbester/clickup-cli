use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub members: Option<Vec<serde_json::Value>>,
}
