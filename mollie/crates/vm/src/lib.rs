mod inst;
mod ty;
mod value;

use std::{any::Any, cell::RefCell, rc::Rc};

use indexmap::IndexMap;

pub use self::{
    inst::{Chunk, Inst},
    ty::*,
    value::*,
};

#[derive(Debug)]
pub struct VTable {
    pub functions: IndexMap<String, (Type, Option<usize>, Value)>,
}

#[derive(Debug)]
pub struct Vm {
    pub state: Box<dyn Any>,
    pub types: Vec<Type>,
    pub vtables: Vec<Vec<Value>>,
    pub stack: Vec<Value>,
    pub locals: Vec<Value>,
}

impl Vm {
    pub fn set_state<T: Any>(&mut self, value: T) {
        self.state = Box::new(value);
    }

    // pub fn get_type<T: AsRef<str>>(&self, name: T) -> Type {
    //     self.types.get(name.as_ref()).unwrap().clone()
    // }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
    }

    fn truncate(&mut self, count: usize) -> Vec<Value> {
        self.stack.split_off(self.stack.len() - count)
    }

    pub fn execute_function(&mut self, args: Vec<Value>, value: Value) -> Option<Value> {
        let args_count = args.len();
        let mut returned = None;

        if let Value::Object(object) = value {
            if let ObjectValue::NativeFunc(func) = &*object.borrow() {
                returned = func(self, args);
            } else if let ObjectValue::Function(func) = &*object.borrow() {
                self.add_locals(args);

                returned = self.execute(&func.body);

                self.remove_locals(args_count);
            }
        } else {
            panic!("there's nothing to call: {args:#?} {value} {args_count}");
        }

        returned
    }

    fn add_locals(&mut self, values: Vec<Value>) {
        self.locals.extend(values);
    }

    fn remove_locals(&mut self, count: usize) {
        self.locals.truncate(self.locals.len() - count);
    }

    fn set_local(&mut self, id: usize, value: Value) {
        self.locals.resize(id + 1, Value::Null);
        self.locals[id] = value;
    }

    fn get_local(&self, id: usize) -> Value {
        self.locals[id].clone()
    }

    pub fn execute(&mut self, chunk: &Chunk) -> Option<Value> {
        let mut is_running = true;
        let mut pc = 0;
        let mut returned = None;

        while is_running {
            match chunk.instructions[pc] {
                Inst::Pop => {
                    self.pop();
                }
                Inst::LoadConst(constant) => self.push(chunk.constants[constant].clone()),
                Inst::SetLocal(local) => {
                    let value = self.pop();

                    self.set_local(local, value);
                }
                Inst::GetLocal(local) => self.push(self.get_local(local)),
                Inst::CreateArray(size) => {
                    let elements = self.truncate(size);

                    self.push(Value::object(ObjectValue::Array(elements)));
                }
                Inst::GetProperty(pos) => {
                    let value = self.pop();

                    if let Value::Object(object) = value {
                        if let ObjectValue::Component(component) = &*object.borrow() {
                            self.push(component.values[pos].clone());
                        } else if let ObjectValue::Struct(component) = &*object.borrow() {
                            self.push(component.values[pos].clone());
                        }
                    }
                }
                Inst::Instantiate(ty, have_children) => {
                    if let Some(ty) = if self.types[ty].variant.is_component() || self.types[ty].variant.is_struct() {
                        Some(self.types[ty].clone())
                    } else {
                        None
                    } {
                        if let Some(component) = ty.variant.as_component() {
                            let mut values = self.truncate(component.properties.len() + usize::from(have_children));
                            let children = if have_children && let Some(Value::Object(value)) = values.pop() {
                                if matches!(&*value.borrow(), ObjectValue::Array(_)) {
                                    let ObjectValue::Array(values) = Rc::into_inner(value).unwrap().into_inner() else {
                                        unreachable!()
                                    };

                                    values
                                } else {
                                    vec![Value::Object(value)]
                                }
                            } else {
                                Vec::new()
                            };

                            self.push(Value::Object(Rc::new(RefCell::new(ObjectValue::Component(Component { ty, values, children })))));
                        } else if let Some(structure) = ty.variant.as_struct() {
                            let values = self.truncate(structure.properties.len());

                            self.push(Value::Object(Rc::new(RefCell::new(ObjectValue::Struct(Struct { ty, values })))));
                        }
                    } else {
                        panic!("there's nothing to instantiate");
                    }
                }
                Inst::Call(args_count) => {
                    let args = self.truncate(args_count);
                    let value = self.pop();

                    if let Some(value) = self.execute_function(args, value) {
                        self.push(value);
                    }
                }
                Inst::GetTypeFunction(ty, function) => {
                    let function = self.vtables[ty][function].clone();

                    self.push(function);
                }
                Inst::Ret => {
                    returned.replace(self.pop());
                }
                Inst::Halt => is_running = false,
            }

            pc += 1;
        }

        returned
    }
}
