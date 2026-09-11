use crate::error::BddError;
use crate::field::Field;
use num_bigint::BigInt;
use num_traits::{Num, One, Signed, ToPrimitive, Zero};

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
/// Returns `Some(tuple)` to proceed, or `None` if the tuple should be filtered out.
pub trait TupleManipulator {
    fn manipulate(&self, tuple: Vec<Field>) -> Option<Vec<Field>>;
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
            match p.trim().parse::<isize>() {
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
    fn manipulate(&self, tuple: Vec<Field>) -> Option<Vec<Field>> {
        if self.empty {
            return Some(tuple);
        }
        let mut out = Vec::new();
        for &f in &self.fieldlist {
            if let Some(idx) = resolve_index(tuple.len(), f) {
                out.push(tuple[idx].clone());
            } else {
                eprintln!("Field {} mentioned in --rearrange missing", f);
            }
        }
        Some(out)
    }
}

pub fn parse_bigint_param(s: &str, opt_name: &str) -> Result<BigInt, BddError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(BddError::ManipulatorArgumentError(format!(
            "Parameter in {} cannot be empty",
            opt_name
        )));
    }
    let (sign, rest) = if let Some(stripped) = trimmed.strip_prefix('-') {
        (-1, stripped)
    } else if let Some(stripped) = trimmed.strip_prefix('+') {
        (1, stripped)
    } else {
        (1, trimmed)
    };

    let val = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        BigInt::from_str_radix(hex, 16)
    } else if let Some(bin) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
        BigInt::from_str_radix(bin, 2)
    } else if let Some(oct) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
        BigInt::from_str_radix(oct, 8)
    } else {
        BigInt::from_str_radix(rest, 10)
    };

    match val {
        Ok(v) => Ok(if sign == -1 { -v } else { v }),
        Err(_) => Err(BddError::ManipulatorArgumentError(format!(
            "Parameter in {} must be a valid number (got '{}')",
            opt_name, s
        ))),
    }
}

fn parse_fp(arg: &str, opt_name: &str) -> Result<(isize, BigInt), BddError> {
    let parts: Vec<&str> = arg.splitn(2, ',').collect();
    if parts.len() < 2 {
        return Err(BddError::ManipulatorArgumentError(format!(
            "Argument for {} must be a list of two numbers (field,parameter)",
            opt_name
        )));
    }
    let field = parts[0].trim().parse::<isize>().map_err(|_| {
        BddError::ManipulatorArgumentError(format!("Field in {} must be a number", opt_name))
    })?;
    let parameter = parse_bigint_param(parts[1], opt_name)?;
    Ok((field, parameter))
}

fn parse_unary_field(arg: &str, opt_name: &str) -> Result<isize, BddError> {
    let parts: Vec<&str> = arg.splitn(2, ',').collect();
    parts[0].trim().parse::<isize>().map_err(|_| {
        BddError::ManipulatorArgumentError(format!("Field in {} must be a number", opt_name))
    })
}

fn parse_shift(arg: &str, opt_name: &str) -> Result<(isize, usize), BddError> {
    let (field, parameter) = parse_fp(arg, opt_name)?;
    let p = parameter.to_usize().ok_or_else(|| {
        BddError::ManipulatorArgumentError(format!(
            "Shift amount in {} must be a positive integer",
            opt_name
        ))
    })?;
    Ok((field, p))
}

fn set_bigint_field(tuple: &mut [Field], idx: usize, val: BigInt) {
    tuple[idx] = if val.is_negative() {
        Field::Int(val)
    } else {
        Field::UInt(val.to_biguint().unwrap_or_default())
    };
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
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let mut val = tuple[idx].as_bigint();
            let neg_max = -&self.parameter;
            if val < neg_max {
                val = neg_max;
            } else if val > self.parameter {
                val = self.parameter.clone();
            }
            set_bigint_field(&mut tuple, idx, val);
        } else {
            eprintln!("Field {} mentioned in --cut-maxint missing", self.field);
        }
        Some(tuple)
    }
}

