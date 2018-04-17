use std::collections::HashMap;
use type_reconstruction::constraint::{Constraints, Equals};
use typed_ast::Type;
use typed_ast::type_env::TypeId;

#[derive(Debug)]
pub struct Substitution(pub HashMap<TypeId, Type>);

impl Substitution {
    pub fn new() -> Substitution {
        Substitution(HashMap::new())
    }

    pub fn apply(&mut self, cs: &Constraints) -> Constraints {
        Constraints(cs.0.iter().map(|eq| self.apply_equals(eq)).collect())
    }

    pub fn apply_equals(&mut self, eq: &Equals) -> Equals {
        let Equals(a, b) = eq;
        Equals(self.apply_ty(a), self.apply_ty(b))
    }

    pub fn apply_ty(&mut self, ty: &Type) -> Type {
        self.0.iter().fold(ty.clone(), |result, solution| {
            let (tvar, solution_type) = solution;
            substitute(result, tvar, solution_type)
        })
    }

    pub fn compose(&mut self, mut other: Substitution) -> Substitution {
        let mut self_substituded: HashMap<TypeId, Type> = self.0
            .clone()
            .into_iter()
            .map(|(k, s)| (k, other.apply_ty(&s)))
            .collect();
        self_substituded.extend(other.0);
        Substitution(self_substituded)
    }

    fn empty() -> Substitution {
        Substitution(HashMap::new())
    }
}

fn occurs(tvar: TypeId, ty: &Type) -> bool {
    use self::Type::*;
    match ty {
        &FUN(ref p, ref r) => occurs(tvar, &p) | occurs(tvar, &r),
        &VAR(ref tvar2) => tvar == *tvar2,
        _ => false,
    }
}

fn substitute(ty: Type, tvar: &TypeId, replacement: &Type) -> Type {
    use self::Type::*;
    match ty {
        Unit => ty,
        INT => ty,
        BOOL => ty,
        FLOAT => ty,
        ResolvedDim(_) => ty,
        VAR(tvar2) => {
            if tvar.clone() == tvar2 {
                replacement.clone()
            } else {
                ty
            }
        }
        DIM(tvar2) => {
            if tvar.clone() == tvar2 {
                replacement.clone()
            } else {
                ty
            }
        }
        FN_ARGS(args) => FN_ARGS(
            args
                .into_iter()
                .map(|ty| match ty {
                    FN_ARG(name, a) => FN_ARG(name, box substitute(*a, tvar, replacement)),
                    _ => unimplemented!(),
                })
                .collect()
        ),
        FUN(p, r) => FUN(
            Box::new(substitute(*p, tvar, &replacement)),
            Box::new(substitute(*r, tvar, &replacement)),
        ),
        TSR(_) => ty,
        _ => {
            panic!("{:?}", ty);
            unimplemented!();
        }
    }
}

pub fn unify(constraints: Constraints) -> Substitution {
    if constraints.0.is_empty() {
        Substitution::new()
    } else {
        let mut it = constraints.0.into_iter();
        let mut subst = unify_one(it.next().unwrap());
        let subst_tail = subst.apply(&Constraints(it.collect()));
        let subst_tail: Substitution = unify(subst_tail);
        subst.compose(subst_tail)
    }
}

fn unify_one(cs: Equals) -> Substitution {
    use self::Type::*;
    match cs {
        Equals(INT, INT) => Substitution::empty(),
        Equals(FLOAT, FLOAT) => Substitution::empty(),
        Equals(BOOL, BOOL) => Substitution::empty(),

        Equals(ResolvedDim(i), ResolvedDim(j)) => if i == j {
            Substitution::empty()
        } else {
            panic!("dimension mismatch")
        },

        Equals(VAR(tvar), ty) => unify_var(tvar, ty),
        Equals(ty, VAR(tvar)) => unify_var(tvar, ty),

        Equals(DIM(tvar), ty) => unify_var(tvar, ty),
        Equals(ty, DIM(tvar)) => unify_var(tvar, ty),

        Equals(FN_ARGS(v1), FN_ARGS(v2)) => unify(Constraints(
            v1.into_iter().zip(v2).map(|(i,j)|Equals(i,j)).collect()
        )),

        Equals(FN_ARG(Some(a), ty1), FN_ARG(Some(b), ty2))  => {
            if a == b {
                unify(Constraints(hashset!{
                    Equals(*ty1, *ty2),
                }))
            } else {
                panic!("supplied parameter is incorrect!");
            }
        }
        Equals(FN_ARG(None, ty1), FN_ARG(Some(_), ty2))  => unify(Constraints(hashset!{
            Equals(*ty1, *ty2),
        })),
        Equals(FN_ARG(Some(_), ty1), FN_ARG(None, ty2))  => unify(Constraints(hashset!{
            Equals(*ty1, *ty2),
        })),
        Equals(FN_ARG(None, ty1), FN_ARG(None, ty2)) => unify(Constraints(hashset!{
            Equals(*ty1, *ty2),
        })),

        Equals(FUN(p1, r1), FUN(p2, r2)) => unify(Constraints(hashset!{
            Equals(*p1, *p2),
            Equals(*r1, *r2),
        })),
        Equals(TSR(dims1), TSR(dims2)) => unify(Constraints({
            dims1
                .into_iter()
                .zip(dims2)
                .map(|(i, j)| Equals(i, j))
                .collect()
        })),
        _ => {
            panic!("{:#?}", cs);
            unimplemented!();
        }
    }
}

fn unify_var(tvar: TypeId, ty: Type) -> Substitution {
    use self::Type::*;
    match ty.clone() {
        VAR(tvar2) => {
            if tvar == tvar2 {
                Substitution::empty()
            } else {
                Substitution(hashmap!{ tvar => ty })
            }
        },
        DIM(tvar2) => {
            if tvar == tvar2 {
                Substitution::empty()
            } else {
                Substitution(hashmap!{ tvar => ty })
            }
        },
        _ => if occurs(tvar, &ty) {
            panic!("circular type")
        } else {
            Substitution(hashmap!{ tvar => ty })
        },
    }
}