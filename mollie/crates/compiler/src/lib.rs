use indexmap::{IndexMap, map::Entry};
use mollie_lexer::{Lexer, Token};
use mollie_parser::{Expression, Parser, Statement, parse_statements_until};
use mollie_shared::{Positioned, Span};
use mollie_vm::{ArrayType, Chunk, ComplexType, PrimitiveType, SmallVec, StackFrame, Trait, TraitFunc, Type, TypeVariant, Value, Vm, any};

pub use self::error::{CompileError, CompileResult, TypeError, TypeResult};

mod error;
mod statement;
mod ty;

#[derive(Debug)]
pub struct Variable {
    pub id: usize,
    pub ty: Type,
    pub value: Option<Value>,
}

type VTable = IndexMap<Option<usize>, IndexMap<String, (Type, Value)>>;

#[derive(Debug)]
pub struct Compiler {
    pub traits: IndexMap<String, Trait>,
    pub types: IndexMap<String, Type>,
    pub impls: IndexMap<TypeVariant, Vec<usize>>,
    pub vtables: IndexMap<TypeVariant, VTable>,
    pub frames: Vec<IndexMap<String, Variable>>,

    pub generics: Vec<Type>,
    pub assign: Option<Positioned<Expression>>,
    pub infer: Option<Type>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self {
            traits: IndexMap::new(),
            types: IndexMap::new(),
            impls: IndexMap::new(),
            vtables: IndexMap::new(),
            frames: vec![IndexMap::new()],
            generics: Vec::new(),
            assign: None,
            infer: None,
        }
    }
}

pub struct TraitBuilder<'a> {
    compiler: &'a mut Compiler,
    name: String,
    functions: Vec<TraitFunc>,
}

impl<'a> TraitBuilder<'a> {
    fn new<T: Into<String>>(compiler: &'a mut Compiler, name: T) -> Self {
        Self {
            compiler,
            name: name.into(),
            functions: Vec::new(),
        }
    }

    #[must_use]
    pub fn static_method<T: Into<String>, I: IntoIterator<Item = TypeVariant>, R: Into<Type>>(mut self, name: T, args: I, returns: R) -> Self {
        self.functions.push(TraitFunc {
            name: name.into(),
            this: false,
            args: args.into_iter().map(Into::into).collect(),
            returns: returns.into(),
        });

        self
    }

    #[must_use]
    pub fn method<T: Into<String>, I: IntoIterator<Item = TypeVariant>, R: Into<Type>>(mut self, name: T, args: I, returns: R) -> Self {
        self.functions.push(TraitFunc {
            name: name.into(),
            this: true,
            args: args.into_iter().map(Into::into).collect(),
            returns: returns.into(),
        });

        self
    }

    pub fn build(self) -> usize {
        let index = self.compiler.traits.len();

        self.compiler.traits.insert(self.name, Trait {
            generics: Vec::new(),
            functions: self.functions,
            declared_at: None,
        });

        index
    }
}

