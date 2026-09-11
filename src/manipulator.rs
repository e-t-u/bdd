use crate::error::BddError;
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

/// Interface for manipulators transforming tuples.
pub trait TupleManipulator {
    fn manipulate(&self, tuple: Vec<Field>) -> Vec<Field>;
}

/// Reorders tuple fields according to a comma-separated list of indices.
pub struct RearrangeManipulator {
    fieldlist: Vec<isize>,
    empty: bool,
}

impl RearrangeManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        if arg.is_empty() {
            return Ok(Self {
                fieldlist: Vec::new(),
                empty: true,
            });
        }
        let parts = arg.split(',');
        let mut fieldlist = Vec::new();
        for p in parts {
            match p.parse::<isize>() {
                Ok(n) => fieldlist.push(n),
                Err(_) => return Err(BddError::RearrangeNonNumber),
            }
        }
        Ok(Self {
            fieldlist,
            empty: false,
        })
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

fn parse_fp(arg: &str, opt_name: &str) -> Result<(isize, BigInt), BddError> {
    let parts: Vec<&str> = arg.splitn(2, ',').collect();
    if parts.len() < 2 {
        return Err(BddError::ManipulatorArgumentError(format!(
            "Argument for {} must be a list of two numbers",
            opt_name
        )));
    }
    let field = parts[0].parse::<isize>().map_err(|_| {
        BddError::ManipulatorArgumentError(format!("Field in {} must be a number", opt_name))
    })?;
    let parameter = parts[1].parse::<BigInt>().map_err(|_| {
        BddError::ManipulatorArgumentError(format!("Parameter in {} must be a number", opt_name))
    })?;
    Ok((field, parameter))
}

/// Clamps field value within `[-maxint, maxint]`.
pub struct CutMaxintManipulator {
    field: isize,
    parameter: BigInt,
}

impl CutMaxintManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--cut-maxint")?;
        Ok(Self { field, parameter })
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

/// Bitwise right shifts field by `bits`.
pub struct RemoveRightManipulator {
    field: isize,
    parameter: usize,
}

impl RemoveRightManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--remove-right")?;
        use num_traits::ToPrimitive;
        let p = parameter.to_usize().unwrap_or(0);
        Ok(Self {
            field,
            parameter: p,
        })
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

/// Bitwise XORs field with a mask of `bits` ones.
pub struct XorManipulator {
    field: isize,
    mask: BigInt,
}

impl XorManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--xor")?;
        use num_traits::ToPrimitive;
        let bits = parameter.to_usize().unwrap_or(0);
        let mask = (BigInt::one() << bits) - BigInt::one();
        Ok(Self { field, mask })
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

/// Replaces field with its absolute value.
pub struct AbsManipulator {
    field: isize,
}

impl AbsManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, _) = parse_fp(arg, "--abs")?;
        Ok(Self { field })
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

/// Replaces field with 1 if negative, 0 otherwise.
pub struct SignManipulator {
    field: isize,
}

impl SignManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, _) = parse_fp(arg, "--sign")?;
        Ok(Self { field })
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

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;

    #[test]
    fn test_rearrange() {
        let manip = RearrangeManipulator::new("1,0").unwrap();
        let input = vec![
            Field::UInt(BigUint::from(10u32)),
            Field::UInt(BigUint::from(20u32)),
        ];
        let output = manip.manipulate(input);
        assert_eq!(
            output,
            vec![
                Field::UInt(BigUint::from(20u32)),
                Field::UInt(BigUint::from(10u32))
            ]
        );

        let manip_neg = RearrangeManipulator::new("-1,0").unwrap();
        let input3 = vec![
            Field::UInt(BigUint::from(1u32)),
            Field::UInt(BigUint::from(9u32)),
            Field::UInt(BigUint::from(2u32)),
        ];
        let output3 = manip_neg.manipulate(input3);
        assert_eq!(
            output3,
            vec![
                Field::UInt(BigUint::from(2u32)),
                Field::UInt(BigUint::from(1u32))
            ]
        );
    }

    #[test]
    fn test_cut_maxint() {
        let manip = CutMaxintManipulator::new("0,50").unwrap();
        let input = vec![Field::Int(BigInt::from(100))];
        let output = manip.manipulate(input);
        assert_eq!(output, vec![Field::UInt(BigUint::from(50u32))]);

        let input_neg = vec![Field::Int(BigInt::from(-100))];
        let output_neg = manip.manipulate(input_neg);
        assert_eq!(output_neg, vec![Field::Int(BigInt::from(-50))]);
    }

    #[test]
    fn test_abs_and_sign() {
        let abs_m = AbsManipulator::new("0,0").unwrap();
        let out = abs_m.manipulate(vec![Field::Int(BigInt::from(-99))]);
        assert_eq!(out, vec![Field::UInt(BigUint::from(99u32))]);

        let sign_m = SignManipulator::new("0,0").unwrap();
        let out_sign = sign_m.manipulate(vec![Field::Int(BigInt::from(-99))]);
        assert_eq!(out_sign, vec![Field::UInt(BigUint::from(1u32))]);
    }
}
