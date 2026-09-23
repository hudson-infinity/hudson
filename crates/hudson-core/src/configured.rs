use crate::{
    adapters::{
        anthropic::AnthropicModel, chat::ChatModel, http_tools::HttpTools, models::ModelExecutor,
        tools::ToolRegistry,
    },
    models::*,
    runtime::Runtime,
    security::Policy,
    skills::{Skill, SkillCatalog},
    storage::Store,
};
use hudson_harness::AgentLoop;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    name: String,
    instructions: String,
    #[serde(default)]
    context: Option<crate::context::ContextPolicy>,
    #[serde(default)]
    allow_user_input: bool,
    #[serde(default)]
    model: Option<String>,
    #[serde(default = "endpoint")]
    endpoint: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default = "output_tokens")]
    max_output_tokens: u32,
    #[serde(default)]
    legacy_token_limit: bool,
    #[serde(default = "version")]
    version: u32,
    #[serde(default)]
    skills: Vec<Skill>,
    #[serde(default)]
    skill_files: Vec<SkillFile>,
    #[serde(default)]
    output_schema: Option<serde_json::Value>,
    #[serde(default)]
    input_schema: Option<serde_json::Value>,
    #[serde(default)]
    limits: Limits,
    #[serde(default)]
    goal: Option<Goal>,
    #[serde(default)]
    http_tools: Vec<HttpTool>,
    #[serde(default)]
    shared_model_budget: Option<SharedBudget>,
    #[serde(default)]
    subagents: Vec<Definition>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillFile {
    name: String,
    description: String,
    path: std::path::PathBuf,
}

fn load_skill_files(
    definition: &mut Definition,
    base: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for source in std::mem::take(&mut definition.skill_files) {
        let path = if source.path.is_absolute() {
            source.path
        } else {
            base.join(source.path)
        };
        definition.skills.push(Skill::from_markdown(
            source.name,
            source.description,
            &path,
        )?);
    }
    if !definition.skills.is_empty() {
        SkillCatalog::new(definition.skills.clone())?;
    }
    for child in &mut definition.subagents {
        load_skill_files(child, base)?;
    }
    Ok(())
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedBudget {
    group: String,
    limit: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpTool {
    name: String,
    description: String,
    endpoint: String,
    input_schema: serde_json::Value,
    #[serde(default)]
    output_schema: Option<serde_json::Value>,
    effect: Effect,
    #[serde(default)]
    token_env: Option<String>,
    #[serde(default)]
    require_approval: bool,
}

fn output_tokens() -> u32 {
    1024
}

fn selected_model(definition: &Definition) -> Result<&str, Box<dyn std::error::Error>> {
    match definition.model.as_deref() {
        Some(model) if !model.trim().is_empty() => Ok(model),
        Some(_) => Err("model must not be empty".into()),
        None if matches!(definition.provider.as_str(), "" | "openai") => Ok("gpt-4.1-mini"),
        None => Err("set a model for the selected provider explicitly".into()),
    }
}
fn endpoint() -> String {
    "https://api.openai.com/v1/chat/completions".into()
}
fn version() -> u32 {
    1
}

fn model_key(name: &str, required: bool) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        _ if required => Err("configured model credential is missing or empty".into()),
        _ => Ok(None),
    }
}

pub type AgentRuntime = Runtime<AgentLoop, Box<dyn ModelExecutor>, HttpTools>;
type ChildRuntimes =
    std::collections::BTreeMap<VersionRef, std::sync::Arc<std::sync::Mutex<AgentRuntime>>>;

/// One configured tree, preserving each agent's own model/tool executor on resume.
pub struct ConfiguredTree {
    pub runtime: AgentRuntime,
    pub reference: VersionRef,
    pub goal: Option<Goal>,
    children: ChildRuntimes,
}
impl ConfiguredTree {
    pub fn bindings(&self) -> std::collections::BTreeMap<VersionRef, Option<Goal>> {
        let mut bindings = self
            .children
            .keys()
            .cloned()
            .map(|reference| (reference, None))
            .collect::<std::collections::BTreeMap<_, _>>();
        bindings.insert(self.reference.clone(), self.goal.clone());
        bindings
    }
    pub fn tick(&mut self, actor: &Actor, id: uuid::Uuid) -> crate::Result<RunView> {
        let run = self.runtime.store.inspect(actor, id)?;
        if run.agent_ref == self.reference {
            self.runtime
                .store
                .validate_resume(actor, id, &self.reference, &self.goal)?;
            return self.runtime.tick(actor, id);
        }
        let child = self.children.get(&run.agent_ref).ok_or_else(|| {
            crate::Error::Conflict("agent version is not configured in this host".into())
        })?;
        self.runtime
            .store
            .validate_resume(actor, id, &run.agent_ref, &None)?;
        child
            .lock()
            .map_err(|_| crate::Error::Conflict("child runtime unavailable".into()))?
            .tick(actor, id)
    }
}

