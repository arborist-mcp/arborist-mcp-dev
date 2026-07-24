use crate::model::SymbolSummary;

#[derive(Debug, Clone)]
pub(in super::super) struct PythonAccessibleSymbol {
    pub(in super::super) name: String,
    pub(in super::super) summary: SymbolSummary,
    pub(in super::super) rank: usize,
    pub(in super::super) visibility: PythonSymbolVisibility,
}

#[derive(Debug, Clone)]
pub(in super::super) enum PythonSymbolVisibility {
    Always,
    ClassBodyLocal {
        class_range: (usize, usize),
    },
    NamedExpression {
        expression_range: (usize, usize),
        comprehension_range: Option<(usize, usize)>,
        comprehension_part_index: Option<usize>,
    },
    ComprehensionTarget {
        comprehension_range: (usize, usize),
        clause_index: usize,
    },
    ExceptTarget {
        except_clause_range: (usize, usize),
    },
    MatchCapture {
        case_clause_range: (usize, usize),
        match_statement_end: usize,
    },
}
