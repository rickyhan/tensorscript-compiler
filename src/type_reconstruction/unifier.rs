use std::str::FromStr;
use typed_ast::{Type, TypeEnv};
use typed_ast::type_env::TypeId;
use span::CSpan;

use codespan::CodeMap;
use codespan_reporting::termcolor::StandardStream;
use codespan_reporting::{emit, ColorArg, Diagnostic, Label, Severity};

use type_reconstruction::constraint::{Constraints, Equals};
use type_reconstruction::subst::Substitution;

pub enum TypeError {
    DimensionMismatch(Type, Type),
}

pub struct Unifier {
    pub errs: Vec<TypeError>,
}

impl Unifier {

    pub fn new() -> Unifier {
        Unifier {
            errs: Vec::new(),
        }
    }

    pub fn unify(&mut self, constraints: Constraints, tenv: &mut TypeEnv) -> Substitution {
        if constraints.is_empty() {
            Substitution::empty()
        } else {
            let mut it = constraints.0.into_iter();
            let mut subst = self.unify_one(it.next().unwrap(), tenv);
            let subst_tail = subst.apply(&Constraints(it.collect()));
            let subst_tail: Substitution = self.unify(subst_tail, tenv);
            subst.compose(subst_tail)
        }
    }

    fn unify_one(&mut self, cs: Equals, tenv: &mut TypeEnv) -> Substitution {
        use self::Type::*;
        // println!("{:?}", cs);
        match cs {
            Equals(Unit(_), Unit(_)) => Substitution::empty(),
            Equals(INT(_), INT(_)) => Substitution::empty(),
            Equals(FLOAT(_), FLOAT(_)) => Substitution::empty(),
            Equals(BOOL(_), BOOL(_)) => Substitution::empty(),

            Equals(INT(_), ResolvedDim(_, _)) => Substitution::empty(),
            Equals(ResolvedDim(_, _), INT(_)) => Substitution::empty(),

            Equals(a @ ResolvedDim(_, _), b @ ResolvedDim(_, _)) => {
                if a.as_num() == b.as_num() {
                    Substitution::empty()
                } else {
                    self.errs.push(TypeError::DimensionMismatch(a.clone(),b.clone()));
                    Substitution::empty()
                    // match (a, b) {
                    //     (ResolvedDim(v1, s1), ResolvedDim(v2, s2)) => {
                    //         // panic!("Dimension mismatch! {:?} != {:?} ({}/{})", v1, v2, s1, s2);
                    //     }
                    //     _ => unimplemented!(),
                    // }
                }
            }

            Equals(VAR(tvar, _), ty) => self.unify_var(tvar, ty),
            Equals(ty, VAR(tvar, _)) => self.unify_var(tvar, ty),

            Equals(DIM(tvar, _), ty) => self.unify_var(tvar, ty),
            Equals(ty, DIM(tvar, _)) => self.unify_var(tvar, ty),

            Equals(FnArgs(v1, _), FnArgs(v2, _)) => self.unify(
                Constraints(v1.into_iter().zip(v2).map(|(i, j)| Equals(i, j)).collect()),
                tenv,
            ),

            Equals(FnArg(Some(a), ty1, _), FnArg(Some(b), ty2, _)) => {
                if a == b {
                    self.unify(
                        Constraints(hashset!{
                            Equals(*ty1, *ty2),
                        }),
                        tenv,
                    )
                } else {
                    panic!("supplied parameter is incorrect! {} != {}", a, b);
                }
            }

            Equals(FUN(p1, r1, _), FUN(p2, r2, _)) => self.unify(
                Constraints(hashset!{
                    Equals(*p1, *p2),
                    Equals(*r1, *r2),
                }),
                tenv,
            ),
            Equals(TSR(dims1, _), TSR(dims2, _)) => self.unify(
                Constraints({
                    dims1
                        .into_iter()
                        .zip(dims2)
                        .map(|(i, j)| Equals(i, j))
                        .collect()
                }),
                tenv,
            ),

            Equals(Module(n1, Some(box ty1), _), Module(n2, Some(box ty2), _)) => self.unify(
                Constraints(hashset!{
                    if n1 == n2 {
                        Equals(ty1, ty2)
                    } else {
                        panic!();
                    }
                }),
                tenv,
            ),

            Equals(UnresolvedModuleFun(_, _, _, _), _) => Substitution::empty(),

            _ => {
                panic!("{:#?}", cs);
            }
        }
    }

    fn unify_var(&mut self, tvar: TypeId, ty: Type) -> Substitution {
        use self::Type::*;

        let span = CSpan::fresh_span();
        match ty.clone() {
            VAR(tvar2, _) => {
                if tvar == tvar2 {
                    Substitution::empty()
                } else {
                    Substitution(hashmap!{ VAR(tvar, span) => ty })
                }
            }
            DIM(tvar2, _) => {
                if tvar == tvar2 {
                    Substitution::empty()
                } else {
                    Substitution(hashmap!{ VAR(tvar, span) => ty })
                }
            }
            _ => if occurs(tvar, &ty) {
                panic!("circular type")
            } else {
                Substitution(hashmap!{ VAR(tvar, span) => ty })
            },
        }
    }

    pub fn print_errs(&self, code_map: &CodeMap) {

        for e in self.errs.iter() {
            match e {
                TypeError::DimensionMismatch(Type::ResolvedDim(v1, s1), Type::ResolvedDim(v2,s2)) => {
                    let warning = Diagnostic::new(
                        Severity::Error,
                        format!("Demension mismatch: {} != {}", v1, v2),
                    )
                    .with_label(Label::new_primary(s1.clone()))
                    .with_label(Label::new_secondary(s2.clone()));

                    let diagnostics = [warning];
                    let writer = StandardStream::stderr(ColorArg::from_str("auto").unwrap().into());
                    for diagnostic in &diagnostics {
                        emit(&mut writer.lock(), &code_map, &diagnostic).unwrap();
                        println!();
                    }
                }
                _ => unimplemented!()
            }
        }


    }
}

fn occurs(tvar: TypeId, ty: &Type) -> bool {
    use self::Type::*;
    match ty {
        &FUN(ref p, ref r, _) => occurs(tvar, &p) | occurs(tvar, &r),
        &VAR(ref tvar2, _) => tvar == *tvar2,
        _ => false,
    }
}
