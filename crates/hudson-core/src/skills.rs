//! Application-supplied skill instructions, loaded on demand through a read tool.
use crate::{
    adapters::tools::{ExecutionError, ToolRegistry},
    definitions::digest,
    models::*,
    Error, Result,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

impl Skill {
    /// Load a trusted, explicitly configured UTF-8 Markdown file. The contents
    /// become part of the immutable catalog; no filesystem access occurs in the loop.
    pub fn from_markdown(
        name: impl Into<String>,
        description: impl Into<String>,
        path: &std::path::Path,
    ) -> Result<Self> {
        use std::io::Read;
        let file = std::fs::File::open(path)
            .map_err(|_| Error::Invalid("cannot open configured skill file".into()))?;
        if !file
            .metadata()
            .map_err(|_| Error::Invalid("cannot inspect skill file".into()))?
            .is_file()
        {
            return Err(Error::Invalid("skill source must be a regular file".into()));
        }
        let mut bytes = Vec::new();
        file.take(32769)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Invalid("cannot read skill file".into()))?;
        if bytes.len() > 32768 {
            return Err(Error::Invalid("skill file exceeds 32 KiB".into()));
        }
        let instructions = String::from_utf8(bytes)
            .map_err(|_| Error::Invalid("skill file must be UTF-8".into()))?;
        let skill = Self {
            name: name.into(),
            description: description.into(),
            instructions,
        };
        SkillCatalog::new(vec![skill.clone()])?;
        Ok(skill)
    }
}

/// A frozen catalog is bound to its content digest, so updating a skill cannot
/// silently change the instructions available to an already published Agent.
pub struct SkillCatalog {
    skills: BTreeMap<String, Skill>,
    digest: String,
}

impl SkillCatalog {
    pub fn new(skills: Vec<Skill>) -> Result<Self> {
        let mut entries = BTreeMap::new();
        for skill in skills {
            if skill.name.is_empty()
                || skill.name.len() > 64
                || !skill
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || skill.description.trim().is_empty()
                || skill.description.len() > 1024
                || skill.instructions.trim().is_empty()
                || skill.instructions.len() > 32768
            {
                return Err(Error::Invalid("skill requires a short identifier, description, and instructions of at most 32 KiB".into()));
            }
            if entries.insert(skill.name.clone(), skill).is_some() {
                return Err(Error::Conflict("duplicate skill name".into()));
            }
        }
        if entries.is_empty() || entries.len() > 64 {
            return Err(Error::Invalid(
                "skill catalog must contain 1 to 64 entries".into(),
            ));
        }
        let digest = digest(&entries)?;
        Ok(Self {
            skills: entries,
            digest,
        })
    }

    /// Register the frozen catalog and return its Tool definition. Publish the
    /// tool and grant its policy using the same APIs as any application tool.
    pub fn register(
        self,
        registry: &mut ToolRegistry,
        workspace: &str,
        policy: &str,
    ) -> Result<Tool> {
        let key = format!("hudson.skills.{}", self.digest);
        let summary = self
            .skills
            .values()
            .map(|s| json!({"name":s.name,"description":s.description}))
            .collect::<Vec<_>>();
        let names = self.skills.keys().cloned().collect::<Vec<_>>();
        let tool = Tool {
            id:key.clone(), workspace_id:workspace.into(), version:1, schema_version:SCHEMA_VERSION,
            name:"load_skill".into(),
            description:format!("Load instructions for a relevant skill before working on its task. Available skills: {}", json!(summary)),
            input_schema:json!({"type":"object","properties":{"name":{"type":"string","enum":names}},"required":["name"],"additionalProperties":false}),
            output_schema:None, execution:Execution::Registered { key:key.clone() },
            credential_ref:None, policy_ref:policy.into(), effect:Effect::Read, created_at:now(),
        };
        registry.register(key, move |call| {
            let skill = call.arguments["name"]
                .as_str()
                .and_then(|name| self.skills.get(name))
                .ok_or_else(|| {
                    ExecutionError::Failed("skill not found in this agent's catalog".into())
                })?;
            Ok(json!({"name":skill.name,"instructions":skill.instructions}))
        })?;
        Ok(tool)
    }
}