impl Compiler {
    pub fn add_trait<T: Into<String>>(&mut self, name: T) -> TraitBuilder<'_> {
        TraitBuilder::new(self, name)
    }

    pub fn add_type<T: Into<String>>(&mut self, name: T, ty: TypeVariant) {
        self.types.insert(name.into(), ty.into());
    }

    pub fn add_declared_type<T: Into<String>>(&mut self, name: T, ty: Type) {
        self.types.insert(name.into(), ty);
    }

    pub fn remove_type<T: AsRef<str>>(&mut self, name: T) {
        self.types.shift_remove(name.as_ref());
    }

    pub fn push_frame(&mut self) {
        self.frames.push(IndexMap::new());
    }

    pub fn pop_frame(&mut self) {
        self.frames.pop();
    }

    pub const fn current_frame_id(&self) -> usize {
        self.frames.len() - 1
    }

    pub fn current_frame(&self) -> &IndexMap<String, Variable> {
        self.frames.last().unwrap()
    }

    pub fn current_frame_mut(&mut self) -> &mut IndexMap<String, Variable> {
        self.frames.last_mut().unwrap()
    }

    pub fn get_var<T: AsRef<str>>(&self, name: T) -> Option<&Variable> {
        let name = name.as_ref();

        for frame in self.frames.iter().rev() {
            if let Some(var) = frame.get(name) {
                return Some(var);
            }
        }

        None
    }

    pub fn get_var_index<T: AsRef<str>>(&self, name: T) -> Option<(usize, usize)> {
        let name = name.as_ref();

        for (frame_id, frame) in self.frames.iter().enumerate().rev() {
            if let Some(var) = frame.get(name) {
                return Some((self.current_frame_id() - frame_id, var.id));
            }
        }

        None
    }

    pub fn vtable_func<T: Into<String>>(&mut self, ty: TypeVariant, name: T, function_ty: Type, value: Value) {
        let name = name.into();

        match self.vtables.entry(ty) {
            Entry::Occupied(mut entry) => match entry.get_mut().entry(None) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().insert(name, (function_ty, value));
                }
                Entry::Vacant(entry) => {
                    entry.insert(IndexMap::from_iter([(name, (function_ty, value))]));
                }
            },
            Entry::Vacant(entry) => {
                entry.insert(IndexMap::from_iter([(None, IndexMap::from_iter([(name, (function_ty, value))]))]));
            }
        }
    }

    pub fn var<T: Into<String>, V: Into<Type>>(&mut self, name: T, ty: V) -> usize {
        let id = self.current_frame().len();
        let name = name.into();

        self.current_frame_mut().insert(name, Variable {
            id,
            ty: ty.into(),
            value: None,
        });

        id
    }

    pub fn remove_var<T: AsRef<str>>(&mut self, name: T) {
        let name = name.as_ref();

        self.current_frame_mut().shift_remove(name);
    }

    pub fn var_value<T: Into<String>>(&mut self, name: T, ty: TypeVariant, value: Value) {
        let id = self.current_frame().len();
        let name = name.into();

        self.current_frame_mut().insert(name, Variable {
            id,
            ty: ty.into(),
            value: Some(value),
        });
    }

    /// # Errors
    ///
    /// Returns `CompileError` if program parsing or compilation fails.
    pub fn compile_program_text<T: AsRef<str>>(&mut self, text: T) -> CompileResult<Chunk> {
        let mut parser = Parser::new(Lexer::lex(text.as_ref()));

        let program = match parse_statements_until(&mut parser, &Token::EOF) {
            Ok(statements) => statements,
            Err(error) => return Err(CompileError::Parse(error)),
        };

        self.compile_program(program)
    }

    /// # Errors
    ///
    /// Returns `CompileError` if program compilation fails.
    pub fn compile_program(&mut self, (statements, returned): (Vec<Positioned<Statement>>, Option<Positioned<Statement>>)) -> CompileResult<Chunk> {
        let mut chunk = Chunk::default();

        for statement in statements {
            if self.compile(&mut chunk, statement)? {
                chunk.pop();
            }
        }

        if let Some(statement) = returned {
            self.compile(&mut chunk, statement)?;

            chunk.ret();
        }

        chunk.halt();

        Ok(chunk)
    }

    /// # Errors
    ///
    /// Returns `CompileError` if compilation fails. This can happen in two
    /// cases: if the referenced variable is not found or if type inference
    /// fails.
    pub fn compile<O, T: Compile<O>>(&mut self, chunk: &mut Chunk, value: T) -> CompileResult<O> {
        value.compile(chunk, self)
    }

    /// # Errors
    ///
    /// Returns `TypeError` if type inference fails. This can happen if, for
    /// example, the required type is not declared.
    pub fn get_positioned_type<T: GetType>(&mut self, value: &Positioned<T>) -> TypeResult {
        value.get_type(self)
    }

    /// # Errors
    ///
    /// Returns `TypeError` if type inference fails. This can happen if, for
    /// example, the required type is not declared.
    pub fn get_value_type<T: GetType>(&mut self, value: &T, span: Span) -> TypeResult {
        value.get_type(self, span)
    }

    /// # Errors
    ///
    /// Will throw `CompileError` if there's no type with that `name`.
    pub fn try_get_type<T: AsRef<str>>(&self, name: T) -> CompileResult<Type> {
        self.types.get(name.as_ref()).cloned().ok_or_else(|| CompileError::VariableNotFound {
            name: name.as_ref().to_string(),
        })
    }

    /// # Errors
    ///
    /// Will throw `TypeError` if there's no local with that `name`.
    pub fn get_local_type<T: AsRef<str>>(&self, name: T) -> TypeResult<Type> {
        for frame in self.frames.iter().rev() {
            if let Some(v) = frame.get(name.as_ref()) {
                return Ok(v.ty.clone());
            }
        }

        Err(TypeError::NotFound {
            ty: None,
            name: name.as_ref().to_string(),
        })
    }

    /// # Panics
    ///
    /// Will panic if there's no type with that `name`.
    pub fn get_type<T: AsRef<str>>(&self, name: T) -> Type {
        self.types
            .get(name.as_ref())
            .map_or_else(|| panic!("{} not found", name.as_ref()), Clone::clone)
    }

    fn _find_vtable_function_index<T: AsRef<str>>(&self, vtable_index: usize, function_name: T) -> Option<(usize, Option<usize>, usize)> {
        let index = self.vtables[vtable_index]
            .iter()
            .find_map(|(i, vtable)| vtable.get_index_of(function_name.as_ref()).map(|index| (*i, index)));

        index.map(|index| (vtable_index, index.0, index.1))
    }

    pub fn find_vtable_function<T: AsRef<str>>(&self, ty: &TypeVariant, contains: T) -> Option<&(Type, Value)> {
        self.find_vtable_function_index(ty, contains).map(|v| &self.vtables[v.0][&v.1][v.2])
    }

    pub fn find_vtable_function_index<T: AsRef<str>>(&self, ty: &TypeVariant, function_name: T) -> Option<(usize, Option<usize>, usize)> {
        self.vtables.get_index_of(ty).map_or_else(
            || {
                ty.as_array().map_or_else(
                    || {
                        ty.as_component()
                            .map_or_else(
                                || self.vtables.get_index_of(&any()),
                                |_| self.vtables.get_index_of(&TypeVariant::Primitive(PrimitiveType::Component)),
                            )
                            .and_then(|v| self._find_vtable_function_index(v, function_name.as_ref()))
                    },
                    |ty| {
                        self.vtables
                            .get_index_of(&TypeVariant::complex(ComplexType::Array(ArrayType {
                                element: ty.element.clone(),
                                size: None,
                            })))
                            .and_then(|v| self._find_vtable_function_index(v, function_name.as_ref()))
                            .or_else(|| {
                                self.vtables
                                    .get_index_of(&TypeVariant::complex(ComplexType::Array(ArrayType {
                                        element: TypeVariant::Generic(0).into(),
                                        size: None,
                                    })))
                                    .and_then(|v| self._find_vtable_function_index(v, function_name.as_ref()))
                            })
                            .or_else(|| {
                                self.vtables
                                    .iter()
                                    .position(|t| t.0.as_array().is_some_and(|arr| matches!(arr.element.variant, TypeVariant::Generic(_))))
                                    .and_then(|v| self._find_vtable_function_index(v, function_name.as_ref()))
                            })
                    },
                )
            },
            |v| self._find_vtable_function_index(v, function_name.as_ref()),
        )
    }

    pub fn get_vtable_index<T: AsRef<str>>(&self, ty: &TypeVariant) -> Option<usize> {
        self.vtables.get_index_of(ty).map_or_else(
            || {
                ty.as_array().map_or_else(
                    || {
                        ty.as_component().map_or_else(
                            || self.vtables.get_index_of(&any()),
                            |_| self.vtables.get_index_of(&TypeVariant::Primitive(PrimitiveType::Component)),
                        )
                    },
                    |ty| {
                        self.vtables
                            .get_index_of(&TypeVariant::complex(ComplexType::Array(ArrayType {
                                element: ty.element.clone(),
                                size: None,
                            })))
                            .or_else(|| {
                                self.vtables.get_index_of(&TypeVariant::complex(ComplexType::Array(ArrayType {
                                    element: TypeVariant::Generic(0).into(),
                                    size: None,
                                })))
                            })
                            .or_else(|| {
                                self.vtables
                                    .iter()
                                    .position(|t| t.0.as_array().is_some_and(|arr| matches!(arr.element.variant, TypeVariant::Generic(_))))
                            })
                    },
                )
            },
            Some,
        )
    }

    pub fn implements_trait(&self, ty: &TypeVariant, trait_index: usize) -> bool {
        self.vtables.get(ty).is_some_and(|vtables| vtables.contains_key(&Some(trait_index)))
    }

    pub fn get_vtable_method_index<T: AsRef<str>>(&self, ty: &TypeVariant, name: T) -> Option<(usize, Option<usize>, usize)> {
        self.find_vtable_function_index(ty, name.as_ref())
    }

    pub fn get_local_index<T: AsRef<str>>(&self, name: T) -> Option<usize> {
        for frame in self.frames.iter().rev() {
            if let Some(index) = frame.get_index_of(name.as_ref()) {
                return Some(index);
            }
        }

        None
    }

    pub fn extend_vm(&self, vm: &mut Vm) {
        vm.types = self.types.values().cloned().collect();
        vm.vtables = self
            .vtables
            .iter()
            .map(|vtable| {
                (
                    vtable.0.clone(),
                    vtable
                        .1
                        .iter()
                        .map(|(index, functions)| (*index, functions.values().map(|v| v.1.clone()).collect()))
                        .collect(),
                )
            })
            .collect();

        vm.frames = self
            .frames
            .iter()
            .map(|variable| StackFrame {
                locals: variable.values().filter_map(|variable| variable.value.clone()).collect(),
            })
            .collect();
    }

    pub fn as_vm(&self) -> Vm {
        Vm {
            state: Box::new(()),
            types: self.types.values().cloned().collect(),
            impls: self.impls.clone(),
            vtables: self
                .vtables
                .iter()
                .map(|vtable| {
                    (
                        vtable.0.clone(),
                        vtable
                            .1
                            .iter()
                            .map(|(index, functions)| (*index, functions.values().map(|v| v.1.clone()).collect()))
                            .collect(),
                    )
                })
                .collect(),
            frames: self
                .frames
                .iter()
                .map(|variable| StackFrame {
                    locals: variable.values().filter_map(|variable| variable.value.clone()).collect(),
                })
                .collect(),
            stack: SmallVec::new(),
        }
    }
}

