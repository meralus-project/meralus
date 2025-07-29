use indexmap::{IndexMap, map::Entry};
use mollie_lexer::{Lexer, Token};
use mollie_parser::{Parser, Statement, parse_statements_until};
use mollie_shared::{Positioned, Span};
use mollie_vm::{ArrayType, Chunk, ComplexType, PrimitiveType, Trait, TraitFunc, Type, TypeVariant, VTable, Value, Vm, any};

pub use self::error::{CompileError, CompileResult, TypeError, TypeResult};

mod error;
mod statement;
mod ty;

#[derive(Debug)]
pub struct Variable {
    pub id: usize,
    pub ty: TypeVariant,
    pub value: Option<Value>,
}

#[derive(Debug, Default)]
pub struct Compiler {
    traits: IndexMap<String, Trait>,
    types: IndexMap<String, Type>,
    vtables: IndexMap<TypeVariant, VTable>,
    locals: IndexMap<String, Variable>,
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

    pub fn build(self) {
        self.compiler.traits.insert(self.name, Trait {
            functions: self.functions,
            declared_at: None,
        });
    }
}

impl Compiler {
    pub fn add_trait<T: Into<String>>(&mut self, name: T) -> TraitBuilder<'_> {
        TraitBuilder::new(self, name)
    }

    pub fn add_type<T: Into<String>>(&mut self, name: T, ty: TypeVariant) {
        self.types.insert(name.into(), Type {
            variant: ty,
            declared_at: None,
        });
    }

    pub fn add_declared_type<T: Into<String>>(&mut self, name: T, ty: Type) {
        self.types.insert(name.into(), ty);
    }

    pub fn remove_type<T: AsRef<str>>(&mut self, name: T) {
        self.types.shift_remove(name.as_ref());
    }

    pub fn vtable_func<T: Into<String>>(&mut self, ty: TypeVariant, name: T, function_ty: Type, value: Value) {
        let name = name.into();

        match self.vtables.entry(ty) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().functions.insert(name, (function_ty, None, value));
            }
            Entry::Vacant(entry) => {
                entry.insert(VTable {
                    functions: IndexMap::from_iter([(name, (function_ty, None, value))]),
                });
            }
        }
    }

    pub fn var<T: Into<String>>(&mut self, name: T, ty: TypeVariant) {
        let id = self.locals.len();
        let name = name.into();

        self.add_type(name.clone(), ty.clone());
        self.locals.insert(name, Variable { id, ty, value: None });
    }

    pub fn remove_var<T: AsRef<str>>(&mut self, name: T) {
        let name = name.as_ref();

        self.remove_type(name);
        self.locals.shift_remove(name);
    }

    pub fn var_value<T: Into<String>>(&mut self, name: T, ty: TypeVariant, value: Value) {
        let id = self.locals.len();
        let name = name.into();

        self.add_type(name.clone(), ty.clone());
        self.locals.insert(name, Variable { id, ty, value: Some(value) });
    }

    pub fn compile_program_text<T: AsRef<str>>(&mut self, text: T) -> CompileResult<Chunk> {
        let mut parser = Parser::new(Lexer::lex(text.as_ref()));

        let program = match parse_statements_until(&mut parser, &Token::EOF) {
            Ok(statements) => statements,
            Err(error) => panic!("{error}"),
        };

        self.compile_program(program)
    }

    pub fn compile_program(&mut self, (statements, returned): (Vec<Positioned<Statement>>, Option<Positioned<Statement>>)) -> CompileResult<Chunk> {
        let mut chunk = Chunk::default();

        for statement in statements {
            self.compile(&mut chunk, statement)?;
        }

        if let Some(statement) = returned {
            self.compile(&mut chunk, statement)?;

            chunk.ret();
        }

        chunk.halt();

        Ok(chunk)
    }

    pub fn compile<O, T: Compile<O>>(&mut self, chunk: &mut Chunk, value: T) -> CompileResult<O> {
        value.compile(chunk, self)
    }

    pub fn get_positioned_type<T: GetType>(&mut self, value: &Positioned<T>) -> TypeResult {
        value.get_type(self)
    }

    pub fn get_value_type<T: GetType>(&mut self, value: &T, span: Span) -> TypeResult {
        value.get_type(self, span)
    }

    pub fn try_get_type<T: AsRef<str>>(&self, name: T) -> CompileResult<Type> {
        self.types.get(name.as_ref()).cloned().ok_or_else(|| CompileError::VariableNotFound {
            name: name.as_ref().to_string(),
        })
    }

    pub fn get_type<T: AsRef<str>>(&self, name: T) -> Type {
        self.types
            .get(name.as_ref())
            .map_or_else(|| panic!("{} not found", name.as_ref()), Clone::clone)
    }

    pub fn get_vtable(&self, ty: &TypeVariant) -> Option<&VTable> {
        self.vtables.get(ty).map_or_else(
            || {
                ty.as_array().map_or_else(
                    || {
                        ty.as_component().map_or_else(
                            || self.vtables.get(&any()),
                            |_| self.vtables.get(&TypeVariant::Primitive(PrimitiveType::Component)),
                        )
                    },
                    |ty| {
                        self.vtables.get(&TypeVariant::complex(ComplexType::Array(ArrayType {
                            element: ty.element.clone(),
                            size: None,
                        })))
                    },
                )
            },
            Some,
        )
    }

    pub fn get_vtable_index(&self, ty: &TypeVariant) -> Option<usize> {
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
                        self.vtables.get_index_of(&TypeVariant::complex(ComplexType::Array(ArrayType {
                            element: ty.element.clone(),
                            size: None,
                        })))
                    },
                )
            },
            Some,
        )
    }

    pub fn get_vtable_method_index<T: AsRef<str>>(&self, ty: &TypeVariant, name: T) -> Option<(usize, usize)> {
        self.get_vtable_index(ty)
            .and_then(|vtable| self.vtables[vtable].functions.get_index_of(name.as_ref()).map(|function| (vtable, function)))
    }

    pub fn get_local_index<T: AsRef<str>>(&self, name: T) -> Option<usize> {
        self.locals.get_index_of(name.as_ref())
    }

    pub fn extend_vm(&self, vm: &mut Vm) {
        vm.types = self.types.values().cloned().collect();
        vm.vtables = self
            .vtables
            .values()
            .map(|vtable| vtable.functions.values().map(|value| value.2.clone()).collect())
            .collect();

        vm.locals = self.locals.values().map(|variable| variable.value.clone().unwrap_or(Value::Null)).collect();
    }

    pub fn as_vm(&self) -> Vm {
        Vm {
            state: Box::new(()),
            types: self.types.values().cloned().collect(),
            vtables: self
                .vtables
                .values()
                .map(|vtable| vtable.functions.values().map(|value| value.2.clone()).collect())
                .collect(),
            locals: self.locals.values().map(|variable| variable.value.clone().unwrap_or(Value::Null)).collect(),
            stack: Vec::new(),
        }
    }
}

