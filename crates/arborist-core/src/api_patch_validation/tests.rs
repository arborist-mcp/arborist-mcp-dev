use super::sarif_artifact_uri;

#[test]
fn sarif_artifact_uris_normalize_windows_paths_and_escape_components() {
    assert_eq!(
        sarif_artifact_uri("E:\\workspace\\a b\\naive-\u{00E9}.c"),
        "file:///E:/workspace/a%20b/naive-%C3%A9.c"
    );
    assert_eq!(sarif_artifact_uri("/tmp/a b.c"), "file:///tmp/a%20b.c");
    assert_eq!(
        sarif_artifact_uri(r"\\server\share\a b.c"),
        "file://server/share/a%20b.c"
    );
}
