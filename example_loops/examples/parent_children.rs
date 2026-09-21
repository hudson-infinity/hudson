//! Local task-tree illustration. No distributed scheduler or shared credentials.
use std::collections::BTreeSet;
use std::thread;

struct Child {
    id: &'static str,
    permissions: BTreeSet<&'static str>,
    reserved_units: u32,
}

fn main() {
    let parent_permissions = BTreeSet::from(["search", "read"]);
    let parent_budget = 10;
    let children = vec![
        Child {
            id: "research",
            permissions: BTreeSet::from(["search"]),
            reserved_units: 4,
        },
        Child {
            id: "verify",
            permissions: BTreeSet::from(["read"]),
            reserved_units: 3,
        },
    ];
    let reserved: u32 = children.iter().map(|c| c.reserved_units).sum();
    assert!(
        reserved <= parent_budget,
        "reserve all child budgets before starting"
    );
    assert!(children
        .iter()
        .all(|c| c.permissions.is_subset(&parent_permissions)));
    assert!(!BTreeSet::from(["delete"]).is_subset(&parent_permissions));

    let results = thread::scope(|scope| {
        let handles: Vec<_> = children
            .into_iter()
            .map(|child| {
                scope.spawn(move || {
                    println!("child {}: scoped tools {:?}", child.id, child.permissions);
                    // Each child has its own local context; mock cost stays within its reservation.
                    (child.id, "bounded findings", child.reserved_units - 1)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("mock child panicked"))
            .collect::<Vec<_>>()
    });
    let spent: u32 = results.iter().map(|r| r.2).sum();
    assert!(spent <= reserved);
    println!(
        "parent collects {} results; used {spent}/{parent_budget} units",
        results.len()
    );
    println!("parent completes after joining its children");
}