pub trait Compile<T = bool> {
    /// # Errors
    ///
    /// Returns `CompileError` if compilation fails. This can happen in two
    /// cases: if the referenced variable is not found or if type inference
    /// fails.
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult<T>;
}

pub trait GetType {
    /// # Errors
    ///
    /// Returns `TypeError` if type inference fails. This can happen if, for
    /// example, the required type is not declared.
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult;
}

pub trait GetPositionedType {
    /// # Errors
    ///
    /// Returns `TypeError` if type inference fails. This can happen if, for
    /// example, the required type is not declared.
    fn get_type(&self, compiler: &Compiler) -> TypeResult;
}

impl<T: GetType> GetPositionedType for Positioned<T> {
    fn get_type(&self, compiler: &Compiler) -> TypeResult {
        self.value.get_type(compiler, self.span)
    }
}

#[cfg(test)]
mod tests {
    use mollie_vm::{
        ComponentChildren, FromValue, ObjectValue, Struct, TypeVariant, Value, Vm, any, array_of, boolean, component, float, function, integer, string,
        structure, void,
    };

    use crate::Compiler;

    //     const BASIC_UI2: &str = "declare Button inherits Container {
    //     title: string,
    //     waat_size_prop: integer,
    //     children: component,

    //     Rectangle from <self as Element> {

