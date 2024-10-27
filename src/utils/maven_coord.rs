//! Things about [Maven coordinates](https://maven.apache.org/pom.html#Maven_Coordinates).

use std::fmt::Display;

use serde::{de::Visitor, Deserialize, Serialize};

/// Represents a [Maven coordinate](https://maven.apache.org/pom.html#Maven_Coordinates).
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ArtifactCoordinate {
    /// Group.
    pub group: String,
    /// Name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Classifier.
    pub classifier: Option<String>,
    /// Extension.
    pub extension: String
}

struct CoordStrVisitor;

impl <'de> Visitor<'de> for CoordStrVisitor {
    type Value = ArtifactCoordinate;
    
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a vaild maven coord")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {
        Ok(ArtifactCoordinate::from(v))
    }
}

impl <'de> Deserialize<'de> for ArtifactCoordinate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
        deserializer.deserialize_str(CoordStrVisitor)
    }
}

impl Serialize for ArtifactCoordinate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

impl Display for ArtifactCoordinate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.group, self.name, self.version)?;
        if let Some(cls) = &self.classifier {
            write!(f, ":{}", cls)?;
        }
        if self.extension != "jar" {
            write!(f, "@{}", self.extension)?;
        }
        Ok(())
    }
}

impl ArtifactCoordinate {
    /// Convert a [ArtifactCoordinate] to path.
    pub fn to_path(&self) -> String {
        let mut ret = format!("{}/{}/{}/{}-{}", self.group.replace(".", "/"), self.name, self.version, self.name, self.version);
        if let Some(cls) = &self.classifier {
            ret.push_str(&format!("-{cls}"));
        }
        ret.push_str(&format!(".{}", self.extension));
        ret
    }
}

impl From<&str> for ArtifactCoordinate {
    fn from(value: &str) -> Self {
        let splited = value.split("@").collect::<Vec<&str>>();
        let ext = if splited.len() == 2{
            splited[1]
        } else {
            "jar"
        };
        let mut splited = splited[0].split(":");
        Self {
            group: splited.next().unwrap().to_owned(),
            name: splited.next().unwrap().to_owned(),
            version: splited.next().unwrap().to_owned(),
            classifier: splited.next().map(str::to_string),
            extension: ext.to_string()
        }
    }
}
