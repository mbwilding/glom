use std::path::PathBuf;

use directories::BaseDirs;

use crate::{
    glom_app::GlomConfig,
    result::{GlomError, Result},
};

pub fn default_config_path() -> PathBuf {
    if let Some(dirs) = BaseDirs::new() {
        dirs.config_dir().join("glom.toml")
    } else {
        PathBuf::from("glom.toml")
    }
}

pub fn save_config(config_file: &PathBuf, config: GlomConfig) -> Result<()> {
    confy::store_path(config_file, &config)
        .map_err(|e| GlomError::config_save_error(config_file.clone(), e))?;

    Ok(())
}
