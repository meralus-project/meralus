use std::{collections::hash_map::Entry, fmt};

use ahash::HashMap;
use serde::{Deserialize, Serialize, de::DeserializeSeed};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockStates {
    pub model: String,
    pub variants: Vec<BlockState>,
}

impl BlockStates {
    pub fn from_slice(bytes: &[u8], registry: &PropertyRegistry) -> Result<Self, serde_json::Error> {
        let value = serde_json::from_slice(bytes)?;

        for error in registry.validate_states(&value) {
            for error in error.errors {
                match error {
                    BlockStateValidationError::UnknownProperty { name } => eprintln!("unknown property `{name}`"),
                    BlockStateValidationError::UnknownVariant { name, expected } => eprintln!("unknown enum variant, expected `{expected:?}`, found `{name}`"),
                    BlockStateValidationError::UnexpectedType { expected, found } => eprintln!("expected `{expected:?}` property type, found `{found:?}`"),
                    BlockStateValidationError::FloatOutOfRange { value, min, max } => eprintln!("value `{value}` is out of range `{min}..={max}`"),
                    BlockStateValidationError::IntegerOutOfRange { value, min, max } => eprintln!("value `{value}` is out of range `{min}..={max}`"),
                }
            }
        }

        Ok(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(untagged)]
pub enum PropertyValue {
    Number(i64),
    Float(f32),
    String(String),
    Boolean(bool),
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => value.fmt(f),
            Self::Float(value) => value.fmt(f),
            Self::String(value) => f.write_str(&format!("{value:?}")),
            Self::Boolean(value) => value.fmt(f),
        }
    }
}

impl Eq for PropertyValue {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(untagged)]
pub enum PropertyType {
    Number { min: i64, max: i64 },
    Float { min: f32, max: f32 },
    Enum(Vec<String>),
    String,
    Boolean,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Property {
    pub name: &'static str,
    pub value: PropertyValue,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockState {
    pub when: HashMap<String, PropertyValue>,
    pub model: String,
}

impl BlockState {
    pub fn from_slice(bytes: &[u8], registry: &PropertyRegistry) -> Result<Self, serde_json::Error> {
        let value = serde_json::from_slice(bytes)?;

        for error in registry.validate_state(&value) {
            match error {
                BlockStateValidationError::UnknownProperty { name } => eprintln!("unknown property `{name}`"),
                BlockStateValidationError::UnknownVariant { name, expected } => eprintln!("unknown enum variant, expected `{expected:?}`, found `{name}`"),
                BlockStateValidationError::UnexpectedType { expected, found } => eprintln!("expected `{expected:?}` property type, found `{found:?}`"),
                BlockStateValidationError::FloatOutOfRange { value, min, max } => eprintln!("value `{value}` is out of range `{min}..={max}`"),
                BlockStateValidationError::IntegerOutOfRange { value, min, max } => eprintln!("value `{value}` is out of range `{min}..={max}`"),
            }
        }

        Ok(value)
    }
}

#[derive(Debug, Default)]
pub struct PropertyRegistry {
    properties: HashMap<String, PropertyType>,
}

pub struct NumericProperty<'a, T> {
    min: &'a mut T,
    max: &'a mut T,
}

impl<T> NumericProperty<'_, T> {
    pub fn with_range(self, min: T, max: T) -> Self {
        self.with_min(min).with_max(max)
    }

    pub fn with_min(self, value: T) -> Self {
        *self.min = value;

        self
    }

    pub fn with_max(self, value: T) -> Self {
        *self.max = value;

        self
    }
}

impl PropertyRegistry {
    pub fn get<T: AsRef<str>>(&self, name: T) -> Option<&PropertyType> {
        self.properties.get(name.as_ref())
    }

    pub fn add<T: Into<String>>(&mut self, name: T, ty: PropertyType) {
        let name = name.into();

        self.properties.insert(name, ty);
    }

    pub fn add_mut<T: Into<String>>(&mut self, name: T, ty: PropertyType) -> &mut PropertyType {
        let name = name.into();

        match self.properties.entry(name) {
            Entry::Occupied(entry) => {
                let value = entry.into_mut();

                *value = ty;

                value
            }
            Entry::Vacant(entry) => entry.insert(ty),
        }
    }

    pub fn add_num<T: Into<String>>(&mut self, name: T) -> NumericProperty<'_, i64> {
        let PropertyType::Number { min, max } = self.add_mut(name, PropertyType::Number { min: i64::MIN, max: i64::MAX }) else {
            unreachable!()
        };

