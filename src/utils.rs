//! Some utilities.

mod better_path;
mod download;
pub mod maven_coord;

use std::ffi::{OsStr, OsString};

use anyhow::{anyhow, Result};
use maven_coord::ArtifactCoordinate;
#[cfg(feature="mod_loaders")]
use serde::de::Visitor;
#[cfg(feature="mod_loaders")]
use serde::Deserializer;
#[cfg(feature="mod_loaders")]
use versions::{Requirement, Versioning};

#[cfg(feature="mod_loaders")]
use crate::components::mods::VersionBound;
use crate::minecraft::schemas::{Arguments, EnvRule, OSType, VersionJSON};

pub use self::better_path::BetterPath;
pub use self::download::{download, download_res, download_txt, download_all, check_hash, download_to_writer, DownloadAllMessage};

#[cfg(not(target_os="windows"))]
/// The path delimiter.
pub const PATH_DELIMITER: &str = ":";

#[cfg(target_os="windows")]
/// The path delimiter.
pub const PATH_DELIMITER: &str = ";";

#[cfg(target_os="windows")]
/// Get current OS.
pub fn get_os() -> OSType {
    OSType::Windows
}

#[cfg(target_os="linux")]
/// Get current OS.
pub fn get_os() -> OSType {
    OSType::Linux
}

#[cfg(target_os="macos")]
/// Get current OS.
pub fn get_os() -> OSType {
    OSType::OSX
}

#[cfg(target_pointer_width = "64")]
/// Get current bits.
pub fn get_bits() -> String {
    "64".to_string()
}

#[cfg(target_pointer_width = "32")]
/// Get current bits.
pub fn get_bits() -> String {
    "32".to_string()
}

/// Check one [EnvRule].
pub fn check_rule(rule: &EnvRule) -> bool {
    if let Some(os) = &rule.os {
        if let Some(os) = &os.name {
            return get_os() == *os;
        }
    }

    rule.features.is_none() // No features support currently
}

/// Check [EnvRule]s.
pub fn check_rules_no_option(rules: &Vec<EnvRule>) -> bool {
    for i in rules {
        if !check_rule(i) {
            return false;
        }
    }
    true
}

/// Optionally check [EnvRule]s.
pub fn check_rules(rules: &Option<Vec<EnvRule>>) -> bool {
    if rules.is_none() {
        return true;
    }
    for i in rules.as_ref().unwrap() {
        if check_rule(i) {
            return true;
        }
    }
    false
}

/// Concat two OsStr.
pub fn osstr_concat<A: Clone, B: Clone + AsRef<OsStr>>(a: &A, b: &B) -> OsString
where OsString: From<A> {
    let mut str = OsString::from(a.clone());
    str.push(b.clone());
    str
}

/// Merge two [VersionJSON]s with the same version.
pub fn merge_version_json(a: &VersionJSON, b: &VersionJSON) -> Result<VersionJSON> {
    let mut c = a.get_base().clone();
    c.libraries = b.get_base().libraries.clone().into_iter().chain(c.libraries.into_iter()).collect();
    c.main_class = b.get_base().main_class.clone();
    match a {
        VersionJSON::Old { base: _, minecraft_arguments: _ } => {
            if let VersionJSON::Old {base: _, minecraft_arguments} = b {
                return Ok(VersionJSON::Old { base: c, minecraft_arguments: minecraft_arguments.to_string() });
            }
            return Result::Err(anyhow!("try to merge different kinds of version json!").into()); // TODO: i18n
        },
        VersionJSON::New { base: _, arguments: arguments_a } => {
            if let VersionJSON::New {base: _, arguments: arguments_b} = b {
                let mut new_game = arguments_a.game.clone();
                if new_game.is_none() {
                    new_game = arguments_b.game.clone();
                } else if let Some(b_game) = arguments_b.game.clone() {
                    new_game.as_mut().unwrap().extend(b_game);
                }

                let mut new_jvm = arguments_a.jvm.clone();
                if new_jvm.is_none() {
                    new_jvm = arguments_b.jvm.clone();
                } else if let Some(b_jvm) = arguments_b.jvm.clone() {
                    new_jvm.as_mut().unwrap().extend(b_jvm);
                }
                return Ok(VersionJSON::New { base: c, arguments: Arguments { game: new_game, jvm: new_jvm } });
            }
            return Result::Err(anyhow!("try to merge different kinds of version json!").into()); // TODO: i18n
        },
    }
}

