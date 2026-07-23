use super::super::cpp_types::CppThisMemberReceiver;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum CppMemberAccess {
    Object,
    Pointer,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum CppStandardUnwrap {
    SmartPointer,
    WeakPointer,
    ReferenceWrapper,
    Optional,
    Expected,
}

#[derive(Clone)]
pub(in super::super) struct CppLocalBinding {
    pub(in super::super) name: String,
    pub(in super::super) type_name: String,
    pub(in super::super) expected_error_type: Option<String>,
    pub(in super::super) expected_error_receiver: Option<CppThisMemberReceiver>,
    pub(in super::super) receiver: CppThisMemberReceiver,
    pub(in super::super) access: CppMemberAccess,
    pub(in super::super) standard_unwrap: Option<CppStandardUnwrap>,
    pub(in super::super) declaration_start: usize,
    pub(in super::super) scope_range: (usize, usize),
}

pub(in super::super) type CppBindingType = (
    String,
    Option<String>,
    Option<CppThisMemberReceiver>,
    CppThisMemberReceiver,
    CppMemberAccess,
    Option<CppStandardUnwrap>,
);