/// Bitwise right shifts field by `bits`.
pub struct RemoveRightManipulator {
    field: isize,
    parameter: usize,
}

impl RemoveRightManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_shift(arg, "--remove-right")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for RemoveRightManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi >> self.parameter;
            set_bigint_field(&mut tuple, idx, val);
        } else {
            eprintln!("Field {} mentioned in --remove-right missing", self.field);
        }
        Some(tuple)
    }
}

/// Bitwise XORs field with a mask of `bits` ones or arbitrary parameter.
pub struct XorManipulator {
    field: isize,
    mask: BigInt,
}

impl XorManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--xor")?;
        // If not hex/binary and positive, bdd treats it as "number of bits to invert (from right)"
        // If hex/binary or negative, it is the mask directly.
        let mask = if !arg.contains("0x") && !arg.contains("0b") && parameter > BigInt::zero() {
            let bits = parameter.to_usize().unwrap_or(0);
            (BigInt::one() << bits) - BigInt::one()
        } else {
            parameter
        };
        Ok(Self { field, mask })
    }
}

impl TupleManipulator for XorManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi ^ &self.mask;
            set_bigint_field(&mut tuple, idx, val);
        } else {
            eprintln!("Field {} mentioned in --xor missing", self.field);
        }
        Some(tuple)
    }
}

/// Replaces field with its absolute value.
pub struct AbsManipulator {
    field: isize,
}

impl AbsManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let field = parse_unary_field(arg, "--abs")?;
        Ok(Self { field })
    }
}

impl TupleManipulator for AbsManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi.abs();
            tuple[idx] = Field::UInt(val.to_biguint().unwrap_or_default());
        } else {
            eprintln!("Field {} mentioned in --abs missing", self.field);
        }
        Some(tuple)
    }
}

/// Replaces field with 1 if negative, 0 otherwise.
pub struct SignManipulator {
    field: isize,
}

impl SignManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let field = parse_unary_field(arg, "--sign")?;
        Ok(Self { field })
    }
}

impl TupleManipulator for SignManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = if bi.is_negative() { 1u32 } else { 0u32 };
            tuple[idx] = Field::UInt(val.into());
        } else {
            eprintln!("Field {} mentioned in --sign missing", self.field);
        }
        Some(tuple)
    }
}

/// Bitwise ANDs field with parameter.
pub struct AndManipulator {
    field: isize,
    parameter: BigInt,
}

impl AndManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--and")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for AndManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi & &self.parameter;
            set_bigint_field(&mut tuple, idx, val);
        }
        Some(tuple)
    }
}

/// Bitwise ORs field with parameter.
pub struct OrManipulator {
    field: isize,
    parameter: BigInt,
}

impl OrManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--or")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for OrManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi | &self.parameter;
            set_bigint_field(&mut tuple, idx, val);
        }
        Some(tuple)
    }
}

/// Bitwise NOT (inverts all bits of field).
pub struct NotManipulator {
    field: isize,
}

impl NotManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let field = parse_unary_field(arg, "--not")?;
        Ok(Self { field })
    }
}

impl TupleManipulator for NotManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = !bi;
            set_bigint_field(&mut tuple, idx, val);
        }
        Some(tuple)
    }
}

/// Bitwise left shifts field by `bits`.
pub struct ShiftLeftManipulator {
    field: isize,
    parameter: usize,
}

impl ShiftLeftManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_shift(arg, "--shift-left")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for ShiftLeftManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi << self.parameter;
            set_bigint_field(&mut tuple, idx, val);
        }
        Some(tuple)
    }
}

/// Bitwise right shifts field by `bits` (synonym for remove-right).
pub struct ShiftRightManipulator {
    inner: RemoveRightManipulator,
}

impl ShiftRightManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        Ok(Self {
            inner: RemoveRightManipulator::new(arg)?,
        })
    }
}

