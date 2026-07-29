use crate::patching::c_validation::cpp_syntax::strip_cpp_outer_parentheses;
use crate::patching::c_validation::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use crate::patching::c_validation::cpp_wrappers::{
    cpp_standard_expected_target_type, cpp_standard_optional_target_type,
};

pub(in crate::patching::c_validation::references) fn cpp_strip_optional_value_access(
    expression: &str,
) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let receiver = expression
        .strip_suffix(".value()")
        .or_else(|| expression.strip_suffix("->value()"))
        .map(str::trim)?;
    // Reject "*expr.value()" where unary * applies to the value call.
    // "(*expr).value()" keeps parentheses and remains valid.
    if receiver.starts_with('*') {
        return None;
    }
    Some(receiver)
}

pub(in crate::patching::c_validation::references) fn cpp_strip_expected_error_access(
    expression: &str,
) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    // Reject "*expr.error()" where unary * applies to the error access.
    // "(*expr).error()" keeps parentheses and remains valid.
    if expression.starts_with('*') {
        return None;
    }
    expression
        .strip_suffix(".error()")
        .or_else(|| expression.strip_suffix("->error()"))
        .map(str::trim)
}

pub(in crate::patching::c_validation::references) fn cpp_standard_value_member_receiver(
    type_name: &str,
    wrapper_receiver: CppThisMemberReceiver,
    preserves_value_category: bool,
) -> Option<(String, CppThisMemberReceiver)> {
    let target = cpp_standard_optional_target_type(type_name)
        .or_else(|| cpp_standard_expected_target_type(type_name))?;
    let target_receiver = cpp_this_receiver_for_type(target, Some(false))?;
    let const_qualified = matches!(
        wrapper_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    ) || matches!(
        target_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    );
    let rvalue = preserves_value_category
        && matches!(
            wrapper_receiver,
            CppThisMemberReceiver::Rvalue | CppThisMemberReceiver::ConstRvalue
        );
    let receiver = match (const_qualified, rvalue) {
        (false, false) => CppThisMemberReceiver::Lvalue,
        (true, false) => CppThisMemberReceiver::ConstLvalue,
        (false, true) => CppThisMemberReceiver::Rvalue,
        (true, true) => CppThisMemberReceiver::ConstRvalue,
    };
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(in crate::patching::c_validation::references) fn cpp_expected_error_receiver(
    error_type: &str,
    expected_receiver: CppThisMemberReceiver,
) -> Option<CppThisMemberReceiver> {
    let error_receiver = cpp_this_receiver_for_type(error_type, Some(false))?;
    let const_qualified = matches!(
        expected_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    ) || matches!(
        error_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    );
    let rvalue = matches!(
        expected_receiver,
        CppThisMemberReceiver::Rvalue | CppThisMemberReceiver::ConstRvalue
    );
    Some(match (const_qualified, rvalue) {
        (false, false) => CppThisMemberReceiver::Lvalue,
        (true, false) => CppThisMemberReceiver::ConstLvalue,
        (false, true) => CppThisMemberReceiver::Rvalue,
        (true, true) => CppThisMemberReceiver::ConstRvalue,
    })
}
