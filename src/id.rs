use compact_str::CompactString;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct JobId {
    value: u64,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ProjectId {
    /// owner/repo identifier for GitHub
    value: CompactString,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct PipelineId {
    value: u64,
}

impl ProjectId {
    pub fn new<S: Into<CompactString>>(id: S) -> Self {
        Self { value: id.into() }
    }
}

impl PipelineId {
    pub fn new(id: u64) -> Self {
        Self { value: id }
    }
}

impl JobId {
    pub fn new(id: u64) -> Self {
        Self { value: id }
    }
}

impl<'de> Deserialize<'de> for ProjectId {
    fn deserialize<D>(deserializer: D) -> Result<ProjectId, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::fmt;

        struct ProjectIdVisitor;

        impl<'de> Visitor<'de> for ProjectIdVisitor {
            type Value = ProjectId;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or integer representing a project ID")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(ProjectId::new(value))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(ProjectId::new(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(ProjectId::new(value.to_string()))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(ProjectId::new(value.to_string()))
            }
        }

        deserializer.deserialize_any(ProjectIdVisitor)
    }
}

impl<'de> Deserialize<'de> for PipelineId {
    fn deserialize<D>(deserializer: D) -> Result<PipelineId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = u64::deserialize(deserializer)?;
        Ok(PipelineId::new(id))
    }
}

impl<'de> Deserialize<'de> for JobId {
    fn deserialize<D>(deserializer: D) -> Result<JobId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = u64::deserialize(deserializer)?;
        Ok(JobId::new(id))
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl std::fmt::Display for PipelineId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}
