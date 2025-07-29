use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockStates {
    pub model: String,
    pub variants: Vec<BlockState>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(untagged)]
pub enum PropertyValue<'a> {
    Number(i64),
    Float(f32),
    String(&'a str),
    Boolean(bool),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Property<'a> {
    pub name: &'static str,
    pub value: PropertyValue<'a>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(untagged)]
pub enum ConditionValue {
    Number(i64),
    Float(f32),
    String(String),
    Boolean(bool),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum BlockCondition {
    Equals { target: String, value: ConditionValue },
}

impl BlockCondition {
    pub fn test(&self, other: Property) -> bool {
        match self {
            Self::Equals { target, value } => {
                target == other.name
                    && match (value, other.value) {
                        (ConditionValue::Number(a), PropertyValue::Number(b)) => a == &b,
                        (ConditionValue::Float(a), PropertyValue::Float(b)) => (a - b).abs() < f32::EPSILON,
                        (ConditionValue::String(a), PropertyValue::String(b)) => a == b,
                        (ConditionValue::Boolean(a), PropertyValue::Boolean(b)) => a == &b,
                        _ => false,
                    }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockState {
    pub conditions: Vec<BlockCondition>,
    pub model: String,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::BlockStates;

    #[test]
    fn test_block_states() {
        if let Some(states) = fs::read("../app/resources/states/grass_block.json")
            .ok()
            .map(|data| serde_json::from_slice::<BlockStates>(&data).unwrap())
        {
            println!("{states:#?}");
        }
    }
}
