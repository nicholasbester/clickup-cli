use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub private: Option<bool>,
    pub archived: Option<bool>,
}
