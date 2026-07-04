use std::path::{Path, PathBuf};

use crate::{CONFIG_FILE_NAME, ConfigError};

/// `start` から上方向に `yuzu.jsonc` を探索し、見つかったディレクトリ
/// （＝プロジェクトルート）を返す。
pub fn find_project_root(start: &Path) -> Result<PathBuf, ConfigError> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(CONFIG_FILE_NAME).is_file() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    Err(ConfigError::ProjectRootNotFound {
        start: start.to_path_buf(),
    })
}
