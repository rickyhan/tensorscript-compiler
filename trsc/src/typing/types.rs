use codespan::ByteSpan;
use std::fmt::{Debug, Error, Formatter};
/// Types for typed AST
use std::hash::{Hash, Hasher};
use typing::type_env::TypeId;
use std::collections::BTreeMap;
use typing::type_env::ModName;

#[derive(Clone, Eq, PartialOrd, Ord)]
pub enum Type {
    // literals
    Unit(ByteSpan),
    INT(ByteSpan),
    FLOAT(ByteSpan),
    BOOL(ByteSpan),
    UnresolvedModuleFun(&'static str, &'static str, &'static str, ByteSpan),
    // type variables that need to be resolved
    VAR(TypeId, ByteSpan),
    DIM(TypeId, ByteSpan),
    Tuple(Vec<Type>, ByteSpan),

    // recursive types
    Module(String, Option<Box<Type>>, ByteSpan),
    FnArgs(Vec<Type>, ByteSpan),
    FnArg(Option<String>, Box<Type>, ByteSpan),
    ResolvedDim(i64, ByteSpan),
    FUN(String, String, Box<Type>, Box<Type>, ByteSpan),
    TSR(Vec<Type>, ByteSpan),
}

impl PartialEq for Type {
    fn eq(&self, other: &Type) -> bool {
        use self::Type::*;
        match (self, other) {
            (Unit(_), Unit(_)) => true,
            (INT(_), INT(_)) => true,
            (FLOAT(_), FLOAT(_)) => true,
            (BOOL(_), BOOL(_)) => true,
            // // UnresolvedModuleFun(_,_,_) => false,
            (VAR(a, _), VAR(b, _)) => a == b,
            (DIM(b, _), DIM(a, _)) => a == b,
            (Module(a1, b1, _), Module(a2, b2, _)) => (a1 == a2) && (b1 == b2),
            (FnArgs(ta, _), FnArgs(tb, _)) => ta == tb,
            (Tuple(ta, _), Tuple(tb, _)) => ta == tb,
            (FnArg(n1, t1, _), FnArg(n2, t2, _)) => (n1 == n2) && (t1 == t2),
            (ResolvedDim(a, _), ResolvedDim(b, _)) => a == b,
            (FUN(m1, n1, p1, r1, _), FUN(m2, n2, p2, r2, _)) =>
                (p1 == p2) && (r1 == r2) && (m1 == m2) && (n1 == n2),
            (TSR(ts1, _), TSR(ts2, _)) => ts1 == ts2,
            (UnresolvedModuleFun(a1, b1, c1, _), UnresolvedModuleFun(a2, b2, c2, _)) =>
                (a1 == a2) && (b1 == b2) && (c1 == c2),
            (VAR(..), _) => false,
            (_, VAR(..)) => false,
            (ResolvedDim(..), DIM(..)) => false,
            (DIM(..), ResolvedDim(..)) => false,
            _ => {
                println!("Undefined comparison:");
                println!("(1) {:?}", self);
                println!("(2) {:?}", other);
                false
            }
        }
    }
}

impl Hash for Type {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use self::Type::*;
        match self {
            Unit(_) => ().hash(state),
            INT(_) => 0.hash(state),
            FLOAT(_) => 1.hash(state),
            BOOL(_) => 2.hash(state),
            // UnresolvedModuleFun(_,_,_) => false,
            VAR(a, _) => {
                3.hash(state);
                a.hash(state)
            }
            DIM(b, _) => {
                4.hash(state);
                b.hash(state)
            }

            Module(a, b, _) => {
                5.hash(state);
                a.hash(state);
                b.hash(state);
            }
            FnArgs(ts, _) => {
                6.hash(state);
                ts.hash(state)
            }
            FnArg(n, t, _) => {
                7.hash(state);
                n.hash(state);
                t.hash(state);
            }
            ResolvedDim(a, _) => {
                8.hash(state);
                a.hash(state)
            }
            FUN(m,n,p, r, _) => {
                9.hash(state);
                m.hash(state);
                n.hash(state);
                p.hash(state);
                r.hash(state);
            }
            TSR(ts, _) => {
                10.hash(state);
                ts.hash(state);
            }
            UnresolvedModuleFun(a, b, c, _) => {
                11.hash(state);
                a.hash(state);
                b.hash(state);
                c.hash(state);
            }
            // MismatchedDim(_,_) => true,
            _ => {
                panic!("{:?}", self);
            }
        }
    }
}