        NumericProperty { min, max }
    }

    pub fn add_float<T: Into<String>>(&mut self, name: T) -> NumericProperty<'_, f32> {
        let PropertyType::Float { min, max } = self.add_mut(name, PropertyType::Float { min: f32::MIN, max: f32::MAX }) else {
            unreachable!()
        };

        NumericProperty { min, max }
    }

    pub fn add_str<T: Into<String>>(&mut self, name: T) {
        self.add(name, PropertyType::String);
    }

    pub fn add_enum<T: Into<String>, V: Into<String>, I: IntoIterator<Item = V>>(&mut self, name: T, variants: I) {
        self.add(name, PropertyType::Enum(variants.into_iter().map(Into::into).collect()));
    }

    pub fn add_bool<T: Into<String>>(&mut self, name: T) {
        self.add(name, PropertyType::Boolean);
    }

    pub fn validate_states<'a, 'b>(&'b self, states: &'a BlockStates) -> Vec<BlockStatesValidationError<'a, 'b>> {
        let mut errors = Vec::new();

        for (index, state) in states.variants.iter().enumerate() {
            errors.push(BlockStatesValidationError {
                errors: self.validate_state(state),
                index,
            });
        }

        errors
    }

    fn validate_state_into<'a, 'b>(&'b self, state: &'a BlockState, errors: &mut Vec<BlockStateValidationError<'a, 'b>>) {
        for (name, value) in &state.when {
            match self.get(name) {
                Some(property) => match (value, property) {
                    (&PropertyValue::Number(value), &PropertyType::Number { min, max }) => {
                        if value < min || value > max {
                            errors.push(BlockStateValidationError::IntegerOutOfRange { value, min, max });
                        }
                    }
                    (&PropertyValue::Float(value), &PropertyType::Float { min, max }) => {
                        if value < min || value > max {
                            errors.push(BlockStateValidationError::FloatOutOfRange { value, min, max });
                        }
                    }
                    (PropertyValue::String(variant), PropertyType::Enum(variants)) => {
                        if !variants.contains(variant) {
                            errors.push(BlockStateValidationError::UnknownVariant { name, expected: variants });
                        }
                    }
                    (PropertyValue::String(_), PropertyType::String) | (PropertyValue::Boolean(_), PropertyType::Boolean) => (),
                    (found, expected) => {
                        errors.push(BlockStateValidationError::UnexpectedType { expected, found });
                    }
                },
                None => errors.push(BlockStateValidationError::UnknownProperty { name }),
            }
        }
    }

    pub fn validate_state<'a, 'b>(&'b self, state: &'a BlockState) -> Vec<BlockStateValidationError<'a, 'b>> {
        let mut errors = Vec::new();

        self.validate_state_into(state, &mut errors);

        errors
    }
}

pub struct BlockStatesValidationError<'a, 'b> {
    pub errors: Vec<BlockStateValidationError<'a, 'b>>,
    pub index: usize,
}

pub enum BlockStateValidationError<'a, 'b> {
    UnknownProperty { name: &'a str },
    UnknownVariant { name: &'a str, expected: &'b [String] },
    UnexpectedType { expected: &'b PropertyType, found: &'a PropertyValue },
    FloatOutOfRange { value: f32, min: f32, max: f32 },
    IntegerOutOfRange { value: i64, min: i64, max: i64 },
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ahash::HashMap;

    use crate::{BlockState, BlockStateValidationError, BlockStates, PropertyRegistry, PropertyValue};

    #[test]
    fn test_block_states() {
        let mut registry = PropertyRegistry::default();

        registry.add_bool("snowy");
        registry.add_num("dusted").with_range(0, 3);

        let bytes = match fs::read("../../resources/states/grass_block.json") {
            Ok(bytes) => bytes,
            Err(error) => panic!("failed to read block states file: {error}"),
        };

        let states = match BlockStates::from_slice(&bytes, &registry) {
            Ok(states) => states,
            Err(error) => panic!("failed to parse block states: {error}"),
        };

        assert_eq!(states, BlockStates {
            model: String::from("game:models/grass_block"),
            variants: vec![BlockState {
                when: HashMap::from_iter([(String::from("snowy"), PropertyValue::Boolean(true))]),
                model: String::from("game:models/grass_block_snowy")
            }]
        });
    }

    #[test]
    fn test_block_states_2() {
        let mut registry = PropertyRegistry::default();

        registry.add_bool("snowy");
        registry.add_num("dusted").with_range(0, 3);

        let bytes = match fs::read("../../resources/states/grass_block.json") {
            Ok(bytes) => bytes,
            Err(error) => panic!("failed to read block states file: {error}"),
        };

        let states: BlockStates = match serde_json::from_slice(&bytes) {
            Ok(states) => states,
            Err(error) => panic!("failed to parse block states: {error}"),
        };

        for error in registry.validate_states(&states) {
            for error in error.errors {
                match error {
                    BlockStateValidationError::UnknownProperty { name } => println!("unknown property `{name}`"),
                    BlockStateValidationError::UnknownVariant { name, expected } => println!("unknown enum variant, expected `{expected:?}`, found `{name}`"),
                    BlockStateValidationError::UnexpectedType { expected, found } => println!("expected `{expected:?}` property type, found `{found:?}`"),
                    BlockStateValidationError::FloatOutOfRange { value, min, max } => println!("value `{value}` is out of range `{min}..={max}`"),
                    BlockStateValidationError::IntegerOutOfRange { value, min, max } => println!("value `{value}` is out of range `{min}..={max}`"),
                }
            }
        }

        assert_eq!(states, BlockStates {
            model: String::from("game:models/grass_block"),
            variants: vec![BlockState {
                when: HashMap::from_iter([(String::from("snowy"), PropertyValue::Boolean(true))]),
                model: String::from("game:models/grass_block_snowy")
            }]
        });
    }
}
