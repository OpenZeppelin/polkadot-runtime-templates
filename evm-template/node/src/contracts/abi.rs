/// This part was easy to implement and may be useful in future.
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum AbiItem {
    #[allow(dead_code)]
    Error(Error),
    #[allow(dead_code)]
    Function(Function),
    Constructor,
    #[allow(dead_code)]
    Event(Event),
    #[allow(dead_code)]
    Receive(Receive),
    #[allow(dead_code)]
    Default(Function),
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Error {
    name: String,
    inputs: Vec<Input>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Receive {
    #[serde(rename = "stateMutability")]
    state_mutability: StateMutability,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Event {
    name: String,
    inputs: Vec<Input>,
    anonymous: bool,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Function {
    name: String,
    inputs: Vec<Input>,
    outputs: Vec<Input>,
    #[serde(rename = "stateMutability")]
    state_mutability: StateMutability,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Input {
    name: String,
    r#type: String,
    components: Option<Vec<Input>>,
    indexed: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StateMutability {
    Pure,
    View,
    Nonpayable,
    Payable,
}
