use std::{io, path::Path};

use super::DirBuilder;

/// Create a new directory at the target path
///
/// # Note
///
/// - This function require the provided path's parent are all existing.
///     - To create a directory and all its missing parents at the same time, use the
///       [`create_dir_all`] function.
/// - Currently this function is supported on unix, windows is unimplement.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * User lacks permissions to create directory at `path`.
/// * A parent of the given path doesn't exist. (To create a directory and all its missing parents
///   at the same time, use the [`create_dir_all`] function.)
/// * `path` already exists.
///
/// # Examples
///
/// ```no_run
/// use monoio::fs;
///
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     fs::create_dir("/some/dir").await?;
///     Ok(())
/// }
/// ```
pub async fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    DirBuilder::new().create(path).await
}

/// Recursively create a directory and all of its missing components
///
/// # Note
///
/// - Currently this function is supported on unix, windows is unimplement.
///
/// # Errors
///
/// Same with [`create_dir`]
///
/// # Examples
///
/// ```no_run
/// use monoio::fs;
///
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     fs::create_dir_all("/some/dir").await?;
///     Ok(())
/// }
/// ```
pub async fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    DirBuilder::new().recursive(true).create(path).await
}
