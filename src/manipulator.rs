use crate::field::Field;
use num_bigint::BigInt;
use num_traits::{One, Signed};

fn resolve_index(len: usize, idx: isize) -> Option<usize> {
    if idx >= 0 {
        let u = idx as usize;
        if u < len {
            Some(u)
        } else {
            None
        }
    } else {
        let neg = (-idx) as usize;
        if neg <= len {
            Some(len - neg)
        } else {
            None
        }
    }
}

pub trait TupleManipulator {
    fn manipulate(&self, tuple: Vec<Field>) -> Vec<Field>;
}

pub struct RearrangeManipulator {
    fieldlist: Vec<isize>,
    empty: bool,
}

impl RearrangeManipulator {
    pub fn new(arg: &str) -> Self {
        if arg.is_empty() {
            return Self {
                fieldlist: Vec::new(),
                empty: true,
            };
        }
        let parts = arg.split(',');
        let mut fieldlist = Vec::new();
        for p in parts {
            match p.parse::<isize>() {
                Ok(n) => fieldlist.push(n),
                Err(_) => {
                    eprintln!("Fields in --rearrange must be numbers");
                    std::process::exit(1);
                }
            }
        }
        Self {
            fieldlist,
            empty: false,
        }
    }
}

impl TupleManipulator for RearrangeManipulator {
    fn manipulate(&self, tuple: Vec<Field>) -> Vec<Field> {
        if self.empty {
            return tuple;
        }
        let mut out = Vec::new();
        for &f in &self.fieldlist {
            if let Some(idx) = resolve_index(tuple.len(), f) {
                out.push(tuple[idx].clone());
            } else {
                eprintln!("Field {} mentioned in --rearrange missing", f);
            }
        }
        out
    }
}

fn parse_fp(arg: &str, opt_name: &str) -> (isize, BigInt) {
    let parts: Vec<&str> = arg.splitn(2, ',').collect();
    if parts.len() < 2 {
        eprintln!("Argument for {} must be a list of two numbers", opt_name);
        std::process::exit(1);
    }
    let field = match parts[0].parse::<isize>() {
        Ok(f) => f,
        Err(_) => {
            eprintln!("Field in {} must be a number", opt_name);
            std::process::exit(1);
        }
    };
    let parameter = match parts[1].parse::<BigInt>() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Parameter in {} must be a number", opt_name);
            std::process::exit(1);
        }
    };
    (field, parameter)
}

pub struct CutMaxintManipulator {
    field: isize,
    parameter: BigInt,
}

impl CutMaxintManipulator {
    pub fn new(arg: &str) -> Self {
        let (field, parameter) = parse_fp(arg, "--cut-maxint");
        Self { field, parameter }
    }
}

impl TupleManipulator for CutMaxintManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Vec<Field> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let mut val = tuple[idx].as_bigint();
            let neg_max = -&self.parameter;
            if val < neg_max {
                val = neg_max;
            } else if val > self.parameter {
                val = self.parameter.clone();
            }
            tuple[idx] = if val.is_negative() {
                Field::Int(val)
            } else {
                Field::UInt(val.to_biguint().unwrap_or_default())
            };
        } else {
            eprintln!("Field {} mentioned in --cut-maxint missing", self.field);
        }
        tuple
    }
}

pub struct RemoveRightManipulator {
    field: isize,
    parameter: usize,
}

impl RemoveRightManipulator {
    pub fn new(arg: &str) -> Self {
        let (field, parameter) = parse_fp(arg, "--remove-right");
        use num_traits::ToPrimitive;
        let p = parameter.to_usize().unwrap_or(0);
        Self { field, parameter: p }
    }
}

impl TupleManipulator for RemoveRightManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Vec<Field> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi >> self.parameter;
            tuple[idx] = if val.is_negative() {
                Field::Int(val)
            } else {
                Field::UInt(val.to_biguint().unwrap_or_default())
            };
        } else {
            eprintln!("Field {} mentioned in --remove-right missing", self.field);
        }
        tuple
    }
}

pub struct XorManipulator {
    field: isize,
    mask: BigInt,
}

impl XorManipulator {
    pub fn new(arg: &str) -> Self {
        let (field, parameter) = parse_fp(arg, "--xor");
        use num_traits::ToPrimitive;
        let bits = parameter.to_usize().unwrap_or(0);
        let mask = (BigInt::one() << bits) - BigInt::one();
        Self { field, mask }
    }
}

impl TupleManipulator for XorManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Vec<Field> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi ^ &self.mask;
            tuple[idx] = if val.is_negative() {
                Field::Int(val)
            } else {
                Field::UInt(val.to_biguint().unwrap_or_default())
            };
        } else {
            eprintln!("Field {} mentioned in --xor missing", self.field);
        }
        tuple
    }
}

pub struct AbsManipulator {
    field: isize,
}

impl AbsManipulator {
    pub fn new(arg: &str) -> Self {
        let (field, _) = parse_fp(arg, "--abs");
        Self { field }
    }
}

impl TupleManipulator for AbsManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Vec<Field> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi.abs();
            tuple[idx] = Field::UInt(val.to_biguint().unwrap_or_default());
        } else {
            eprintln!("Field {} mentioned in --abs missing", self.field);
        }
        tuple
    }
}

pub struct SignManipulator {
    field: isize,
}

impl SignManipulator {
    pub fn new(arg: &str) -> Self {
        let (field, _) = parse_fp(arg, "--sign");
        Self { field }
    }
}

impl TupleManipulator for SignManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Vec<Field> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = if bi.is_negative() { 1u32 } else { 0u32 };
            tuple[idx] = Field::UInt(val.into());
        } else {
            eprintln!("Field {} mentioned in --sign missing", self.field);
        }
        tuple
    }
}