/// Independently locked agent executors for a durable scheduler. Different team
/// members can execute concurrently while each configured executor stays serial.
pub struct ScheduledTree {
    pub store: Store,
    runtimes: ChildRuntimes,
    bindings: std::collections::BTreeMap<VersionRef, Option<Goal>>,
}
impl ConfiguredTree {
    pub fn into_scheduled(self) -> ScheduledTree {
        let bindings = self.bindings();
        let store = self.runtime.store.clone();
        let mut runtimes = self.children;
        runtimes.insert(
            self.reference,
            std::sync::Arc::new(std::sync::Mutex::new(self.runtime)),
        );
        ScheduledTree {
            store,
            runtimes,
            bindings,
        }
    }
}
impl ScheduledTree {
    pub fn tick(&self, actor: &Actor, id: uuid::Uuid) -> crate::Result<RunView> {
        let run = self.store.inspect(actor, id)?;
        let goal = self
            .bindings
            .get(&run.agent_ref)
            .ok_or_else(|| crate::Error::Conflict("agent version is not configured".into()))?;
        self.store
            .validate_resume(actor, id, &run.agent_ref, goal)?;
        self.runtimes
            .get(&run.agent_ref)
            .ok_or(crate::Error::NotFound)?
            .lock()
            .map_err(|_| crate::Error::Conflict("agent runtime unavailable".into()))?
            .tick(actor, id)
    }
}

