use super::*;

#[test]
fn refreshes_javascript_typescript_static_module_dependents() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let bridge = dir.join("bridge.ts");
    let caller = dir.join("caller.ts");
    let legacy = dir.join("legacy.cjs");
    let dynamic = dir.join("dynamic.ts");
    let unrelated = dir.join("unrelated.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(&bridge, "export { helper } from \"./helper\";\n").unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./bridge\";\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();
    fs::write(
        &legacy,
        "const helperModule = require(\"./helper\");\nexport function legacy(value) { return helperModule.helper(value); }\n",
    )
    .unwrap();
    fs::write(
        &dynamic,
        "const moduleName = \"./helper\";\nexport async function dynamic() { return import(moduleName); }\n",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "export function unrelated(): number { return 0; }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 2; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 6);
    assert_eq!(stats.rebuilt_files, 4);
    assert_eq!(stats.reused_files, 2);
}