impl Type {

    pub fn span(&self) -> ByteSpan {
        use self::Type::*;
        match self {
            // literals
            Unit(s) => *s,
            INT(s) => *s,
            FLOAT(s) => *s,
            BOOL(s) => *s,
            UnresolvedModuleFun(_, _, _, s) => *s,
            // type variables that need to be resolved
            VAR(_, s) => *s,
            DIM(_, s) => *s,
            Tuple(_, s) => *s,

            // recursive types
            Module(_, _, s) => *s,
            FnArgs(_, s) => *s,
            FnArg(_, _, s) => *s,
            ResolvedDim(_, s) => *s,
            FUN(_, _, _, _, s) => *s,
            TSR(_, s) => *s,
        }
    }

    pub fn as_vec(&self) -> Option<Vec<Type>> {
        use self::Type::TSR;
        match self {
            TSR(ts, _) => Some(ts.to_owned()),
            _ => None,
        }
    }

    pub fn as_args_map(&self) -> Option<BTreeMap<String, Type>> {
        use self::Type::{FnArg, FnArgs};
        match self {
            FnArgs(vs, _) => {
                Some(
                    vs.iter()
                    .filter_map(|ty|
                        if let FnArg(ref name,box ref ty, _) = ty {
                            if name.is_some() {
                                Some((name.clone().unwrap(), ty.clone()))
                            } else {
                                None
                            }
                        } else { None }
                    )
                    .collect()
                )
            }
            _ => None
        }
    }

    // pub fn last_dim(&self) -> Option<Type> {
    //     match self {
    //         Type::TSR(vs, _) => {
    //             Some(vs[vs.len()-1].clone())
    //         }
    //         _ => None
    //     }
    // }

    /// returns the first argument type of a function argument
    pub fn first_arg_ty(&self) -> Option<Type> {
        match self {
            Type::FnArgs(vs, _) => {
                if let Type::FnArg(_,box ref ty, _) = vs[0] {
                    Some(ty.clone())
                } else { None }
            }
            Type::FUN(_,_,arg,_,_) => arg.first_arg_ty(),
            _ => None
        }
    }

    /// modifies the span parameter in type to the most relevant
    pub fn with_span(&self, sp: &ByteSpan) -> Type {
        use self::Type::*;
        match self {
            Unit(_) => Unit(*sp),
            VAR(ref a, _) => VAR(*a, *sp),
            DIM(ref a, _) => DIM(*a, *sp),
            INT(_) => INT(*sp),
            FLOAT(_) => FLOAT(*sp),
            BOOL(_) => BOOL(*sp),
            UnresolvedModuleFun(ref a, ref b, ref c, _) => UnresolvedModuleFun(a, b, c, *sp),
            FnArgs(ref args, _) => FnArgs(args.clone(), *sp),
            FnArg(ref name, ref ty, _) => FnArg(name.clone(), ty.clone(), *sp),
            ResolvedDim(ref d, _) => ResolvedDim(*d, *sp),
            Module(ref s, ref ty, _) => Module(s.clone(), ty.clone(), *sp),
            FUN(ref m,ref n,ref p, ref r, _) => FUN(m.clone(),n.clone(),p.clone(), r.clone(), *sp),
            TSR(ref dims, _) => TSR(dims.clone(), *sp),
            Tuple(ref vs, _) => Tuple(vs.clone(), *sp),
        }
    }

    pub fn as_mod_name(&self) -> ModName {
        match self {
            Type::Module(s,..) => ModName::Named(s.to_owned()),
            _ => unimplemented!(),
        }
    }

    pub fn as_string(&self) -> String {
        use self::Type::*;
        match self {
            Module(ref n, _, _) => n.to_owned(),
            TSR(tys, _) => tys.iter().map(|t| t.as_string()).collect::<Vec<_>>().join(", "),
            DIM(_, _) => "-1".to_owned(),
            ResolvedDim(i, _) => format!("{}", i),
            _ => panic!("{:?}", self),
        }
    }

    pub fn as_num(&self) -> Option<i64> {
        use self::Type::*;
        match self {
            ResolvedDim(ref i, _) => Some(*i),
            _ => None,
        }
    }

    pub fn as_rank(&self) -> usize {
        use self::Type::*;
        match self {
            TSR(ref i, _) => i.len(),
            _ => unimplemented!(),
        }
    }