fn build(
    definition: Definition,
    store: Store,
    actor: &Actor,
    budget: Option<&SharedBudget>,
    children: &mut ChildRuntimes,
    deferred: bool,
) -> Result<(AgentRuntime, VersionRef), Box<dyn std::error::Error>> {
    let selected_model = selected_model(&definition)?.to_owned();
    let is_team = !definition.subagents.is_empty();
    let transport_fingerprint = crate::definitions::digest(&(
        &definition.provider,
        &definition.endpoint,
        definition.max_output_tokens,
        definition.legacy_token_limit,
    ))?;
    let model: Box<dyn ModelExecutor> = match definition.provider.as_str() {
        "" | "openai" => {
            let key = model_key(
                definition
                    .api_key_env
                    .as_deref()
                    .unwrap_or("OPENAI_API_KEY"),
                definition.api_key_env.is_some() || definition.endpoint == endpoint(),
            )?;
            Box::new(
                ChatModel::new(&definition.endpoint, key, definition.max_output_tokens)?
                    .with_legacy_token_limit(definition.legacy_token_limit),
            )
        }
        "anthropic" => {
            let url = if definition.endpoint == endpoint() {
                "https://api.anthropic.com/v1/messages"
            } else {
                &definition.endpoint
            };
            let key = model_key(
                definition
                    .api_key_env
                    .as_deref()
                    .unwrap_or("ANTHROPIC_API_KEY"),
                definition.api_key_env.is_some() || url == "https://api.anthropic.com/v1/messages",
            )?;
            Box::new(AnthropicModel::new(url, key, definition.max_output_tokens)?)
        }
        "gemini" | "ollama" => {
            let default_url = if definition.provider == "gemini" {
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
            } else {
                "http://127.0.0.1:11434/v1/chat/completions"
            };
            let url = if definition.endpoint == endpoint() {
                default_url
            } else {
                &definition.endpoint
            };
            let key_env =
                definition
                    .api_key_env
                    .as_deref()
                    .or(if definition.provider == "gemini" {
                        Some("GEMINI_API_KEY")
                    } else {
                        None
                    });
            let key = key_env
                .map(|name| model_key(name, true))
                .transpose()?
                .flatten();
            Box::new(
                ChatModel::new(url, key, definition.max_output_tokens)?
                    .with_legacy_token_limit(true),
            )
        }
        _ => return Err("unsupported provider".into()),
    };
    let model: Box<dyn ModelExecutor> = if let Some(budget) = budget {
        Box::new(crate::budgets::BudgetedModel::new(
            model,
            store.clone(),
            &actor.workspace_id,
            &budget.group,
            budget.limit,
        )?)
    } else {
        model
    };
    let mut registry = ToolRegistry::new();
    let mut bindings = Vec::new();
    if let Some(context) = &definition.context {
        let policy_ref = format!("context:{}:{}", definition.name, definition.version);
        let tool = crate::context::register(
            &mut registry,
            store.clone(),
            &actor.workspace_id,
            &policy_ref,
            context,
        )?;
        bindings.push(AgentTool {
            tool_ref: tool.reference(),
            alias: tool.name.clone(),
        });
        store.ensure_tool(tool)?;
        store.ensure_policy(
            &actor.workspace_id,
            &policy_ref,
            Policy {
                actors: [actor.id.clone()].into(),
                approvers: Default::default(),
                require_approval: false,
            },
        )?;
    }

    if !definition.skills.is_empty() {
        let mut tool = SkillCatalog::new(definition.skills)?.register(
            &mut registry,
            &actor.workspace_id,
            "skills",
        )?;
        tool.created_at = 0;
        bindings.push(AgentTool {
            tool_ref: tool.reference(),
            alias: tool.name.clone(),
        });
        store.ensure_tool(tool)?;
        store.ensure_policy(
            &actor.workspace_id,
            "skills",
            Policy {
                actors: [actor.id.clone()].into(),
                approvers: Default::default(),
                require_approval: false,
            },
        )?;
    }
    for child_definition in definition.subagents {
        let alias = format!("delegate_{}", child_definition.name);
        let description = format!("Delegate a task to specialist {}. Inspect its returned status before claiming completion.", child_definition.name);
        let (child, child_ref) = build(
            child_definition,
            store.clone(),
            actor,
            budget,
            children,
            deferred,
        )?;
        let child = std::sync::Arc::new(std::sync::Mutex::new(child));
        children.insert(child_ref.clone(), child.clone());
        let delegation_tools = crate::subagents::register_shared(
            &mut registry,
            child,
            actor.clone(),
            child_ref,
            &alias,
            &description,
            "configured-subagents",
            deferred,
        )?;
        for tool in delegation_tools {
            bindings.push(AgentTool {
                tool_ref: tool.reference(),
                alias: tool.name.clone(),
            });
            store.ensure_tool(tool)?;
        }
    }
    store.ensure_policy(
        &actor.workspace_id,
        "configured-subagents",
        Policy {
            actors: [actor.id.clone()].into(),
            approvers: Default::default(),
            require_approval: false,
        },
    )?;
    let mut executor = HttpTools::new(registry)?;
    for spec in definition.http_tools {
        let token = spec
            .token_env
            .as_ref()
            .map(|name| {
                std::env::var(name).map_err(|_| "HTTP tool credential environment variable missing")
            })
            .transpose()?;
        executor.bind(&spec.endpoint, token)?;
        let tool = Tool {
            id: format!("{}:{}", definition.name, spec.name),
            workspace_id: actor.workspace_id.clone(),
            version: definition.version,
            schema_version: SCHEMA_VERSION,
            name: spec.name.clone(),
            description: spec.description,
            input_schema: spec.input_schema,
            output_schema: spec.output_schema,
            execution: Execution::Http {
                endpoint: spec.endpoint,
            },
            credential_ref: spec.token_env,
            policy_ref: format!(
                "http:{}:{}:{}",
                definition.name, definition.version, spec.name
            ),
            effect: spec.effect,
            created_at: 0,
        };
        bindings.push(AgentTool {
            tool_ref: tool.reference(),
            alias: tool.name.clone(),
        });
        store.ensure_policy(
            &actor.workspace_id,
            &tool.policy_ref,
            Policy {
                actors: [actor.id.clone()].into(),
                approvers: [actor.id.clone()].into(),
                require_approval: spec.require_approval,
            },
        )?;
        store.ensure_tool(tool)?;
    }
    let agent = Agent {
        id: definition.name.clone(),
        workspace_id: actor.workspace_id.clone(),
        version: definition.version,
        schema_version: SCHEMA_VERSION,
        name: definition.name,
        instructions: if deferred && is_team {
            format!("{}\n\nDelegation submits durable child runs. The scheduler waits for the delegated team before your next model turn. Use each join_delegate tool with the returned child_run ID to retrieve the completed result; the original delegation receipt only reports submission. Do not claim success without inspecting each child result.", definition.instructions)
        } else {
            definition.instructions
        },
        allow_user_input: definition.allow_user_input,
        model: selected_model,
        tools: bindings,
        input_schema: definition.input_schema,
        output_schema: definition.output_schema,
        limits: definition.limits,
        created_at: 0,
    };
    let reference = agent.reference();
    store.ensure_agent(agent)?;
    store.bind_context_policy(&actor.workspace_id, &reference, definition.context.as_ref())?;
    store.bind_model_transport(&actor.workspace_id, &reference, &transport_fingerprint)?;
    Ok((Runtime::new(store, AgentLoop, model, executor), reference))
}

