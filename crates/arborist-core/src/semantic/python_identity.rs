use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(crate) struct PythonSymbolIdentity<'a> {
    pub(crate) file_path: &'a str,
    pub(crate) semantic_path: &'a str,
    pub(crate) is_overload: bool,
    pub(crate) byte_range: (usize, usize),
}

pub(crate) fn python_symbol_ids(entries: &[PythonSymbolIdentity<'_>]) -> Vec<String> {
    let mut path_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut groups: BTreeMap<(&str, &str), Vec<usize>> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        *path_counts.entry(entry.semantic_path).or_default() += 1;
        groups
            .entry((entry.file_path, entry.semantic_path))
            .or_default()
            .push(index);
    }

    let mut ids = entries
        .iter()
        .map(|entry| entry.semantic_path.to_string())
        .collect::<Vec<_>>();
    for indices in groups.values_mut() {
        indices.sort_by_key(|index| entries[*index].byte_range);
        let first = entries[indices[0]];
        let identity_path = if first.file_path.is_empty() {
            first.semantic_path.to_string()
        } else {
            format!("{}::{}", first.file_path, first.semantic_path)
        };
        if indices.len() == 1 {
            if path_counts[first.semantic_path] > 1 {
                ids[indices[0]] = identity_path;
            }
            continue;
        }

        let has_overloads = indices.iter().any(|index| entries[*index].is_overload);
        let implementation_count = indices
            .iter()
            .filter(|index| !entries[**index].is_overload)
            .count();
        let mut overload_ordinal = 0usize;
        let mut implementation_ordinal = 0usize;

        for index in indices.iter().copied() {
            let entry = entries[index];
            ids[index] = if has_overloads && entry.is_overload {
                overload_ordinal += 1;
                format!("{identity_path}#overload[{overload_ordinal}]")
            } else if has_overloads {
                implementation_ordinal += 1;
                if implementation_count == 1 {
                    format!("{identity_path}#implementation")
                } else {
                    format!("{identity_path}#implementation[{implementation_ordinal}]")
                }
            } else {
                implementation_ordinal += 1;
                format!("{identity_path}#definition[{implementation_ordinal}]")
            };
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::{PythonSymbolIdentity, python_symbol_ids};

    #[test]
    fn qualifies_singletons_when_a_path_repeats_across_files() {
        let entries = [
            PythonSymbolIdentity {
                file_path: "first.py",
                semantic_path: "Store.get",
                is_overload: false,
                byte_range: (10, 20),
            },
            PythonSymbolIdentity {
                file_path: "second.py",
                semantic_path: "Store.get",
                is_overload: false,
                byte_range: (10, 20),
            },
        ];

        assert_eq!(
            python_symbol_ids(&entries),
            ["first.py::Store.get", "second.py::Store.get"]
        );
    }

    #[test]
    fn assigns_distinct_overload_and_implementation_ids() {
        let entries = [
            PythonSymbolIdentity {
                file_path: "sample.py",
                semantic_path: "Store.get",
                is_overload: true,
                byte_range: (10, 20),
            },
            PythonSymbolIdentity {
                file_path: "sample.py",
                semantic_path: "Store.get",
                is_overload: true,
                byte_range: (30, 40),
            },
            PythonSymbolIdentity {
                file_path: "sample.py",
                semantic_path: "Store.get",
                is_overload: false,
                byte_range: (50, 60),
            },
        ];

        assert_eq!(
            python_symbol_ids(&entries),
            [
                "sample.py::Store.get#overload[1]",
                "sample.py::Store.get#overload[2]",
                "sample.py::Store.get#implementation",
            ]
        );
    }
}