    //     }
    // }

    // Rectangle {
    //     width: 100px,
    //     height: 20%,
    //     corner_radius: all(12px),
    //     background: 0x00FF00,
    //     foreground: rgb(255, 0, 0),
    //     some_flag: true,
    //     string_value: \"okak!\",

    //     Button {
    //         title: \"xz\",
    //         waat_size_prop: 2,

    //         Rectangle {
    //             width: 100px,
    //             height: 20%,
    //             corner_radius: all(12px),
    //             background: 0x00FF00,
    //             foreground: rgb(255, 0, 0),
    //             some_flag: true,
    //             string_value: \"okak!\",
    //         }
    //     }
    // }";

    // const BASIC_UI: &str = include_str!("../tests/main.mol");

    #[allow(dead_code)]
    #[derive(Debug)]
    struct Color {
        red: u8,
        green: u8,
        blue: u8,
    }

    impl FromValue for Color {
        fn from_value(value: &Value) -> Option<Self> {
            let object = value.as_object()?;
            let object_ref = object.borrow();
            let structure = object_ref.as_struct()?;

            let red = structure.get_property("red")?;
            let green = structure.get_property("green")?;
            let blue = structure.get_property("blue")?;

            Some(Self { red, green, blue })
        }
    }

    #[allow(dead_code)]
    fn get_value_method<T: AsRef<str>>(value: &Value, trait_index: usize, name: T, compiler: &Compiler, vm: &Vm) -> Option<Value> {
        let ty = value.get_type()?;

        if compiler.implements_trait(&ty.variant, trait_index) {
            compiler
                .get_vtable_method_index(&ty.variant, name)
                .map(|(vtable, vtable2, method)| vm.vtables[vtable][&vtable2][method].clone())
        } else {
            None
        }
    }