fn validate_tree(
    definition: &Definition,
    depth: usize,
    count: &mut usize,
    names: &mut std::collections::BTreeSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    *count += 1;
    if depth > 4 || *count > 32 {
        return Err("agent tree exceeds depth 4 or 32 total agents".into());
    }
    if !names.insert(definition.name.clone()) {
        return Err("agent names must be unique within the tree".into());
    }
    if depth > 0 && (definition.shared_model_budget.is_some() || definition.goal.is_some()) {
        return Err(
            "child configs inherit the root budget; put child acceptance criteria in output_schema"
                .into(),
        );
    }
    for child in &definition.subagents {
        validate_tree(child, depth + 1, count, names)?;
    }
    Ok(())
}

fn validate_contracts(definition: &Definition) -> Result<(), Box<dyn std::error::Error>> {
    if definition.name.trim().is_empty()
        || definition.instructions.trim().is_empty()
        || definition.version == 0
    {
        return Err("agent requires a name, instructions, positive version, and model".into());
    }
    if !matches!(
        definition.provider.as_str(),
        "" | "openai" | "anthropic" | "gemini" | "ollama"
    ) {
        return Err("unsupported provider".into());
    }
    selected_model(definition)?;
    definition.limits.validate()?;
    if let Some(context) = &definition.context {
        context.validate()?;
    }
    if definition.max_output_tokens == 0 {
        return Err("max_output_tokens must be positive".into());
    }
    if let Some(schema) = &definition.input_schema {
        crate::definitions::validate_schema(schema)?;
    }
    if let Some(schema) = &definition.output_schema {
        crate::definitions::validate_schema(schema)?;
    }
    if let Some(goal) = &definition.goal {
        if goal.objective.trim().is_empty() || goal.objective.len() > 16384 {
            return Err("invalid goal objective".into());
        }
        crate::definitions::validate_schema(&goal.success_schema)?;
        for criterion in &goal.criteria {
            criterion.validate()?;
        }
    }
    if let Some(budget) = &definition.shared_model_budget {
        if budget.group.trim().is_empty() || budget.group.len() > 256 || budget.limit == 0 {
            return Err("invalid shared budget".into());
        }
    }
    let mut names = std::collections::BTreeSet::new();
    if definition.context.is_some() {
        names.insert("read_context_artifact".to_owned());
    }
    if definition.allow_user_input {
        names.insert("ask_user".to_owned());
    }
    if !definition.skills.is_empty() {
        names.insert("load_skill".to_owned());
    }
    for tool in &definition.http_tools {
        if tool.name.trim().is_empty() || !names.insert(tool.name.clone()) {
            return Err("empty or duplicate tool name".into());
        }
        crate::definitions::validate_schema(&tool.input_schema)?;
        if let Some(schema) = &tool.output_schema {
            crate::definitions::validate_schema(schema)?;
        }
    }
    for child in &definition.subagents {
        for name in [
            format!("delegate_{}", child.name),
            format!("join_delegate_{}", child.name),
        ] {
            if !names.insert(name) {
                return Err("delegation tool name conflicts with another tool".into());
            }
        }
        validate_contracts(child)?;
    }
    Ok(())
}

