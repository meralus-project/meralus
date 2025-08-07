mod inst;
mod ty;
mod value;

use std::{any::Any, rc::Rc};

use indexmap::IndexMap;
pub use smallvec::SmallVec;

pub use self::{
    inst::{Chunk, Inst},
    ty::*,
    value::*,
};

#[derive(Debug)]
pub struct VTable {
    pub functions: IndexMap<String, (Type, Option<usize>, Value)>,
}

#[derive(Debug, Default)]
pub struct StackFrame {
    pub locals: Vec<Value>,
}

const STACK_SIZE: usize = 32;

#[derive(Debug)]
pub struct Vm {
    pub state: Box<dyn Any>,
    pub types: Vec<Type>,
    pub impls: IndexMap<TypeVariant, Vec<usize>>,
    pub vtables: IndexMap<TypeVariant, IndexMap<Option<usize>, Vec<Value>>>,
    pub stack: SmallVec<Value, STACK_SIZE>,
    pub frames: Vec<StackFrame>,
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
        self.stack.pop().unwrap_or_else(|| panic!("{:#?}", self.stack))
    }

    fn truncate(&mut self, count: usize) -> SmallVec<Value, STACK_SIZE> {
        self.stack.split_off(self.stack.len() - count)
    }

    pub fn execute_function(&mut self, args: Vec<Value>, value: Value) -> Option<Value> {
        let args_count = args.len();
        let returned;

        if let Value::Object(object) = value {
            if let ObjectValue::NativeFunc(func) = &*object.borrow() {
                self.frames.push(StackFrame::default());

                returned = func(self, args);

                self.frames.pop();
            } else if let ObjectValue::Function(func) = &*object.borrow() {
                self.frames.push(StackFrame::default());
                self.add_locals(args);

                println!("executing func: {}", func.body);

                returned = self.execute(&func.body);

                self.remove_locals(args_count);
                self.frames.pop();
            } else {
                panic!("there's nothing to call: {args:#?} {} {args_count}", object.borrow());
            }
        } else {
            panic!("there's nothing to call: {args:#?} {value} {args_count}");
        }

        returned
    }

    fn current_frame(&self) -> &StackFrame {
        self.frames.last().unwrap()
    }

    fn current_frame_mut(&mut self) -> &mut StackFrame {
        self.frames.last_mut().unwrap()
    }

    fn add_locals(&mut self, values: Vec<Value>) {
        self.current_frame_mut().locals.extend(values);
    }

    fn remove_locals(&mut self, count: usize) {
        let locals = self.current_frame().locals.len();

        self.current_frame_mut().locals.truncate(locals - count);
    }

    fn set_local(&mut self, id: usize, value: Value) {
        self.current_frame_mut().locals.resize(id + 1, Value::Null);
        self.current_frame_mut().locals[id] = value;
    }

    fn get_local(&self, frame: usize, id: usize) -> Value {
        // 2 = 4 - 1 - 1
        // 1 = 4 - 2 - 1

        // 2 = 5 - 1 - 2
        // 2 = 5 - 1 - 2

        self.frames[frame]
            .locals
            .get(id)
            .unwrap_or_else(|| panic!("no value at {frame}:{id} (getl {} {id})", self.frames.len() - 1 - frame))
            .clone()
    }

    #[allow(clippy::too_many_lines)]
    pub fn execute(&mut self, chunk: &Chunk) -> Option<Value> {
        println!("EXECUTIGN CHUNK {}", chunk.frame);

        let mut is_running = true;
        let mut pc = 0;
        let mut returned = None;

        while is_running {
            println!(
                "executing {}: {}",
                chunk.instructions[pc],
                self.stack.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
            );

            match chunk.instructions[pc] {
                Inst::Add => {
                    let a = self.pop();
                    let b = self.pop();

                    self.push(a + b);
                }
                Inst::Sub => {
                    // 2 - 1 => [1, 2] sub
                    let a = self.pop();
                    let b = self.pop();

                    self.push(a - b);
                }
                Inst::Mul => {
                    let a = self.pop();
                    let b = self.pop();

                    self.push(a * b);
                }
                Inst::Div => {
                    let a = self.pop();
                    let b = self.pop();

                    self.push(a / b);
                }
                Inst::Equals => {
                    let a = self.pop();
                    let b = self.pop();

                    self.push(Value::Boolean(a == b));
                }
                Inst::LessThan => {
                    let a = self.pop();
                    let b = self.pop();

                    self.push(Value::Boolean(a < b));
                }
                Inst::GreaterThan => {
                    let a = self.pop();
                    let b = self.pop();

                    self.push(Value::Boolean(a > b));
                }
                Inst::Negate => {
                    let value = self.pop();

                    self.push(-value);
                }
                Inst::Copy => {
                    if let Some(last) = self.stack.last() {
                        self.push(last.clone());
                    } else {
                        panic!("there's nothing to copy")
                    }
                }
                Inst::Jump(count) => {
                    if count.is_negative() {
                        let count = count.unsigned_abs();

                        pc = pc.max(count) - count;
                    } else {
                        pc += count.unsigned_abs();
                    }

                    continue;
                }
                Inst::JumpIfFalse(count) => {
                    let value = self.pop();

                    if value == Value::Boolean(false) {
                        pc += count;

                        continue;
                    } else if value != Value::Boolean(true) {
                        panic!("got incorrect value for jumping: {value}");
                    }
                }
                Inst::Pop => {
                    self.pop();
                }
                Inst::PushFrame => {
                    self.frames.push(StackFrame::default());
                }
                Inst::PopFrame => {
                    self.frames.pop();
                }
                Inst::LoadConst(constant) => self.push(chunk.constants[constant].clone()),
                Inst::SetLocal(local) => {
                    let value = self.pop();

                    self.set_local(local, value);
                }
                Inst::GetLocal(frame, local) => {
                    // println!("getlocal {}:{local} ({} frames)", chunk.frame - frame, self.frames.len());

                    self.push(self.get_local(self.frames.len() -1 - frame, local));
                }
                Inst::CreateArray(size) => {
                    let elements = self.truncate(size);

                    self.push(Value::object(ObjectValue::Array(elements.into_vec())));
                }
                Inst::GetArrayElement => {
                    let index = self.pop();
                    let value = self.pop();

                    if let Value::Object(object) = value
                        && let ObjectValue::Array(values) = &*object.borrow()
                        && let Value::Integer(pos) = index
                    {
                        self.push(values[pos as usize].clone());
                    } else {
                        panic!("trying to get array element from invalid value type")
                    }
                }
                Inst::SetArrayElement => {
                    let element_value = self.pop();
                    let index = self.pop();
                    let value = self.pop();

                    if let Value::Object(object) = value
                        && let ObjectValue::Array(values) = &mut *object.borrow_mut()
                        && let Value::Integer(pos) = index
                    {
                        values[pos as usize] = element_value;
                    } else {
                        panic!("trying to set array element for invalid value type")
                    }
                }
                Inst::GetProperty(pos) => {
                    let value = self.pop();

                    if let Value::Object(object) = value {
                        if let ObjectValue::Component(component) = &*object.borrow() {
                            if pos == component.ty.variant.as_component().unwrap().properties.len() {
                                match component.ty.variant.as_component().unwrap().children {
                                    ComponentChildren::None => {}
                                    ComponentChildren::Single => {
                                        self.push(component.children[0].clone());
                                    }
                                    ComponentChildren::MaybeSingle => {
                                        if let Some(value) = component.children.first().cloned() {
                                            self.push(value);
                                        }
                                    }
                                    ComponentChildren::Multiple(_) => {
                                        self.push(Value::object(ObjectValue::Array(component.children.clone())));
                                    }
                                    ComponentChildren::MaybeMultiple(_) => {
                                        self.push(Value::object(ObjectValue::Array(component.children.clone())));
                                    }
                                }
                            } else {
                                self.push(component.values[pos].clone());
                            }
                        } else if let ObjectValue::Struct(component) = &*object.borrow() {
                            self.push(component.values[pos].clone());
                        } else if let ObjectValue::Enum(enum_instance) = &*object.borrow() {
                            self.push(enum_instance.values[pos].clone());
                        } else {
                            panic!("trying to get property from invalid value type")
                        }
                    } else {
                        panic!("we've got somethin invalid: {value}");
                    }
                }
                Inst::SetProperty(pos) => {
                    let property_value = self.pop();
                    let value = self.pop();

                    if let Value::Object(object) = value {
                        if let ObjectValue::Component(component) = &mut *object.borrow_mut() {
                            component.values[pos] = property_value;
                        } else if let ObjectValue::Struct(component) = &mut *object.borrow_mut() {
                            component.values[pos] = property_value;
                        } else {
                            panic!("trying to set property for invalid value type")
                        }
                    } else {
                        panic!("trying to set property for invalid value type")
                    }
                }
                Inst::Impls(trait_index) => {
                    let value = self.pop();

                    let result = if let Some(value_type) = value.get_type() {
                        if let Some(traits) = self.impls.get(&value_type.variant) {
                            traits.contains(&trait_index)
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    self.push(Value::Boolean(result));
                }
                Inst::IsInstanceOf(ty, variant) => {
                    let value = self.pop();

                    let result = if let Some(value_type) = value.get_type() {
                        let expected_type = &self.types[ty].variant;

                        if value_type.variant.same_as(expected_type, &[]) {
                            if expected_type.is_enum() {
                                if let Some(object) = value.as_object() {
                                    if let ObjectValue::Enum(enum_instance) = &*object.borrow() {
                                        if enum_instance.variant == variant { true } else { false }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                true
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    self.push(Value::Boolean(result));
                }
                Inst::Instantiate(ty, variant, have_children) => {
                    if let Some(ty) = if self.types[ty].variant.is_component() || self.types[ty].variant.is_struct() || self.types[ty].variant.is_enum() {
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

                            self.push(Value::object(ObjectValue::Component(Component {
                                ty,
                                values: values.into_vec(),
                                children,
                            })));
                        } else if let Some(structure) = ty.variant.as_struct() {
                            let values = self.truncate(structure.properties.len());

                            self.push(Value::object(ObjectValue::Struct(Struct { ty, values: values.into_vec() })));
                        } else if let Some(enumeration) = ty.variant.as_enum() {
                            let values = if let Some(properties) = &enumeration.variants[variant].1.properties {
                                self.truncate(properties.len()).into_vec()
                            } else {
                                Vec::new()
                            };

                            self.push(Value::object(ObjectValue::Enum(Enum { ty, variant, values })));
                        } else {
                            panic!("HOW")
                        }
                    } else {
                        panic!("there's nothing to instantiate");
                    }
                }
                Inst::Call(args_count) => {
                    let args = self.truncate(args_count);
                    let value = self.pop();

                    if let Some(value) = self.execute_function(args.into_vec(), value) {
                        self.push(value);
                    }
                }
                Inst::GetTypeFunction(ty, trait_index, function) => {
                    self.push(self.vtables[ty][&trait_index][function].clone());
                }
                Inst::GetTypeFunction2(trait_index, function) => {
                    let value = self.pop();

                    if let Some(ty) = value.get_type() {
                        if let Some(vtable) = self.vtables.get(&ty.variant) {
                            let func = vtable[&trait_index][function].clone();
                            let push_value_back = if let Some(o) = vtable[&trait_index][function].as_object() {
                                if let ObjectValue::Function(f) = &*o.borrow() { f.have_self } else { false }
                            } else {
                                false
                            };

                            println!("push back: {push_value_back}");

                            self.push(func);

                            if push_value_back {
                                self.push(value);
                            }
                        }
                    }
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
