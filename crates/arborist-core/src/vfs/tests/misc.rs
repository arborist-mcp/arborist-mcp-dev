use super::*;

#[test]
fn virtual_workspace_overrides_skip_symlink_file_escape() {
    let dir = temp_workspace();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("linked.py"), "def leaked():\n    return 1\n").unwrap();

    let linked_path = workspace.join("linked.py");
    if !try_symlink_file(&outside.join("linked.py"), &linked_path) {
        let _ = fs::remove_dir_all(dir);
        return;
    }

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&linked_path, Some("def leaked():\n    return 2\n"))
        .unwrap();

    let overrides = vfs.virtual_overrides_for_workspace(&workspace).unwrap();

    assert!(overrides.is_empty());
}