impl TupleManipulator for ShiftRightManipulator {
    fn manipulate(&self, tuple: Vec<Field>) -> Option<Vec<Field>> {
        self.inner.manipulate(tuple)
    }
}

/// Adds parameter to field.
pub struct AddManipulator {
    field: isize,
    parameter: BigInt,
}

impl AddManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--add")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for AddManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi + &self.parameter;
            set_bigint_field(&mut tuple, idx, val);
        }
        Some(tuple)
    }
}

/// Subtracts parameter from field.
pub struct SubManipulator {
    field: isize,
    parameter: BigInt,
}

impl SubManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--sub")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for SubManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi - &self.parameter;
            set_bigint_field(&mut tuple, idx, val);
        }
        Some(tuple)
    }
}

/// Multiplies field by parameter.
pub struct MulManipulator {
    field: isize,
    parameter: BigInt,
}

impl MulManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--mul")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for MulManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let val = bi * &self.parameter;
            set_bigint_field(&mut tuple, idx, val);
        }
        Some(tuple)
    }
}

/// Integer division of field by parameter.
pub struct DivManipulator {
    field: isize,
    parameter: BigInt,
}

impl DivManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--div")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for DivManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            if self.parameter.is_zero() {
                eprintln!("Division by zero in --div on field {}", self.field);
                set_bigint_field(&mut tuple, idx, BigInt::zero());
            } else {
                let val = bi / &self.parameter;
                set_bigint_field(&mut tuple, idx, val);
            }
        }
        Some(tuple)
    }
}

/// Modulo of field by parameter.
pub struct ModManipulator {
    field: isize,
    parameter: BigInt,
}

impl ModManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (field, parameter) = parse_fp(arg, "--mod")?;
        Ok(Self { field, parameter })
    }
}

impl TupleManipulator for ModManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            if self.parameter.is_zero() {
                set_bigint_field(&mut tuple, idx, BigInt::zero());
            } else {
                let val = bi % &self.parameter;
                set_bigint_field(&mut tuple, idx, val);
            }
        }
        Some(tuple)
    }
}

/// Filter operator for conditional testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Conditional filter dropping tuples where predicate evaluates to false.
pub struct FilterManipulator {
    field: isize,
    op: FilterOp,
    val: BigInt,
}

impl FilterManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        // Syntax: field,op,val (e.g. "0,==,0xFF" or "0,!=,0" or "0,>=,10")
        let parts: Vec<&str> = arg.splitn(3, ',').collect();
        if parts.len() == 3 {
            let field = parts[0].trim().parse::<isize>().map_err(|_| {
                BddError::ManipulatorArgumentError("Field in --filter must be a number".to_string())
            })?;
            let op = match parts[1].trim() {
                "==" | "=" => FilterOp::Eq,
                "!=" => FilterOp::Ne,
                "<" => FilterOp::Lt,
                "<=" => FilterOp::Le,
                ">" => FilterOp::Gt,
                ">=" => FilterOp::Ge,
                other => {
                    return Err(BddError::ManipulatorArgumentError(format!(
                        "Unknown operator '{}' in --filter (expected ==, !=, <, <=, >, >=)",
                        other
                    )));
                }
            };
            let val = parse_bigint_param(parts[2], "--filter")?;
            Ok(Self { field, op, val })
        } else {
            Err(BddError::ManipulatorArgumentError(
                "Filter syntax must be: --filter <field>,<op>,<value> (e.g. 0,==,0xFF)".to_string(),
            ))
        }
    }
}