/// Expand a [Maven coordinate](https://maven.apache.org/pom.html#Maven_Coordinates) to a path.
pub fn expand_maven_id(id: &str) -> String {
    ArtifactCoordinate::from(id).to_path()
}

// or

#[cfg(feature="mod_loaders")]
/// Parse a [Maven version range](https://maven.apache.org/enforcer/enforcer-rules/versionRanges.html).
pub fn parse_maven_version_range(v: &str) -> anyhow::Result<Vec<VersionBound>> {
    let error = anyhow!(format!("{v} isn't a vaild Maven version range!")); // TODO: i18n
    if !v.contains(",") {
        if v.starts_with("[") && v.ends_with("]") {
            let version = &v[1..v.len() - 1];
            return Ok(vec![VersionBound::new_one(Requirement {
                op: versions::Op::Exact,
                version: Some(Versioning::parse(version).map_err(|_|error)?.1)
            })]);
        }
        return Ok(vec![VersionBound::new_one(Requirement {
            op: versions::Op::GreaterEq,
            version: Some(Versioning::parse(v).map_err(|_|error)?.1)
        })]);
    }
    let splited = v.split(",");
    let mut ret = vec![];
    for [lower, upper] in splited.array_chunks() {
        let mut bounds = vec![];
        if lower != "(" && lower.starts_with("(") {
            bounds.push(Requirement {
                op: versions::Op::Greater,
                version: Versioning::new(&lower[1..])
            });
        } else if lower.starts_with("[") {
            bounds.push(Requirement {
                op: versions::Op::GreaterEq,
                version: Versioning::new(&lower[1..])
            });
        } else if lower != "(" {
            return Err(error);
        }
        if upper != ")" && lower.ends_with(")") {
            bounds.push(Requirement {
                op: versions::Op::Greater,
                version: Versioning::new(&upper[..upper.len() - 1])
            });
        }
        if lower.ends_with("]") {
            bounds.push(Requirement {
                op: versions::Op::GreaterEq,
                version: Versioning::new(&upper[..upper.len() - 1])
            });
        } else if upper != ")" {
            return Err(error.into());
        }
        ret.push(VersionBound(bounds))
    }
    Ok(ret)
}

#[cfg(feature="mod_loaders")]
/// Deserialize a [Maven version range](https://maven.apache.org/enforcer/enforcer-rules/versionRanges.html).
pub fn deserialize_maven_version_range<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<VersionBound>, D::Error> {
    struct StructVisitor;
    impl <'de> Visitor<'de> for StructVisitor {
        type Value = Vec<VersionBound>;
    
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("A Maven Version Range")
        }

        fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error {
            parse_maven_version_range(v).map_err(|_|serde::de::Error::invalid_value(serde::de::Unexpected::Str(v), &"A Maven Version Range"))
        }
    }
    deserializer.deserialize_str(StructVisitor)
}

/// Converts silly newlines to "\\n" in JSONs.
pub fn json_newline_transform(source: &String) -> String {
    let mut result = vec![];
    let mut is_str = 0;
    for  i in source.chars() {
        match i {
            '"' => {
                if is_str == 1 {
                    is_str = 0;
                }
                else if is_str == 0 {
                    is_str = 1;
                }
                result.push(i);
            },
            '\'' => {
                if is_str == 2 {
                    is_str = 0;
                }
                else if is_str == 0 {
                    is_str = 2;
                }
                result.push(i);
            }
            '\n' => {
                if is_str != 0 {
                    result.push('\\');
                    result.push('n');
                }
            }
            _ => {
                result.push(i)
            }
        }
    }
    return result.into_iter().collect();
}
