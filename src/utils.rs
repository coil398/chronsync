use directories::UserDirs;
use std::path::PathBuf;

pub fn get_config_path() -> Result<PathBuf, String> {
    if let Some(user_dirs) = UserDirs::new() {
        let home_dir = user_dirs.home_dir();
        let config_path = home_dir
            .join(".config")
            .join("chronsync")
            .join("config.json");

        return Ok(config_path);
    }

    Err("Could not determine user home directory.".to_string())
}
