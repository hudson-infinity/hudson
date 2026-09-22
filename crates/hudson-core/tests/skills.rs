use hudson_core::{
    adapters::tools::{Invocation, ToolExecutor, ToolRegistry},
    skills::{Skill, SkillCatalog},
};
use serde_json::json;
fn skill(body: &str) -> Skill {
    Skill {
        name: "analyze-data".into(),
        description: "Analyze a dataset".into(),
        instructions: body.into(),
    }
}
#[test]
fn skill_bodies_are_lazy_and_versions_remain_pinned() {
    let mut registry = ToolRegistry::new();
    let old = SkillCatalog::new(vec![skill("Check missing values first.")])
        .unwrap()
        .register(&mut registry, "a", "read")
        .unwrap();
    let new = SkillCatalog::new(vec![skill("Check column types first.")])
        .unwrap()
        .register(&mut registry, "a", "read")
        .unwrap();
    assert_ne!(old.id, new.id);
    assert!(!old.description.contains("missing values"));
    for (tool, expected) in [
        (&old, "Check missing values first."),
        (&new, "Check column types first."),
    ] {
        let arguments = json!({"name":"analyze-data"});
        let result = registry
            .execute(Invocation {
                operation_id: uuid::Uuid::new_v4(),
                tool,
                arguments: &arguments,
            })
            .unwrap();
        assert_eq!(result["instructions"], expected);
    }
    assert!(registry
        .execute(Invocation {
            operation_id: uuid::Uuid::new_v4(),
            tool: &old,
            arguments: &json!({"name":"other"})
        })
        .is_err());
}
#[test]
fn rejects_ambiguous_or_oversized_catalogs() {
    assert!(SkillCatalog::new(vec![skill("one"), skill("two")]).is_err());
    assert!(SkillCatalog::new(vec![skill(&"x".repeat(32769))]).is_err());
    assert!(SkillCatalog::new(vec![]).is_err());
}

#[test]
fn markdown_load_is_bounded_and_catalog_freezes_content() {
    let directory = std::env::temp_dir().join(format!("hudson-skills-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("analysis.md");
    std::fs::write(&path, "Check missing values.").unwrap();
    let loaded = Skill::from_markdown("analysis", "Analyze data", &path).unwrap();
    std::fs::write(&path, "Changed later.").unwrap();
    assert_eq!(loaded.instructions, "Check missing values.");
    std::fs::write(&path, vec![b'x'; 32769]).unwrap();
    assert!(Skill::from_markdown("analysis", "Analyze data", &path).is_err());
    std::fs::write(&path, [0xff]).unwrap();
    assert!(Skill::from_markdown("analysis", "Analyze data", &path).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}