    fn add_builtins(compiler: &mut Compiler) {
        compiler.vtable_func(
            string(),
            "length",
            function(true, [string()], integer()).into(),
            Value::object(ObjectValue::native_func(|_, args| {
                let Value::Object(object) = &args[0] else { unreachable!() };
                let ObjectValue::String(string) = &*object.borrow() else { unreachable!() };

                Some(Value::Integer(string.len() as i64))
            })),
        );

        compiler.vtable_func(
            array_of(TypeVariant::Generic(0), None),
            "size",
            function(true, [array_of(TypeVariant::Generic(0), None)], integer()).into(),
            Value::object(ObjectValue::native_func(|_, args| {
                let Value::Object(object) = &args[0] else { unreachable!("{}", args[0]) };
                let ObjectValue::Array(array) = &*object.borrow() else { unreachable!() };

                Some(Value::Integer(array.len() as i64))
            })),
        );

        compiler.var_value(
            "println",
            function(false, [any()], void()),
            Value::object(ObjectValue::native_func(|_, args| println(args))),
        );

        compiler.var_value(
            "println_type",
            function(false, [any()], void()),
            Value::object(ObjectValue::native_func(|_, args| println_type(args))),
        );
    }

    #[allow(dead_code)]
    fn add_customs(compiler: &mut Compiler) -> usize {
        let color_ty = structure([("red", integer()), ("green", integer()), ("blue", integer())]);

        compiler.add_type("Color", color_ty.clone());
        compiler.add_type(
            "Thickness",
            structure([("left", integer()), ("top", integer()), ("right", integer()), ("bottom", integer())]),
        );

        compiler.add_type(
            "Rectangle",
            component(
                [
                    ("width", false, integer().into()),
                    ("height", false, integer().into()),
                    ("corner_radius", false, compiler.get_type("Thickness")),
                    ("background", false, compiler.get_type("Color")),
                    ("foreground", false, compiler.get_type("Color")),
                    ("some_flag", false, boolean().into()),
                    ("string_value", false, string().into()),
                    ("test_array", false, array_of(boolean(), Some(3)).into()),
                ],
                ComponentChildren::MaybeSingle,
            ),
        );

        let context_ty = structure::<String, TypeVariant, _>([]);

        compiler.add_type("DrawContext", context_ty.clone());
        compiler.vtable_func(
            context_ty.clone(),
            "draw_rect",
            function(true, [context_ty.clone(), float(), float(), float(), float(), color_ty], void()).into(),
            Value::object(ObjectValue::native_func(|_, args| {
                let x = args[1].as_float()?;
                let y = args[2].as_float()?;
                let w = args[3].as_float()?;
                let h = args[4].as_float()?;
                let color = args[5].to::<Color>()?;

                println!("{x}x{y} {w}x{h} {color:#?}");

                None
            })),
        );

        let drawable = compiler.add_trait("Drawable").method("draw", [context_ty.clone()], void()).build();

        compiler.var_value(
            "all",
            function(false, [integer()], compiler.get_type("Thickness")),
            Value::object(ObjectValue::native_func(|vm, args| Some(all(vm, args)))),
        );

        compiler.var_value(
            "rgb",
            function(false, [integer(), integer(), integer()], compiler.get_type("Color")),
            Value::object(ObjectValue::native_func(|vm, args| Some(rgb(vm, args)))),
        );

        compiler.var_value(
            "hex",
            function(false, [integer()], compiler.get_type("Color")),
            Value::object(ObjectValue::native_func(|vm, args| Some(hex(vm, args)))),
        );

        drawable
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn chaotic_test() {
        let mut compiler = Compiler::default();

        add_builtins(&mut compiler);

        compiler.compile_program_text(include_str!("../tests/std.mol")).unwrap();

        match compiler.compile_program_text(include_str!("../tests/main.mol")) {
            Ok(chunk) => {
                println!("{chunk}");

                let mut vm = compiler.as_vm();

                let value = vm.execute(&chunk);

                println!("/*** LOCALS START ***/");

                for frame in &vm.frames {
                    for value in &frame.locals {
                        println!("{value}");
                    }
                }

                println!("/*** LOCALS END ***/");

                println!("/*** STACK START ***/");

                for value in &vm.stack {
                    println!("{value}");
                }

                println!("/*** STACK END ***/");

                if let Some(value) = value {
                    println!("/*** RETURNED VALUE ***/");
                    println!("{value}");
                }
            }
            Err(error) => println!("{error}"),
        }
    }

    fn println_type(mut args: Vec<Value>) -> Option<Value> {
        let value = args.remove(0);

        println!("{}", value.get_type()?);

        None
    }

    fn println(mut args: Vec<Value>) -> Option<Value> {
        let value = args.remove(0);

        println!("{value}");

        None
    }

    fn rgb(vm: &Vm, mut args: Vec<Value>) -> Value {
        let ty = vm.types[0].clone();
        let red = args.remove(0);
        let green = args.remove(0);
        let blue = args.remove(0);

        Value::object(ObjectValue::Struct(Struct {
            ty,
            values: vec![red, green, blue],
        }))
    }

    fn all(vm: &Vm, mut args: Vec<Value>) -> Value {
        let ty = vm.types[1].clone();
        let value = args.remove(0);

        Value::object(ObjectValue::Struct(Struct {
            ty,
            values: vec![value.clone(), value.clone(), value.clone(), value],
        }))
    }

    fn hex(vm: &Vm, mut args: Vec<Value>) -> Value {
        let ty = vm.types[0].clone();
        let Value::Integer(color) = args.remove(0) else { unreachable!() };
        let color = color.cast_unsigned();
        let red = (color & 0xFF) as u8;
        let green = ((color >> 8) & 0xFF) as u8;
        let blue = ((color >> 16) & 0xFF) as u8;

        Value::object(ObjectValue::Struct(Struct {
            ty,
            values: vec![
                Value::Integer(i64::from(red)),
                Value::Integer(i64::from(green)),
                Value::Integer(i64::from(blue)),
            ],
        }))
    }
}
