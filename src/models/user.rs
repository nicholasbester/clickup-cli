use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub email: String,
    pub color: Option<String>,
    #[serde(alias = "profilePicture")]
    pub profile_picture: Option<String>,
}
