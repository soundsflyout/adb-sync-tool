use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ConfigFile {
    pub local_dir: String,
    pub remote_dir: String,
    pub allow_hidden: bool,
}