    pub fn is_resolved(&self) -> bool {
        use self::Type::*;
        match self {
            Unit(..) => true,
            INT(..) => true,
            FLOAT(..) => true,
            BOOL(..) => true,
            UnresolvedModuleFun(..) => false,

            VAR(..) => false,
            DIM(..) => false,

            Module(_, Some(i), _) => i.is_resolved(),
            Module(_, None, _) => false,
            FnArgs(ts, _) => ts.iter().map(|t| t.is_resolved()).all(|t| t),
            FnArg(_, t, _) => t.is_resolved(),
            ResolvedDim(_, _) => true,
            FUN(_,_, p, r, _) => Type::is_resolved(p) && r.is_resolved(),
            TSR(_ts, _) => true, //ts.iter().map(|t| t.is_resolved()).all(|t|t),
            _ => unimplemented!(),
        }
    }
}

impl Debug for Type {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        use self::Type::*;
        match self {
            Unit(_) => write!(f, "()"),
            INT(_) => write!(f, "int"),
            FLOAT(_) => write!(f, "float"),
            BOOL(_) => write!(f, "bool"),
            UnresolvedModuleFun(ref a, ref b, ref c, _) => {
                write!(f, "UNRESOLVED({}::{}::{})", a, b, c)
            }
            Tuple(ref tys, _) => write!(f, "({:?})", tys),
            VAR(ref t_id, _) => write!(f, "'{:?}", t_id),
            DIM(ref t_id, _) => write!(f, "!{:?}", t_id),
            FnArgs(ref args, _) => write!(f, "FnArgs({:?})", args),
            FnArg(ref name, ref ty, _) => write!(f, "ARG({:?}={:?})", name, ty),
            ResolvedDim(ref d, _) => write!(f, "<{}>", d),
            Module(ref s, ref ty, _) => write!(f, "MODULE({}, {:?})", s, ty),
            FUN(ref module, ref name,ref p, ref r, _) => write!(f, "{}::{}({:?} -> {:?})", module,name,p, r),
            TSR(ref dims, _) => {
                if !dims.is_empty() {
                    write!(f, "[")?;
                    for i in dims[0..dims.len() - 1].iter() {
                        write!(f, "{:?}, ", i)?;
                    }
                    write!(f, "{:?}]", dims[dims.len() - 1])
                } else {
                    write!(f, "[]")
                }
            }
        }
    }
}

macro_rules! args {
    ( $( $x:expr ),* ) => {
        {
            Type::FnArgs(vec![$($x),*], CSpan::fresh_span())
        }
    };
}

macro_rules! arg {
    ($name:expr, $ty:expr) => {
        Type::FnArg(Some($name.to_owned()), box $ty, CSpan::fresh_span())
    };
}

macro_rules! fun {
    ($m:expr, $n: expr, $e1:expr, $e2:expr) => {
        Type::FUN($m.to_owned(),$n.to_owned(), box $e1, box $e2, CSpan::fresh_span())
    };
}

macro_rules! float {
    () => {
        Type::FLOAT(CSpan::fresh_span())
    };
}

macro_rules! tsr {
    ($tsr:expr) => {
        Type::TSR($tsr, CSpan::fresh_span())
    };
}

macro_rules! unit {
    () => {
        Type::Unit(CSpan::fresh_span())
    };
}

macro_rules! int {
    () => {
        Type::INT(CSpan::fresh_span())
    };
}

macro_rules! tuple {
    (int 2) => {
        Type::Tuple(vec![int!(), int!()], CSpan::fresh_span())
    };
}

macro_rules! module {
    ($e1:expr) => {
        Type::Module($e1.to_owned(), None, CSpan::fresh_span())
    };
}


#[cfg(test)]
mod tests {
    use super::*;
    use codespan::{Span, ByteIndex};
    #[test]
    fn should_not_take_span_into_hash() {
        let h = hashset!(
            Type::VAR(1, Span::new(ByteIndex(1), ByteIndex(1))),
            Type::VAR(1, Span::new(ByteIndex(2), ByteIndex(2))),

            Type::VAR(2, Span::new(ByteIndex(1), ByteIndex(1))),
            Type::VAR(2, Span::new(ByteIndex(2), ByteIndex(2))),
        );
        assert_eq!(h.len(), 2);
    }
}
