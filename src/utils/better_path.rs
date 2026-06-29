use std::{fmt::Debug, ops::Div, path::{Path, PathBuf}};


/// A better PathBuf with "divide" support.
///
/// # Examples
/// ```
/// use dmclc5::utils::BetterPath;
/// use std::path::PathBuf;
///
/// assert_eq!(BetterPath(PathBuf::from("/usr/bin/bash")), *(&BetterPath(PathBuf::from("/usr")) / "bin/bash"))
/// ```
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BetterPathBuf(pub PathBuf);

impl AsRef<Path> for BetterPathBuf {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl <B: AsRef<Path>> Div<B> for BetterPathBuf {
    type Output = BetterPathBuf;

    fn div(mut self, rhs: B) -> Self::Output {
        self.0.push(&rhs);
        self
    }
}
