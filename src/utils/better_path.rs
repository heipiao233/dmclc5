use std::{ffi::{OsStr, OsString}, fmt::Debug, ops::Div, path::{Path, PathBuf}};

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
#[derive(Clone, PartialEq, Eq)]
pub struct BetterPath<T: AsRef<Path> + Debug = PathBuf>(pub T);

impl <T: AsRef<Path> + Debug> Debug for BetterPath<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl <T: AsRef<Path> + Debug> AsRef<Path> for BetterPath<T>{
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl <T: AsRef<Path> + Debug> AsRef<Path> for Box<BetterPath<T>> {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl <T: AsRef<Path> + Debug> Div<OsString> for &BetterPath<T>{
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: OsString) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<&OsString> for &BetterPath<T>{
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: &OsString) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<&OsStr> for &BetterPath<T>{
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: &OsStr) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<String> for &BetterPath<T>{
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: String) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<&String> for &BetterPath<T>{
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: &String) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<&str> for &BetterPath<T>{
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: &str) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<OsString> for Box<BetterPath<T>> {
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: OsString) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<&OsString> for Box<BetterPath<T>> {
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: &OsString) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<&OsStr> for Box<BetterPath<T>> {
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: &OsStr) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<String> for Box<BetterPath<T>> {
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: String) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<&String> for Box<BetterPath<T>> {
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: &String) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> Div<&str> for Box<BetterPath<T>> {
    type Output = Box<BetterPath<PathBuf>>;

    fn div(self, rhs: &str) -> Self::Output {
        let mut buf = self.0.as_ref().to_path_buf();
        buf.push(rhs);
        Box::new(BetterPath(buf))
    }
}

impl <T: AsRef<Path> + Debug> From<T> for BetterPath<T>{
    fn from(value: T) -> Self {
        Self(value)
    }
}
