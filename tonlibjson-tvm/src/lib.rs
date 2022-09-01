use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.slice")]
pub struct Slice {
    pub bytes: String
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.cell")]
pub struct Cell {
    pub bytes: String
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.numberDecimal")]
pub struct Number {
    pub number: String
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.tuple")]
pub struct Tuple {
    pub elements: Vec<StackEntry>
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.list")]
pub struct List {
    pub elements: Vec<StackEntry>
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
pub enum StackEntry {
    #[serde(rename = "tvm.stackEntrySlice")]
    Slice { slice: Slice },
    #[serde(rename = "tvm.stackEntryCell")]
    Cell { cell: Cell },
    #[serde(rename = "tvm.stackEntryNumber")]
    Number { number: Number },
    #[serde(rename = "tvm.stackEntryTuple")]
    Tuple { tuple: Tuple },
    #[serde(rename = "tvm.stackEntryList")]
    List { list: List },

    #[serde(rename = "tvm.stackEntryUnsupported")]
    Unsupported
}

#[cfg(test)]
mod tests {
    use crate::{Cell, List, Number, Slice, StackEntry, Tuple};

    #[test]
    fn slice_correct_json() {
        let slice = Slice { bytes: "test".to_string() };

        assert_eq!(serde_json::to_string(&slice).unwrap(), "{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}")
    }

    #[test]
    fn cell_correct_json() {
        let cell = Cell { bytes: "test".to_string() };

        assert_eq!(serde_json::to_string(&cell).unwrap(), "{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}")
    }

    #[test]
    fn number_correct_json() {
        let number = Number { number: "100.2".to_string() };

        assert_eq!(serde_json::to_string(&number).unwrap(), "{\"@type\":\"tvm.numberDecimal\",\"number\":\"100.2\"}")
    }

    #[test]
    fn stack_entry_correct_json() {
        let slice = StackEntry::Slice { slice: Slice { bytes: "test".to_string() }};
        let cell = StackEntry::Cell { cell: Cell { bytes: "test".to_string() }};
        let number = StackEntry::Number { number: Number { number: "123".to_string() }};
        let tuple = StackEntry::Tuple { tuple: Tuple { elements: vec![slice.clone(), cell.clone()]  }};
        let list = StackEntry::List { list: List { elements: vec![slice.clone(), tuple.clone()]  }};

        assert_eq!(serde_json::to_string(&slice).unwrap(), "{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}}");
        assert_eq!(serde_json::to_string(&cell).unwrap(), "{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}");
        assert_eq!(serde_json::to_string(&number).unwrap(), "{\"@type\":\"tvm.stackEntryNumber\",\"number\":{\"@type\":\"tvm.numberDecimal\",\"number\":\"123\"}}");
        assert_eq!(serde_json::to_string(&tuple).unwrap(), "{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}");
        assert_eq!(serde_json::to_string(&list).unwrap(), "{\"@type\":\"tvm.stackEntryList\",\"list\":{\"@type\":\"tvm.list\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}]}}");
    }
}
