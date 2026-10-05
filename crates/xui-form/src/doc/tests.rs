use super::*;

#[test]
fn duplicate_and_cycle_edits_are_plain_data() {
    let mut doc = FormDoc::new("main_form");
    let mut a = Node::new("Panel", "panA");
    a.set_prop("width", Value::Int(200));
    a.set_prop("height", Value::Int(120));
    let mut b = Node::new("Panel", "panB");
    b.parent = Some("panA".to_owned());
    doc.insert(a);
    doc.insert(b);

    assert_eq!(doc.children_of("panA").len(), 1);
    assert!(doc.rename("panA", "panRoot"));
    assert_eq!(
        doc.node("panB").and_then(|n| n.parent.as_deref()),
        Some("panRoot")
    );
    assert!(!doc.rename("panB", "panRoot"), "the new name is taken");
    assert!(doc.reparent("panB", None));
    assert!(doc.node("panB").is_some_and(|n| n.parent.is_none()));
    assert_eq!(doc.roots().len(), 2);
}

#[test]
fn remove_cascades_to_descendants() {
    let mut doc = FormDoc::new("main_form");
    let mut panel = Node::new("Panel", "panA");
    panel.set_prop("width", Value::Int(10));
    doc.insert(panel);
    let mut child = Node::new("Button", "cmdGo");
    child.parent = Some("panA".to_owned());
    child.set_prop("width", Value::Int(10));
    doc.insert(child);
    let mut grandchild = Node::new("Button", "cmdDeep");
    grandchild.parent = Some("cmdGo".to_owned());
    doc.insert(grandchild);

    assert!(doc.remove("panA"));
    assert!(doc.nodes.is_empty());
    assert!(!doc.remove("panA"));
}
