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

/// A portable Agent Skills package frozen at configuration time. Resources are
/// served as data only; this loader never executes scripts or follows symlinks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillPackage {
    pub skill: Skill,
    pub resources: BTreeMap<String, String>,
}

impl SkillPackage {
    pub fn load(directory: &std::path::Path) -> Result<Self> {
        use std::path::Path;
        #[derive(Deserialize)]
        struct Frontmatter {
            name: String,
            description: String,
        }
        fn read(path: &Path, limit: usize) -> Result<String> {
            use std::io::Read;
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|_| Error::Invalid("cannot inspect skill package resource".into()))?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(Error::Invalid(
                    "skill resources must be regular files without symlinks".into(),
                ));
            }
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .map_err(|_| Error::Invalid("cannot open skill resource".into()))?
                .take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::Invalid("cannot read skill resource".into()))?;
            if bytes.len() > limit {
                return Err(Error::Invalid("skill resource exceeds size limit".into()));
            }
            String::from_utf8(bytes)
                .map_err(|_| Error::Invalid("skill resources must be UTF-8".into()))
        }
        fn collect(
            root: &Path,
            dir: &Path,
            resources: &mut BTreeMap<String, String>,
            total: &mut usize,
            depth: usize,
            entries: &mut usize,
        ) -> Result<()> {
            if depth > 8 {
                return Err(Error::Invalid("skill package nesting exceeds limit".into()));
            }
            for entry in std::fs::read_dir(dir)
                .map_err(|_| Error::Invalid("cannot read skill directory".into()))?
            {
                *entries += 1;
                if *entries > 256 {
                    return Err(Error::Invalid(
                        "skill package exceeds 256 filesystem entries".into(),
                    ));
                }
                let entry = entry.map_err(|_| Error::Invalid("cannot read skill entry".into()))?;
                let kind = entry
                    .file_type()
                    .map_err(|_| Error::Invalid("cannot inspect skill entry".into()))?;
                if kind.is_symlink() {
                    return Err(Error::Invalid(
                        "skill package symlinks are forbidden".into(),
                    ));
                }
                if kind.is_dir() {
                    collect(root, &entry.path(), resources, total, depth + 1, entries)?;
                } else {
                    if resources.len() >= 128 {
                        return Err(Error::Invalid("skill package exceeds 128 resources".into()));
                    }
                    let contents = read(&entry.path(), 131072)?;
                    *total += contents.len();
                    if *total > 2_097_152 {
                        return Err(Error::Invalid("skill package exceeds 2 MiB".into()));
                    }
                    let path = entry.path();
                    let relative = path
                        .strip_prefix(root)
                        .map_err(|_| Error::Invalid("resource outside skill package".into()))?
                        .to_str()
                        .ok_or_else(|| Error::Invalid("skill resource path must be UTF-8".into()))?
                        .replace('\\', "/");
                    resources.insert(relative, contents);
                }
            }
            Ok(())
        }
        let metadata = std::fs::symlink_metadata(directory)
            .map_err(|_| Error::Invalid("cannot inspect skill package".into()))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::Invalid(
                "skill package must be a directory without symlinks".into(),
            ));
        }
        let markdown = read(&directory.join("SKILL.md"), 32768)?;
        let normalized = markdown.replace("\r\n", "\n");
        let rest = normalized
            .strip_prefix("---\n")
            .ok_or_else(|| Error::Invalid("SKILL.md requires YAML frontmatter".into()))?;
        let (header, body) = rest
            .split_once("\n---\n")
            .ok_or_else(|| Error::Invalid("unterminated skill frontmatter".into()))?;
        let metadata: Frontmatter = serde_yaml_ng::from_str(header)
            .map_err(|_| Error::Invalid("invalid skill frontmatter".into()))?;
        let name = &metadata.name;
        if name.is_empty()
            || name.len() > 64
            || name.starts_with('-')
            || name.ends_with('-')
            || name.contains("--")
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || directory.file_name().and_then(|s| s.to_str()) != Some(name)
        {
            return Err(Error::Invalid("portable skill name must match its directory and use lowercase letters, digits, single hyphens".into()));
        }
        let skill = Skill {
            name: metadata.name,
            description: metadata.description,
            instructions: body.into(),
        };
        SkillCatalog::new(vec![skill.clone()])?;
        let mut resources = BTreeMap::new();
        let mut total = markdown.len();
        let mut entries = 0;
        for folder in ["references", "assets", "scripts"] {
            let path = directory.join(folder);
            match std::fs::symlink_metadata(&path) {
                Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => collect(
                    directory,
                    &path,
                    &mut resources,
                    &mut total,
                    0,
                    &mut entries,
                )?,
                Ok(_) => {
                    return Err(Error::Invalid(
                        "skill resource folder must be a directory without symlinks".into(),
                    ))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err(Error::Invalid(
                        "cannot inspect skill resource folder".into(),
                    ))
                }
            }
        }
        Ok(Self { skill, resources })
    }
}

/// A frozen catalog is bound to its content digest, so updating a skill cannot
/// silently change the instructions available to an already published Agent.
pub struct SkillCatalog {
    skills: BTreeMap<String, Skill>,
    digest: String,
    resources: BTreeMap<String, BTreeMap<String, String>>,
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
            resources: BTreeMap::new(),
        })
    }

    pub fn with_packages(mut skills: Vec<Skill>, packages: Vec<SkillPackage>) -> Result<Self> {
        if packages.is_empty() {
            return Self::new(skills);
        }
        let mut resources = BTreeMap::new();
        for package in packages {
            resources.insert(package.skill.name.clone(), package.resources);
            skills.push(package.skill);
        }
        let mut catalog = Self::new(skills)?;
        catalog.digest = digest(&(&catalog.skills, &resources))?;
        catalog.resources = resources;
        Ok(catalog)
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
        let mut input_schema = json!({"type":"object","properties":{"name":{"type":"string","enum":names}},"required":["name"],"additionalProperties":false});
        if !self.resources.is_empty() {
            input_schema["properties"]["resource"] = json!({"type":"string","description":"Optional relative resource path listed by load_skill"});
        }
        let tool = Tool {
            id:key.clone(), workspace_id:workspace.into(), version:1, schema_version:SCHEMA_VERSION,
            name:"load_skill".into(),
            description:format!("Load instructions for a relevant skill before working on its task. Available skills: {}", json!(summary)),
            input_schema,
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
            if self.resources.is_empty() {
                return Ok(json!({"name":skill.name,"instructions":skill.instructions}));
            }
            let resources = self.resources.get(&skill.name);
            if let Some(path) = call.arguments.get("resource").and_then(|value| value.as_str()) {
                let contents = resources.and_then(|entries| entries.get(path)).ok_or_else(|| ExecutionError::Failed("resource not found in frozen skill package".into()))?;
                return Ok(json!({"name":skill.name,"resource":path,"contents":contents}));
            }
            Ok(json!({"name":skill.name,"instructions":skill.instructions,"resources":resources.map(|entries| entries.keys().collect::<Vec<_>>()).unwrap_or_default()}))
        })?;
        Ok(tool)
    }
}