/// Validated, immutable startup configuration shared by CLI and HTTP hosts.
pub struct Configuration(Definition);
impl Configuration {
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut definition: Definition = serde_json::from_slice(&std::fs::read(path)?)?;
        validate_tree(&definition, 0, &mut 0, &mut Default::default())?;
        if !definition.subagents.is_empty() && definition.shared_model_budget.is_none() {
            return Err("subagent trees require shared_model_budget on the root".into());
        }
        load_skill_files(
            &mut definition,
            path.parent().unwrap_or(std::path::Path::new(".")),
        )?;
        validate_contracts(&definition)?;
        Ok(Self(definition))
    }

    pub fn build(
        self,
        store: Store,
        actor: &Actor,
    ) -> Result<(AgentRuntime, VersionRef, Option<Goal>), Box<dyn std::error::Error>> {
        let tree = self.build_tree(store, actor)?;
        Ok((tree.runtime, tree.reference, tree.goal))
    }
    pub fn build_tree(
        self,
        store: Store,
        actor: &Actor,
    ) -> Result<ConfiguredTree, Box<dyn std::error::Error>> {
        self.build_with_scheduler(store, actor, false)
    }

    /// Build a tree whose child tools only submit/inspect runs. The caller must
    /// schedule children and wait for them before advancing the parent model.
    pub fn build_temporal_tree(
        self,
        store: Store,
        actor: &Actor,
    ) -> Result<ConfiguredTree, Box<dyn std::error::Error>> {
        self.build_with_scheduler(store, actor, true)
    }

    fn build_with_scheduler(
        self,
        store: Store,
        actor: &Actor,
        deferred: bool,
    ) -> Result<ConfiguredTree, Box<dyn std::error::Error>> {
        let mut definition = self.0;
        let goal = definition.goal.take();
        let budget = definition.shared_model_budget.take();
        let mut children = ChildRuntimes::new();
        let (runtime, reference) = build(
            definition,
            store,
            actor,
            budget.as_ref(),
            &mut children,
            deferred,
        )?;
        Ok(ConfiguredTree {
            runtime,
            reference,
            goal,
            children,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn definition(name: &str) -> Definition {
        serde_json::from_value(serde_json::json!({"name":name,"instructions":"help"})).unwrap()
    }
    #[test]
    fn user_input_reserves_its_tool_name_only_when_enabled() {
        let mut config: Definition = serde_json::from_value(serde_json::json!({
            "name":"root","instructions":"help","allow_user_input":true,
            "http_tools":[{"name":"ask_user","description":"custom","endpoint":"http://127.0.0.1:9000/tool","input_schema":{"type":"object"},"effect":"read"}]
        })).unwrap();
        assert!(validate_contracts(&config).is_err());
        config.allow_user_input = false;
        validate_contracts(&config).unwrap();
    }
    #[test]
    fn rejects_duplicate_names_and_child_budget_overrides() {
        let mut root = definition("root");
        root.subagents.push(definition("root"));
        assert!(validate_tree(&root, 0, &mut 0, &mut Default::default()).is_err());
        root.subagents = vec![definition("child")];
        root.subagents[0].shared_model_budget = Some(SharedBudget {
            group: "escape".into(),
            limit: 100,
        });
        assert!(validate_tree(&root, 0, &mut 0, &mut Default::default()).is_err());
    }
    #[test]
    fn rejects_invalid_contracts_before_startup() {
        let mut config = definition("root");
        config.output_schema = Some(serde_json::json!({"type":"not-a-json-type"}));
        assert!(validate_contracts(&config).is_err());
        config.output_schema = None;
        config.input_schema = Some(serde_json::json!({"type":"invalid-type"}));
        assert!(validate_contracts(&config).is_err());
        config.input_schema = None;
        config.shared_model_budget = Some(SharedBudget {
            group: "test".into(),
            limit: 0,
        });
        assert!(validate_contracts(&config).is_err());
        config.shared_model_budget = None;
        config.provider = "unsupported".into();
        assert!(validate_contracts(&config).is_err());
    }

    #[test]
    fn rejects_excessive_delegation_depth() {
        let mut tree = definition("leaf");
        for index in 0..5 {
            let mut parent = definition(&format!("node-{index}"));
            parent.subagents.push(tree);
            tree = parent;
        }
        assert!(validate_tree(&tree, 0, &mut 0, &mut Default::default()).is_err());
    }
    #[test]
    fn configuration_and_publication_share_limit_validation() {
        for field in [
            "max_harness_steps",
            "max_model_calls",
            "max_operations",
            "max_batch_size",
            "max_context_bytes",
            "max_payload_bytes",
        ] {
            let mut limits = serde_json::to_value(Limits::default()).unwrap();
            limits[field] = serde_json::json!(0);
            let mut root = definition("root");
            root.limits = serde_json::from_value(limits).unwrap();
            assert!(validate_contracts(&root).is_err(), "accepted zero {field}");
            let mut parent = definition("parent");
            parent.subagents.push(root);
            assert!(
                validate_contracts(&parent).is_err(),
                "accepted invalid child {field}"
            );
        }
    }
    #[test]
    fn default_gpt_does_not_reserve_a_model_name_on_other_protocols() {
        let mut config = definition("root");
        assert_eq!(selected_model(&config).unwrap(), "gpt-4.1-mini");
        for provider in ["openai", "anthropic", "gemini", "ollama"] {
            config.provider = provider.into();
            config.model = Some("gpt-4.1-mini".into());
            assert_eq!(selected_model(&config).unwrap(), "gpt-4.1-mini");
            validate_contracts(&config).unwrap();
            config.model = Some("custom/model-alias:version".into());
            assert_eq!(
                selected_model(&config).unwrap(),
                "custom/model-alias:version"
            );
            config.model = Some(" ".into());
            assert!(validate_contracts(&config).is_err());
            config.model = None;
            assert_eq!(selected_model(&config).is_ok(), provider == "openai");
        }
    }
}
