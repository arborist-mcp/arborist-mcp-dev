use super::super::cpp_types::CppThisMemberReceiver;
use crate::symbol_index_model::{
    CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_CONST_LVALUE_THIS_CALL_PREFIX,
    CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_CONST_RVALUE_THIS_CALL_PREFIX, CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_RVALUE_THIS_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_TEMPORARY_MEMBER_CALL_SEPARATOR,
};

pub(super) fn encode_cpp_this_member_call_name(
    name: String,
    receiver: CppThisMemberReceiver,
) -> String {
    match receiver {
        CppThisMemberReceiver::Lvalue => name,
        CppThisMemberReceiver::ConstLvalue => {
            format!("{CPP_CONST_LVALUE_THIS_CALL_PREFIX}{name}")
        }
        CppThisMemberReceiver::Rvalue => format!("{CPP_RVALUE_THIS_CALL_PREFIX}{name}"),
        CppThisMemberReceiver::ConstRvalue => {
            format!("{CPP_CONST_RVALUE_THIS_CALL_PREFIX}{name}")
        }
    }
}

pub(super) fn encode_cpp_temporary_member_call_name(
    type_name: String,
    name: String,
    receiver: CppThisMemberReceiver,
) -> String {
    let prefix = match receiver {
        CppThisMemberReceiver::Lvalue => return name,
        CppThisMemberReceiver::ConstLvalue => CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::Rvalue => CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::ConstRvalue => CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    };
    format!("{prefix}{type_name}{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}{type_name}::{name}")
}

pub(super) fn encode_cpp_local_member_call_name(
    type_name: String,
    name: String,
    receiver: CppThisMemberReceiver,
) -> String {
    let prefix = match receiver {
        CppThisMemberReceiver::Lvalue => CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::ConstLvalue => CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::Rvalue => CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CppThisMemberReceiver::ConstRvalue => CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    };
    format!("{prefix}{type_name}{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}{type_name}::{name}")
}
