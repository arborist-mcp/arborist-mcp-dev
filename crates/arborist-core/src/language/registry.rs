use std::collections::BTreeMap;
use std::ops::{BitOr, BitOrAssign};
use std::sync::OnceLock;

use tree_sitter::Language;

use super::{C_LANGUAGE_EXTENSIONS, CPP_LANGUAGE_EXTENSIONS};
use crate::model::LanguageId;

const PYTHON_EXTENSIONS: &[&str] = &["py", "pyi"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageCapabilities(u32);

impl LanguageCapabilities {
    pub const TREE_QUERY: Self = Self(1 << 0);
    pub const SEMANTIC_SKELETON: Self = Self(1 << 1);
    pub const SYMBOL_INDEX: Self = Self(1 << 2);
    pub const FILE_DEPENDENCIES: Self = Self(1 << 3);
    pub const REFERENCE_TRACE: Self = Self(1 << 4);
    pub const PATCH_TARGETING: Self = Self(1 << 5);
    pub const PATCH_VALIDATION: Self = Self(1 << 6);

    pub const FULL_CURRENT_SUPPORT: Self = Self(
        Self::TREE_QUERY.0
            | Self::SEMANTIC_SKELETON.0
            | Self::SYMBOL_INDEX.0
            | Self::FILE_DEPENDENCIES.0
            | Self::REFERENCE_TRACE.0
            | Self::PATCH_TARGETING.0
            | Self::PATCH_VALIDATION.0,
    );

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }
}

impl BitOr for LanguageCapabilities {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for LanguageCapabilities {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug)]
pub struct LanguageDescriptor {
    pub id: LanguageId,
    pub display_name: &'static str,
    pub extensions: &'static [&'static str],
    pub capabilities: LanguageCapabilities,
    pub analysis_revision: &'static str,
    grammar: fn() -> Language,
}

impl LanguageDescriptor {
    pub fn tree_sitter_language(&self) -> Language {
        (self.grammar)()
    }
}

#[derive(Debug)]
pub struct LanguageRegistry {
    descriptors: BTreeMap<LanguageId, LanguageDescriptor>,
    extensions: BTreeMap<&'static str, LanguageId>,
}

impl LanguageRegistry {
    fn builtin() -> Self {
        Self::new([
            LanguageDescriptor {
                id: LanguageId::Python,
                display_name: "Python",
                extensions: PYTHON_EXTENSIONS,
                capabilities: LanguageCapabilities::FULL_CURRENT_SUPPORT,
                analysis_revision: "python-v1",
                grammar: python_grammar,
            },
            LanguageDescriptor {
                id: LanguageId::C,
                display_name: "C",
                extensions: C_LANGUAGE_EXTENSIONS,
                capabilities: LanguageCapabilities::FULL_CURRENT_SUPPORT,
                analysis_revision: "c-v1",
                grammar: c_grammar,
            },
            LanguageDescriptor {
                id: LanguageId::Cpp,
                display_name: "C++",
                extensions: CPP_LANGUAGE_EXTENSIONS,
                capabilities: LanguageCapabilities::FULL_CURRENT_SUPPORT,
                analysis_revision: "cpp-v1",
                grammar: cpp_grammar,
            },
        ])
    }

    fn new(descriptors: impl IntoIterator<Item = LanguageDescriptor>) -> Self {
        let mut descriptors_by_id = BTreeMap::new();
        let mut language_by_extension = BTreeMap::new();

        for descriptor in descriptors {
            let language_id = descriptor.id;
            let extensions = descriptor.extensions;
            assert!(
                descriptors_by_id.insert(language_id, descriptor).is_none(),
                "duplicate builtin language descriptor for {language_id:?}",
            );
            for extension in extensions {
                assert!(
                    language_by_extension
                        .insert(*extension, language_id)
                        .is_none(),
                    "duplicate builtin language extension {extension}",
                );
            }
        }

        Self {
            descriptors: descriptors_by_id,
            extensions: language_by_extension,
        }
    }

    pub fn descriptor(&self, language_id: LanguageId) -> Option<&LanguageDescriptor> {
        self.descriptors.get(&language_id)
    }

    pub fn language_for_extension(&self, extension: &str) -> Option<LanguageId> {
        let extension = extension.to_ascii_lowercase();
        self.extensions.get(extension.as_str()).copied()
    }

    pub fn supported_language_names(&self) -> Vec<&'static str> {
        self.descriptors
            .values()
            .map(|descriptor| match descriptor.id {
                LanguageId::Python => "python",
                LanguageId::C => "c",
                LanguageId::Cpp => "cpp",
            })
            .collect()
    }
}

pub fn builtin_language_registry() -> &'static LanguageRegistry {
    static REGISTRY: OnceLock<LanguageRegistry> = OnceLock::new();
    REGISTRY.get_or_init(LanguageRegistry::builtin)
}

fn python_grammar() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

fn c_grammar() -> Language {
    tree_sitter_c::LANGUAGE.into()
}

fn cpp_grammar() -> Language {
    tree_sitter_cpp::LANGUAGE.into()
}
