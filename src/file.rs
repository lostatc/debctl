use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

/// The path of a repo source file.
#[derive(Debug, Clone)]
pub enum SourceFilePath {
    /// A source file installed in the APT sources directory.
    Installed { name: String },

    /// A source file at an arbitrary file path.
    File { path: PathBuf },
}

/// A kind of repo source file.
#[derive(Debug, Clone, Copy)]
pub enum SourceFileKind {
    /// A one-line-style source file.
    OneLine,

    /// A deb822-style source file.
    Deb822,
}

/// A repo source file.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: SourceFilePath,
    pub kind: SourceFileKind,
}

impl SourceFile {
    const SOURCES_DIR: &str = "/etc/apt/sources.list.d";

    /// The path of this source file.
    pub fn path(&self) -> Cow<'_, Path> {
        let extension = match self.kind {
            SourceFileKind::OneLine => "list",
            SourceFileKind::Deb822 => "sources",
        };

        match &self.path {
            SourceFilePath::Installed { name } => Cow::Owned(
                [Self::SOURCES_DIR, &format!("{}.{}", name, extension)]
                    .iter()
                    .collect(),
            ),
            SourceFilePath::File { path } => Cow::Borrowed(path),
        }
    }
}
