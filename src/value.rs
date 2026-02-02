use std::{fmt::Display, str::FromStr};

/// SQL value.
///
/// # Display and Parse Format
///
/// - Null: `null`
/// - Bool: `false`
/// - Integer: `1`
/// - String: `'string'`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i32),
    String(String),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::String(s) => write!(f, "'{s}'"),
        }
    }
}

impl FromStr for Value {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "null" {
            return Ok(Value::Null);
        } else if let Ok(i) = s.parse() {
            return Ok(Value::Bool(i));
        } else if let Ok(i) = s.parse() {
            return Ok(Value::Int(i));
        } else if s.starts_with('\'') && s.ends_with('\'') {
            return Ok(Value::String(s[1..s.len() - 1].to_string()));
        }
        Err(s.to_string())
    }
}

impl Value {
    pub fn is_zero(&self) -> bool {
        matches!(self, Value::Int(0))
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl Value {
    pub fn and(&self, other: &Self) -> Self {
        use Value::*;
        match (self, other) {
            (&Bool(a), &Bool(b)) => Bool(a && b),
            (&Bool(false), _) | (_, &Bool(false)) => Bool(false),
            (Null, _) | (_, Null) => Null,
            _ => panic!("unsupported AND between {:?} and {:?}", self, other),
        }
    }
    pub fn or(&self, other: &Self) -> Self {
        use Value::*;
        match (self, other) {
            (&Bool(a), &Bool(b)) => Bool(a || b),
            (&Bool(true), _) | (_, &Bool(true)) => Bool(true),
            (Null, _) | (_, Null) => Null,
            _ => panic!("unsupported OR between {:?} and {:?}", self, other),
        }
    }
    pub fn xor(&self, other: &Self) -> Self {
        use Value::*;
        match (self, other) {
            (&Bool(a), &Bool(b)) => Bool(a ^ b),
            (Null, _) | (_, Null) => Null,
            _ => panic!("unsupported XOR between {:?} and {:?}", self, other),
        }
    }
}

impl std::ops::Not for Value {
    type Output = Value;
    fn not(self) -> Self::Output {
        use Value::*;
        match self {
            Bool(b) => Bool(!b),
            Null => Null,
            _ => panic!("unsupported negation for {:?}", self),
        }
    }
}

impl std::ops::Neg for Value {
    type Output = Value;
    fn neg(self) -> Self::Output {
        use Value::*;
        match self {
            Int(i) => Int(-i),
            Null => Null,
            _ => panic!("unsupported negation for {:?}", self),
        }
    }
}

macro_rules! impl_arith_for_value {
    ($Trait:ident, $fn:ident) => {
        // impl for references
        impl std::ops::$Trait for &Value {
            type Output = Value;
            fn $fn(self, other: Self) -> Self::Output {
                use Value::*;
                match (self, other) {
                    (Int(a), Int(b)) => Int(a.$fn(*b)),
                    (Null, _) | (_, Null) => Null,
                    _ => panic!("unsupported {} between {:?} and {:?}", stringify!($Trait), self, other),
                }
            }
        }

        // impl for owned
        impl std::ops::$Trait for Value {
            type Output = Value;
            fn $fn(self, other: Self) -> Self::Output {
                (&self).$fn(&other) // delegate to reference impl
            }
        }
    };
}

impl_arith_for_value!(Add, add);
impl_arith_for_value!(Sub, sub);
impl_arith_for_value!(Mul, mul);
impl_arith_for_value!(Div, div);
impl_arith_for_value!(Rem, rem);

pub type Column = egg::Symbol;
