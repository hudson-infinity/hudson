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
    assert!(old.input_schema["properties"].get("resource").is_none());
    let compatible =
        SkillCatalog::with_packages(vec![skill("Check missing values first.")], vec![])
            .unwrap()
            .register(&mut ToolRegistry::new(), "a", "read")
            .unwrap();
    assert_eq!(old.id, compatible.id);
    assert_eq!(old.input_schema, compatible.input_schema);
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

#[test]
fn portable_packages_freeze_lazy_resources_and_reject_traversal() {
    use hudson_core::skills::SkillPackage;
    let root = std::env::temp_dir().join(format!("hudson-package-{}", uuid::Uuid::new_v4()));
    let directory = root.join("analyze-data");
    std::fs::create_dir_all(directory.join("references")).unwrap();
    std::fs::write(directory.join("SKILL.md"),"---\nname: analyze-data\ndescription: Analyze company data\nlicense: MIT\nmetadata:\n  version: '1'\n---\nConsult references/metrics.md before calculating.\n").unwrap();
    let path = directory.join("references/metrics.md");
    std::fs::write(&path, "Revenue excludes refunds.").unwrap();
    let package = SkillPackage::load(&directory).unwrap();
    let mut registry = ToolRegistry::new();
    let old = SkillCatalog::with_packages(vec![], vec![package])
        .unwrap()
        .register(&mut registry, "a", "read")
        .unwrap();
    std::fs::write(&path, "Revenue includes refunds.").unwrap();
    let new = SkillCatalog::with_packages(vec![], vec![SkillPackage::load(&directory).unwrap()])
        .unwrap()
        .register(&mut registry, "a", "read")
        .unwrap();
    assert_ne!(old.id, new.id);
    assert!(!old.description.contains("Revenue"));
    let output = registry
        .execute(Invocation {
            operation_id: uuid::Uuid::new_v4(),
            tool: &old,
            arguments: &json!({"name":"analyze-data"}),
        })
        .unwrap();
    assert_eq!(output["resources"], json!(["references/metrics.md"]));
    let output = registry
        .execute(Invocation {
            operation_id: uuid::Uuid::new_v4(),
            tool: &old,
            arguments: &json!({"name":"analyze-data","resource":"references/metrics.md"}),
        })
        .unwrap();
    assert_eq!(output["contents"], "Revenue excludes refunds.");
    for path in ["../secret", "/etc/passwd", "references/../SKILL.md"] {
        assert!(registry
            .execute(Invocation {
                operation_id: uuid::Uuid::new_v4(),
                tool: &old,
                arguments: &json!({"name":"analyze-data","resource":path})
            })
            .is_err());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/passwd", directory.join("references/outside.md")).unwrap();
        assert!(SkillPackage::load(&directory).is_err());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn portable_resources_are_bounded_utf8_pages_with_exact_reassembly() {
    use hudson_core::skills::SkillPackage;
    let directory = std::env::temp_dir().join(format!("hudson-paging-{}", uuid::Uuid::new_v4()));
    let package = directory.join("large-resource");
    std::fs::create_dir_all(package.join("references")).unwrap();
    std::fs::write(
        package.join("SKILL.md"),
        "---\nname: large-resource\ndescription: Large resource\n---\nRead the references.",
    )
    .unwrap();
    let original = "多字\n\u{0001}".repeat(12000);
    assert!(original.len() <= 131072);
    std::fs::write(package.join("references/data.txt"), &original).unwrap();
    let mut registry = ToolRegistry::new();
    let tool = SkillCatalog::with_packages(vec![], vec![SkillPackage::load(&package).unwrap()])
        .unwrap()
        .register(&mut registry, "workspace", "read")
        .unwrap();
    let mut joined = String::new();
    let mut offset = 0;
    loop {
        let value=registry.execute(Invocation{operation_id:uuid::Uuid::new_v4(),tool:&tool,arguments:&json!({"name":"large-resource","resource":"references/data.txt","offset":offset})}).unwrap();
        assert!(serde_json::to_vec(&value).unwrap().len() < 65536);
        joined.push_str(value["contents"].as_str().unwrap());
        let Some(next) = value["next_offset"].as_u64() else {
            break;
        };
        assert!(next > offset);
        offset = next;
    }
    assert_eq!(joined, original);
    assert!(registry
        .execute(Invocation {
            operation_id: uuid::Uuid::new_v4(),
            tool: &tool,
            arguments: &json!({"name":"large-resource","resource":"references/data.txt","offset":1})
        })
        .is_err());
    std::fs::remove_dir_all(directory).unwrap();
}
