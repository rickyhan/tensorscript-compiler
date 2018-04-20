use core::Core;
/// Type Environment holds the state during type reconstruction
/// which is really just a few tree traversals.
///
/// It handles, in broad strokes, 3 things:
/// 1. Type Aliasing during the first pass (annotate)
/// 2. pushing and popping scopes (during `annotate` and `collect`)
/// 3. module type and method type reconstruction
use parser::term::{NodeAssign, TensorTy, Term};
use std::collections::{HashMap, VecDeque, HashSet};
use std::fmt::{Debug, Error, Formatter};
use typed_ast::Type;
use typed_ast::typed_term::TyFnAppArg;
use codespan::{Span, ByteIndex};

pub type TypeId = usize;

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum ModName {
    Global,
    Named(String),
}

impl ModName {
    pub fn as_str(&self) -> &str {
        use self::ModName::*;
        match self {
            &Global => unimplemented!(),
            &Named(ref s) => s,
        }
    }
}

impl Debug for ModName {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        use self::ModName::*;
        match self {
            &Named(ref s) => write!(f, "MOD({})", s),
            &Global => write!(f, "MOD(Global)"),
        }
    }
}

/// Represents a single level of scope
#[derive(Debug)]
pub struct Scope {
    /// type information of aliases
    types: HashMap<Alias, Type>,
}

impl Scope {
    pub fn new() -> Scope {
        Scope {
            types: HashMap::new(),
        }
    }
}

type InitMap = HashMap<String, Vec<TyFnAppArg>>;

#[derive(Debug)]
pub struct TypeEnv {
    counter: TypeId,
    current_mod: ModName,
    modules: HashMap<ModName, (VecDeque<Scope>, VecDeque<Scope>, InitMap)>,
    to_verify: HashSet<Type>,
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Alias {
    Variable(String),
    Function(String),
}

impl Debug for Alias {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self {
            Alias::Function(a) => write!(f, "F({})", a),
            Alias::Variable(a) => write!(f, "V({})", a),
        }
    }
}

impl Alias {
    pub fn as_str(&self) -> &str {
        match self {
            Alias::Function(s) => s,
            Alias::Variable(s) => s,
        }
    }
}


impl TypeEnv {
    pub fn new() -> Self {
        Self {
            counter: 0,
            current_mod: ModName::Global,
            modules: HashMap::new(),
            to_verify: HashSet::new(),
        }
    }

    /// create new dimension type variable
    pub fn fresh_dim(&mut self, span: &Span<ByteIndex>) -> Type {
        self.counter += 1;
        Type::DIM(self.counter, span.clone())
    }

    /// create new type variable
    pub fn fresh_var(&mut self, span: &Span<ByteIndex>) -> Type {
        self.counter += 1;
        // println!("new_var: {}", self.counter);
        Type::VAR(self.counter, span.clone())
    }

    /// push scope onto stack during tree traversal
    pub fn push_scope(&mut self, mod_name: &ModName) {
        let stack = self.modules.get_mut(mod_name).unwrap();
        stack.0.push_back(Scope::new());
    }

    /// during constraint collection, push the popped scopes back
    pub fn push_scope_collection(&mut self, mod_name: &ModName) {
        let stack = self.modules.get_mut(mod_name).unwrap();
        let scp = stack.1.pop_front().unwrap();
        stack.0.push_back(scp);
    }

    /// exiting block during tree traversal
    pub fn pop_scope(&mut self, mod_name: &ModName) {
        let stack = self.modules.get_mut(mod_name).unwrap();
        let popped = stack.0.pop_back().unwrap();
        stack.1.push_back(popped);
    }

    pub fn resolve_init(&self, mod_name: &ModName, alias: &str) -> Option<Vec<TyFnAppArg>> {
        let stack = self.modules.get(mod_name).unwrap();
        stack.2.get(alias).cloned()
    }

    /// resolve the type of an identifier
    /// first check current mod name, if it doesn not exist,
    /// then check in the global scope
    pub fn resolve_type(&self, mod_name: &ModName, alias: &Alias) -> Option<Type> {
        self.resolve_type_inner(mod_name, alias)
            .or(self.resolve_type_inner(&ModName::Global, alias))
    }

    /// inside the module or global scope, iterate over block scope and find
    /// the last defn of the alias which may be shadowed
    fn resolve_type_inner(&self, mod_name: &ModName, alias: &Alias) -> Option<Type> {
        let types = self.get_scoped_types(mod_name, alias);
        types.iter().last().cloned()
    }

    /// iterate over scopes and find the alias in each
    fn get_scoped_types(&self, mod_name: &ModName, alias: &Alias) -> Vec<Type> {
        let stack = self.modules.get(mod_name).unwrap();
        stack
            .0
            .iter()
            .rev()
            .map(|sc| sc.types.get(alias))
            .filter(|i| i.is_some())
            .map(|i| i.unwrap())
            .cloned()
            .collect()
    }

    /// add type alias in current scope
    pub fn add_type(&mut self, mod_name: &ModName, alias: &Alias, ty: Type) {
        let stack = self.modules.entry(mod_name.clone()).or_insert({
            // if the module does not yet exist, add with an empty scope
            let mut q = VecDeque::new();
            q.push_back(Scope::new());
            (q, VecDeque::new(), HashMap::new())
        });

        let top = stack.0.len() - 1;
        let scope = stack.0.get_mut(top).unwrap();
        if scope.types.contains_key(alias) {
            panic!("duplicate item");
        }
        let _ = scope.types.insert(alias.clone(), ty);
    }