impl TupleManipulator for FilterManipulator {
    fn manipulate(&self, tuple: Vec<Field>) -> Option<Vec<Field>> {
        if let Some(idx) = resolve_index(tuple.len(), self.field) {
            let bi = tuple[idx].as_bigint();
            let matches = match self.op {
                FilterOp::Eq => bi == self.val,
                FilterOp::Ne => bi != self.val,
                FilterOp::Lt => bi < self.val,
                FilterOp::Le => bi <= self.val,
                FilterOp::Gt => bi > self.val,
                FilterOp::Ge => bi >= self.val,
            };
            if matches {
                Some(tuple)
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Inspects command-line arguments to construct an ordered manipulation pipeline.
pub fn build_pipeline_from_args(
    args: &[String],
) -> Result<Vec<Box<dyn TupleManipulator>>, BddError> {
    let mut pipeline: Vec<Box<dyn TupleManipulator>> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let (opt, val) = if let Some(eq_idx) = arg.find('=') {
            (&arg[..eq_idx], Some(&arg[eq_idx + 1..]))
        } else {
            (arg.as_str(), None)
        };

        macro_rules! handle_opt {
            ($manip_ty:ident) => {{
                let val_str = if let Some(v) = val {
                    v.to_string()
                } else if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    i += 1;
                    args[i].clone()
                } else {
                    String::new()
                };
                pipeline.push(Box::new($manip_ty::new(&val_str)?));
            }};
        }

        match opt {
            "--rearrange" => handle_opt!(RearrangeManipulator),
            "--cut-maxint" => handle_opt!(CutMaxintManipulator),
            "--remove-right" => handle_opt!(RemoveRightManipulator),
            "--shift-right" => handle_opt!(ShiftRightManipulator),
            "--shift-left" => handle_opt!(ShiftLeftManipulator),
            "--xor" => handle_opt!(XorManipulator),
            "--and" => handle_opt!(AndManipulator),
            "--or" => handle_opt!(OrManipulator),
            "--not" => handle_opt!(NotManipulator),
            "--abs" => handle_opt!(AbsManipulator),
            "--sign" => handle_opt!(SignManipulator),
            "--add" => handle_opt!(AddManipulator),
            "--sub" => handle_opt!(SubManipulator),
            "--mul" => handle_opt!(MulManipulator),
            "--div" => handle_opt!(DivManipulator),
            "--mod" => handle_opt!(ModManipulator),
            "--filter" => handle_opt!(FilterManipulator),
            _ => {}
        }
        i += 1;
    }
    Ok(pipeline)
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
        let output = manip.manipulate(input).unwrap();
        assert_eq!(
            output,
            vec![
                Field::UInt(BigUint::from(20u32)),
                Field::UInt(BigUint::from(10u32)),
            ]
        );
    }

    #[test]
    fn test_arithmetic_pipeline() {
        let add = AddManipulator::new("0,5").unwrap();
        let mul = MulManipulator::new("0,3").unwrap();
        let sub = SubManipulator::new("0,2").unwrap();

        let mut t = vec![Field::UInt(BigUint::from(10u32))];
        t = add.manipulate(t).unwrap(); // 15
        t = mul.manipulate(t).unwrap(); // 45
        t = sub.manipulate(t).unwrap(); // 43

        assert_eq!(t[0], Field::UInt(BigUint::from(43u32)));
    }

    #[test]
    fn test_bitwise_pipeline() {
        let and_m = AndManipulator::new("0,0xF0").unwrap();
        let or_m = OrManipulator::new("0,0x05").unwrap();
        let shl_m = ShiftLeftManipulator::new("0,2").unwrap();

        let mut t = vec![Field::UInt(BigUint::from(0xABu32))];
        t = and_m.manipulate(t).unwrap(); // 0xA0
        t = or_m.manipulate(t).unwrap(); // 0xA5
        t = shl_m.manipulate(t).unwrap(); // 0xA5 << 2 = 0x294

        assert_eq!(t[0], Field::UInt(BigUint::from(0x294u32)));
    }

    #[test]
    fn test_filter_manipulator() {
        let filt = FilterManipulator::new("0,==,42").unwrap();
        let t1 = vec![Field::UInt(BigUint::from(42u32))];
        let t2 = vec![Field::UInt(BigUint::from(43u32))];

        assert!(filt.manipulate(t1).is_some());
        assert!(filt.manipulate(t2).is_none());
    }
}