pub trait Compile<T = ()> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult<T>;
}

pub trait GetType {
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult;
}

pub trait GetPositionedType {
    fn get_type(&self, compiler: &Compiler) -> TypeResult;
}

impl<T: GetType> GetPositionedType for Positioned<T> {
    fn get_type(&self, compiler: &Compiler) -> TypeResult {
        self.value.get_type(compiler, self.span)
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use mollie_lexer::{Lexer, Token};
    use mollie_parser::{Parser, parse_statements_until};
    use mollie_vm::{
        ComponentChildren, FromValue, ObjectValue, Struct, StructType, TypeVariant, Value, Vm, any, array_of, boolean, component, float, function, integer,
        string, structure, void,
    };

    use crate::Compiler;

    const BASIC_UI2: &str = "declare Button inherits Container {
    title: string,
    waat_size_prop: integer,
    children: component,
    
    Rectangle from <self as Element> {

    }
}

Rectangle {
    width: 100px,
    height: 20%,
    corner_radius: all(12px),
    background: 0x00FF00,
    foreground: rgb(255, 0, 0),
    some_flag: true,
    string_value: \"okak!\",

    Button {
        title: \"xz\",
        waat_size_prop: 2,

        Rectangle {
            width: 100px,
            height: 20%,
            corner_radius: all(12px),
            background: 0x00FF00,
            foreground: rgb(255, 0, 0),
            some_flag: true,
            string_value: \"okak!\",
        }
    }
}";

    // trait Iterable<T> {
    //  fn next(self) -> T;
    // }
    //
    // struct ArrayIter<T> {
    //  array: T[],
    //  index: integer,
    // }
    //
    // impl<T> trait Iterable<T> for ArrayIter<T> {
    //  fn next(self) -> T {
    //   self.index += 1;
    //
    //   self.array[self.index]
    //  }
    // }

