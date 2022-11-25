use anyhow::anyhow;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use tonlibjson_tokio::block::{Cell, Number, Slice, SmcStack, StackEntry, Tuple};

#[derive(Deserialize, Debug)]
pub struct RunGetMethodParams {
    pub address: String,
    pub method: String,
    pub stack: StackWrapped
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CellType {
    #[serde(rename = "num", alias = "number", alias = "int", alias = "tvm.numberDecimal")]
    Number,
    #[serde(rename = "cell", alias = "tvm.Cell", alias = "tvm.cell")]
    Cell,
    #[serde(rename = "slice", alias = "tvm.Slice", alias = "tvm.slice")]
    Slice,
    #[serde(rename = "list", alias = "tvm.List", alias = "tvm.list")]
    List,
    #[serde(rename = "tuple", alias = "tvm.Tuple", alias = "tvm.tuple")]
    Tuple,
}

type StackElement = (CellType, Value);

#[derive(Debug, Serialize, Deserialize)]
pub struct Stack(Vec<StackElement>);

impl TryInto<SmcStack> for Stack {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<SmcStack, Self::Error> {
        self.0.iter().map(|(t, val)| {
            match (t, val) {
                (CellType::Slice, Value::String(val)) => Ok(StackEntry::Slice { slice: Slice { bytes: val.to_owned() } }),
                (CellType::Cell, Value::String(val)) => Ok(StackEntry::Cell { cell: Cell { bytes: val.to_owned() } }),
                (CellType::Number, Value::String(val)) => Ok(StackEntry::Number { number: Number { number: val.to_owned() } }),
                (CellType::Number, Value::Number(val)) => Ok(StackEntry::Number { number: Number { number: val.to_string() } }),
                _ => Err(anyhow!("Unsupported stack element type"))
            }
        }).collect()
    }
}

impl TryFrom<SmcStack> for Stack {
    type Error = anyhow::Error;

    fn try_from(value: SmcStack) -> Result<Self, Self::Error> {
        let elements: Result<Vec<StackElement>, Self::Error> = value.iter().map(|e| {
            match e {
                StackEntry::Number { number: val} =>  Ok((CellType::Number, json!(format!("0x{:x}", &val.number.parse::<i64>()?)))),
                StackEntry::Slice { slice: val} =>  Ok((CellType::Cell, json!({
                    "bytes": val.bytes.clone()
                }))),
                StackEntry::Cell { cell: val } => Ok((CellType::Cell, json!({
                    "bytes": val.bytes.clone()
                }))),
                StackEntry::Tuple { tuple: val} => Ok((CellType::Tuple, json!(val))),
                StackEntry::List { list: val} => Ok((CellType::List, json!(val))),
                _ => Err(anyhow!("Unsupported stack element type"))
            }
        }).collect();

        Ok(Self { 0: elements? })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StackWrapped {
    Normalized(SmcStack),
    Shitty(Stack)
}

impl TryInto<SmcStack> for StackWrapped {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<SmcStack, Self::Error> {
        match self {
            Self::Normalized(s) => Ok(s),
            Self::Shitty(s) => s.try_into()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::params::{CellType, Stack, StackElement};

    #[test]
    pub fn serialize_cell_type() {
        let cell = CellType::Cell;

        assert_eq!("\"cell\"", serde_json::to_string(&cell).unwrap());
    }

    #[test]
    pub fn parse_stack() {
        let input = json!([["int",0],["tvm.cell","te6cckEBAQEAAgAAAEysuc0="]]);

        let stack = serde_json::from_value::<Stack>(input).unwrap();

        assert_eq!(stack.0.len(), 2)
    }
}