    /// add type alias in current scope
    pub unsafe fn add_type_allow_dup(&mut self, mod_name: &ModName, alias: &Alias, ty: Type) {
        let stack = self.modules.entry(mod_name.clone()).or_insert({
            // if the module does not yet exist, add with an empty scope
            let mut q = VecDeque::new();
            q.push_back(Scope::new());
            (q, VecDeque::new(), HashMap::new())
        });

        let top = stack.0.len() - 1;
        let scope = stack.0.get_mut(top).unwrap();
        // if scope.types.contains_key(alias) {
        //     panic!("duplicate item");
        // }
        let _ = scope.types.insert(alias.clone(), ty);
    }

    /// add stateful initialization in current scope
    pub fn add_init(&mut self, mod_name: &ModName, alias: &str, ty: Vec<TyFnAppArg>) {
        let stack = self.modules.get_mut(&mod_name).unwrap();

        if stack.2.contains_key(alias) {
            panic!("duplicate item");
        }
        let _ = stack.2.insert(alias.to_owned(), ty);
    }

    /// tie an alias with a type variable dimension
    pub fn add_dim_alias(&mut self, mod_name: &ModName, alias: &Alias, span: &Span<ByteIndex>) {
        let tyvar = self.fresh_dim(span);
        self.add_type(mod_name, alias, tyvar);
    }

    /// tie an alias with a resolved dimension
    pub fn add_resolved_dim_alias(&mut self, mod_name: &ModName, alias: &Alias, num: i64, span: &Span<ByteIndex>) {
        let tyvar = Type::ResolvedDim(num, span.clone());
        self.add_type(mod_name, alias, tyvar);
    }

    /// tie an alias with a tensor
    pub fn add_tsr_alias(&mut self, mod_name: &ModName, alias: &Alias, tsr: &[String], span: &Span<ByteIndex>) {
        // first insert all the dims
        tsr.iter()
            .map(|t| Alias::Variable(t.to_string()))
            .map(|t| {
                if !self.exists(mod_name, &t) {
                    self.add_dim_alias(mod_name, &t, span);
                }
            })
            .collect::<Vec<()>>();

        // then insert the tensor itself
        let tsr = self.create_tensor(mod_name, tsr, span);
        self.add_type(mod_name, alias, tsr)
    }

    // make a new tensor based on type signature
    pub fn create_tensor(&mut self, mod_name: &ModName, dims: &[String], span: &Span<ByteIndex>) -> Type {
        // each dimension alias in the tensor type signature must exist
        let dims_ty = dims.iter()
            .map(|t| self.resolve_type(mod_name, &Alias::Variable(t.to_string())).unwrap().clone())
            .collect();
        // create the tensor type
        Type::TSR(dims_ty, span.clone())
    }

    /// generate a tensor from untyped ast tensor signature
    pub fn resolve_tensor(&mut self, mod_name: &ModName, t: &TensorTy, span: &Span<ByteIndex>) -> Type {
        match t {
            &TensorTy::Generic(ref dims) => self.create_tensor(mod_name, &dims, span),
            &TensorTy::TyAlias(ref alias) => self.resolve_type(mod_name, &Alias::Variable(alias.to_string())).unwrap(),
        }
    }

    /// check if an alias exists
    pub fn exists(&self, mod_name: &ModName, alias: &Alias) -> bool {
        let types = self.get_scoped_types(mod_name, alias);
        types.len() > 0
    }

    /// create aliases for an untyped AST node assign
    pub fn import_node_assign(&mut self, mod_name: &ModName, a: &NodeAssign) {
        match a {
            &NodeAssign::TyAlias {
                ident: ref id,
                rhs: TensorTy::Generic(ref tys),
                ref span,
            } => {
                self.add_tsr_alias(mod_name, &Alias::Variable(id.to_string()), tys, span);
            }
            &NodeAssign::ValueAlias {
                ident: ref id,
                rhs: Term::Integer(num, _),
                ref span,
            } => {
                self.add_resolved_dim_alias(mod_name, &Alias::Variable(id.to_string()), num, span);
            }
            _ => unimplemented!(),
        }
    }

    /// get current module name
    pub fn module(&self) -> ModName {
        self.current_mod.clone()
    }

    /// set current module name
    pub fn set_module(&mut self, scp: ModName) {
        self.current_mod = scp;
    }

    /// import module type and associated methods into type environment
    pub fn import_module(&mut self, path_name: &str, mod_name: &str) {
        let methods = Core::import(path_name, mod_name, self);
        for &(ref name, ref ty) in methods.iter() {
            self.add_type(
                &ModName::Named(mod_name.to_owned()),
                &Alias::Function(name.to_string()),
                ty.clone(),
            );
        }
    }

    pub fn resolve_unresolved(&mut self, ty: Type, symbol_name: &str, module: &Type, fn_name: &str, inits: Option<Vec<TyFnAppArg>>) -> Option<Type> {
        let (mod_name, mod_ty) = {
            if let Type::Module(name, opty, span) = module {
                (name, opty.clone().map(|i|*i))
            } else {
                panic!();
            }
        };

        if let Type::UnresolvedModuleFun(p0, p1, p2, span) = ty {
            assert_eq!(fn_name, &format!("self.{}", p2));
            let op = Core::find(p0, p1);
            let ty = op.resolve(self, mod_ty, fn_name, inits);
            ty
        } else {
            unimplemented!();
        }
    }

    pub fn add_unverified(&mut self, v: Type) {
        self.to_verify.insert(v);
    }
}
