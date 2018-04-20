/// Hindley-Milner type inference for type reconstruction
///
/// This consists of three substeps:
///
/// 1. Collect constraints. (handled in constraint.rs)
///     In this step, traverse typed ast and collect types of adjacent nodes that should
///     be equivalent. This generates a Constraint struct which is just a thin wrapper
///     around a hashset of (Type, Type) tuple.
///
/// 2. Unify constraints by generating substitutions.
///     This is a variant to Algorithm W in H-M type inference. Bascially, unify_one
///     function tries to replace 1 type var with a concrete type. The parent function, unify,
///     then uses that substitution on the rest of the constraints, thus eliminating the type
///     variable from the constraint set. The process is iterated until one of these conditions are met:
///     a) all type variable are exhausted. b) equivalence that can never happen. c) circular
///     type dependence (handled by occurs check).
///
/// 3. Generate Substitutions
///     Now after the unification is complete, the function returns a list of substitutions that
///     should remove all type variables from the typed AST.
///
use std::collections::HashMap;
use type_reconstruction::constraint::{Constraints, Equals};
use typed_ast::type_env::TypeEnv;
use typed_ast::type_env::TypeId;
use typed_ast::Type;

use codespan_reporting::termcolor::StandardStream;
use codespan_reporting::{emit, ColorArg, Diagnostic, Label, Severity};

use span::CSpan;

#[derive(Debug)]
pub struct Substitution(pub HashMap<Type, Type>);

impl Substitution {

    /// apply substitution to a set of constraints
    pub fn apply(&mut self, cs: &Constraints) -> Constraints {
        Constraints(
            cs.0
                .iter()
                .map(|Equals(a, b)| Equals(self.apply_ty(a), self.apply_ty(b)))
                .collect(),
        )
    }

    pub fn apply_ty(&mut self, ty: &Type) -> Type {
        self.0.iter().fold(ty.clone(), |result, solution| {
            let (ty, solution_type) = solution;
            if let Type::VAR(ref tvar, ref span) = ty {
                substitute_tvar(result, tvar, &solution_type.with_span(span))
            } else {
                panic!();
                // substitute_ty(result, ty, solution_type)
            }
        })
    }

    pub fn compose(&mut self, mut other: Substitution) -> Substitution {
        let mut self_substituded: HashMap<Type, Type> = self.0
            .clone()
            .into_iter()
            .map(|(k, s)| (k, other.apply_ty(&s)))
            .collect();
        self_substituded.extend(other.0);
        Substitution(self_substituded)
    }

    pub fn empty() -> Substitution {
        Substitution(HashMap::new())
    }
}


/// replace tvar with replacement in ty
fn substitute_tvar(ty: Type, tvar: &TypeId, replacement: &Type) -> Type {
    use self::Type::*;
    // println!("\nTVAR:::\n{:?}, \n'{:?}, \n{:?}\n", ty, tvar, replacement);
    match ty {
        UnresolvedModuleFun(_, _, _, _) => ty,
        Unit(_) => ty,
        INT(_) => ty,
        BOOL(_) => ty,
        FLOAT(_) => ty,
        ResolvedDim(_, _) => ty,
        VAR(tvar2, span) => {
            if tvar.clone() == tvar2 {
                replacement.with_span(&span)
            } else {
                ty
            }
        }
        DIM(tvar2, span) => {
            if tvar.clone() == tvar2 {
                replacement.with_span(&span)
            } else {
                ty
            }
        }
        FnArgs(args, span) => FnArgs(
            args.into_iter()
                .map(|ty| match ty {
                    FnArg(name, a, s) => FnArg(name, box substitute_tvar(*a, tvar, replacement), s),
                    _ => panic!(ty),
                })
                .collect(),
            span,
        ),
        FUN(p, r, s) => FUN(
            box substitute_tvar(*p, tvar, &replacement),
            box substitute_tvar(*r, tvar, &replacement),
            s,
        ),
        TSR(_, _) => ty,

        Module(n, Some(box ty), s) => {
            Module(n, Some(box substitute_tvar(ty, tvar, replacement)), s)
        }

        Module(_, None, _) => ty,
        FnArg(name, box ty, s) => FnArg(name, box substitute_tvar(ty, tvar, replacement), s),
        _ => {
            panic!("{:?}", ty);
        }
    }
}