    const BASIC_UI: &str = "println(\"эвоно как\");
println([\"эвоно как\", \"эвоно как\", \"эвоно как\"]);

struct Daun {
    tochno: boolean
}

declare Rect {
    width: float,
    height: float,
    color: Color,
}

impl trait Drawable for Rect {
    fn draw(self, context: DrawContext) {
        context.draw_rect(0.0, 0.0, self.width, self.height, self.color);
        context.draw_rect(40.0, 40.0, self.width, self.height, self.color);
    }
}

impl string {
    fn get_prikol() -> string {
        \"HELLO\"
    }

    fn log_len(self) {
        println(self.length());
    }
}

impl string[] {
    fn lmao(self) -> boolean {
        true
    }
}

println(<string>::get_prikol().length());
println(Daun { tochno: true }.tochno);

\"dubai\".log_len();

println([\"эвоно как\", \"эвоно как\", \"эвоно как\", \"эвоно как\"].lmao());
println([\"эвоно как\", \"эвоно как\", \"эвоно как\"].lmao());

Rectangle {
    width: 100px,
    height: 20%,
    corner_radius: all(12px),
    background: hex(0x00FF00),
    foreground: rgb(255, 0, 0),
    some_flag: true,
    string_value: \"okak!\",
    test_array: [true, false, true],

    Rectangle {
        width: 100px,
        height: 20%,
        corner_radius: all(12px),
        background: hex(0x00FF00),
        foreground: rgb(255, 0, 0),
        some_flag: true,
        string_value: \"йобань\",
        test_array: [true, false, true],
    }
};
    
Rect { width: 100.0, height: 100.0, color: rgb(0xFF, 0xFF, 0x00) }";

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

    fn get_value_method<T: AsRef<str>>(value: &Value, name: T, compiler: &Compiler, vm: &Vm) -> Option<Value> {
        let ty = value.get_type()?;

        compiler
            .get_vtable_method_index(&ty, name)
            .map(|(vtable, method)| vm.vtables[vtable][method].clone())
    }

    #[test]
    fn chaotic_test() {
        let mut parser = Parser::new(Lexer::lex(BASIC_UI));

        let program = match parse_statements_until(&mut parser, &Token::EOF) {
            Ok(statements) => statements,
            Err(error) => {
                return println!(
                    "{error} [{:?}]",
                    error.location().map_or("", |location| &BASIC_UI2[location.start..location.end])
                );
            }
        };

        let mut compiler = Compiler {
            traits: IndexMap::new(),
            types: IndexMap::new(),
            locals: IndexMap::new(),
            vtables: IndexMap::new(),
        };

        compiler.vtable_func(
            string(),
            "length",
            function(true, [string()], integer()).into(),
            Value::object(ObjectValue::NativeFunc(|_, args| {
                let Value::Object(object) = &args[0] else { unreachable!() };
                let ObjectValue::String(string) = &*object.borrow() else { unreachable!() };

                Some(Value::Integer(string.len() as i64))
            })),
        );

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

        let context_ty = TypeVariant::complex(mollie_vm::ComplexType::Struct(StructType { properties: Vec::new() }));

        compiler.add_type("DrawContext", context_ty.clone());
        compiler.vtable_func(
            context_ty.clone(),
            "draw_rect",
            function(true, [context_ty.clone(), float(), float(), float(), float(), color_ty], void()).into(),
            Value::object(ObjectValue::NativeFunc(|vm, args| {
                let x = args[1].as_float()?;
                let y = args[2].as_float()?;
                let w = args[3].as_float()?;
                let h = args[4].as_float()?;
                let color = args[5].to::<Color>()?;

                println!("{x}x{y} {w}x{h} {color:#?}");

                None
            })),
        );

        compiler.add_trait("Drawable").method("draw", [context_ty.clone()], void()).build();

        compiler.var_value(
            "all",
            function(false, [integer()], compiler.get_type("Thickness")),
            Value::object(ObjectValue::NativeFunc(|vm, args| Some(all(vm, args)))),
        );

        compiler.var_value(
            "rgb",
            function(false, [integer(), integer(), integer()], compiler.get_type("Color")),
            Value::object(ObjectValue::NativeFunc(|vm, args| Some(rgb(vm, args)))),
        );

        compiler.var_value(
            "println",
            function(false, [any()], void()),
            Value::object(ObjectValue::NativeFunc(|_, args| println(args))),
        );

        compiler.var_value(
            "hex",
            function(false, [integer()], compiler.get_type("Color")),
            Value::object(ObjectValue::NativeFunc(|vm, args| Some(hex(vm, args)))),
        );

        std::fs::write("./parse", format!("{program:#?}")).unwrap();

        match compiler.compile_program(program) {
            Ok(chunk) => {
                println!("{chunk}");

                let mut vm = compiler.as_vm();

                let value = vm.execute(&chunk);

                println!("/*** LOCALS ***/");

                for value in &vm.locals {
                    println!("{value}");
                }

                println!("/*** STACK ***/");

                for value in &vm.stack {
                    println!("{value}");
                }

                if let Some(value) = value {
                    vm.execute_function(
                        vec![
                            value.clone(),
                            Value::object(ObjectValue::Struct(Struct {
                                ty: context_ty.into(),
                                values: Vec::new(),
                            })),
                        ],
                        get_value_method(&value, "draw", &compiler, &vm).unwrap(),
                    );

                    println!("/*** RETURNED ***/");
                    println!("{value}");
                }
            }
            Err(error) => println!("{error}"),
        }
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
