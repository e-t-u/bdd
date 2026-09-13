use crate::error::BddError;
use crate::field::Field;
use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

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
                crate::diag::warn(format!("Field {} mentioned in --rearrange missing", f));
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
    crate::field::parse_radix_bigint(trimmed).ok_or_else(|| {
        BddError::ManipulatorArgumentError(format!(
            "Parameter in {} must be a valid number (got '{}')",
            opt_name, s
        ))
    })
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

/// Rounding and overflow handling modes for CutMaxintManipulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutMode {
    /// Saturate / clamp within [-maxint, maxint] (default)
    Saturate,
    /// Wrap around modulo bounds
    Wrap,
    /// Reset value to zero if out of bounds
    Zero,
    /// Drop/discard tuple if value is out of bounds
    Drop,
    /// Truncate magnitude toward zero
    Trunc,
    /// Round toward negative infinity (floor)
    Floor,
    /// Round toward positive infinity (ceil)
    Ceil,
    /// Round to nearest neighbor, ties away from zero (standard round)
    Round,
    /// Round to nearest neighbor, ties to even digit (banker's / IEEE 754 default)
    RoundTiesEven,
}

impl std::str::FromStr for CutMode {
    type Err = BddError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "saturate" | "saturating" | "clamp" | "clamping" => Ok(CutMode::Saturate),
            "wrap" | "wrapping" | "modulo" => Ok(CutMode::Wrap),
            "zero" | "reset" => Ok(CutMode::Zero),
            "drop" | "discard" | "filter" | "checked" => Ok(CutMode::Drop),
            "trunc" | "truncate" | "toward_zero" | "towardzero" | "down" => Ok(CutMode::Trunc),
            "floor" | "toward_negative" | "toward_neg" => Ok(CutMode::Floor),
            "ceil" | "ceiling" | "toward_positive" | "toward_pos" | "up" => Ok(CutMode::Ceil),
            "round" | "half_up" | "half_away" | "nearest" => Ok(CutMode::Round),
            "round_ties_even" | "round_ties_to_even" | "ties_even" | "even" | "banker"
            | "bankers" | "nearest_even" => Ok(CutMode::RoundTiesEven),
            other => Err(BddError::ManipulatorArgumentError(format!(
                "Unknown rounding/cut mode '{}' in --round / --cut-maxint. Supported modes: saturate, wrap, zero, drop, trunc, floor, ceil, round, round_ties_even",
                other
            ))),
        }
    }
}

/// Clamps or rounds field value supporting Rust rounding, precision steps, and overflow modes.
pub struct RoundManipulator {
    field: isize,
    parameter: Option<BigInt>,
    prec_step: Option<f64>,
    mode: CutMode,
}

pub type CutMaxintManipulator = RoundManipulator;

impl RoundManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        // Accepts:
        // FIELD,LIMIT
        // FIELD,LIMIT,MODE
        // FIELD,LIMIT:MODE
        // FIELD,MODE
        // FIELD:MODE
        let (field_str, param_str, mode_str) = if arg.contains(':') {
            let parts: Vec<&str> = arg.splitn(2, ':').collect();
            let left = parts[0];
            let mode = Some(parts[1]);
            if left.contains(',') {
                let fp_parts: Vec<&str> = left.splitn(2, ',').collect();
                (fp_parts[0], Some(fp_parts[1]), mode)
            } else {
                (left, None, mode)
            }
        } else {
            let parts: Vec<&str> = arg.split(',').collect();
            if parts.len() == 2 {
                // Could be FIELD,LIMIT or FIELD,MODE
                if parts[1].parse::<CutMode>().is_ok() {
                    (parts[0], None, Some(parts[1]))
                } else {
                    (parts[0], Some(parts[1]), None)
                }
            } else if parts.len() >= 3 {
                (parts[0], Some(parts[1]), Some(parts[2]))
            } else {
                return Err(BddError::ManipulatorArgumentError(
                    "Argument for --round must be FIELD,LIMIT[,MODE] or FIELD,MODE".to_string(),
                ));
            }
        };

        let field = field_str.trim().parse::<isize>().map_err(|_| {
            BddError::ManipulatorArgumentError("Field in --round must be a number".to_string())
        })?;

        let (parameter, prec_step) = match param_str {
            Some(s) if !s.trim().is_empty() => {
                let trimmed = s.trim();
                if trimmed.contains('.') {
                    let prec = trimmed.parse::<f64>().ok();
                    (None, prec)
                } else {
                    let p = trimmed.parse::<BigInt>().ok().map(|x| x.abs());
                    (p, None)
                }
            }
            _ => (None, None),
        };

        let mode = match mode_str {
            Some(m) if !m.trim().is_empty() => m.parse::<CutMode>()?,
            _ => {
                if prec_step.is_some() {
                    CutMode::Round
                } else {
                    CutMode::Saturate
                }
            }
        };

        Ok(Self {
            field,
            parameter,
            prec_step,
            mode,
        })
    }
}

impl TupleManipulator for RoundManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        let idx = match resolve_index(tuple.len(), self.field) {
            Some(i) => i,
            None => {
                crate::diag::warn(format!("Field {} mentioned in --round missing", self.field));
                return Some(tuple);
            }
        };

        let is_true_rounding = matches!(
            self.mode,
            CutMode::Round | CutMode::RoundTiesEven | CutMode::Floor | CutMode::Ceil | CutMode::Trunc
        );

        if let Field::Float(f) = tuple[idx] {
            let step = self.prec_step.unwrap_or(1.0);
            let rounded = if is_true_rounding && step > 0.0 {
                let q = f / step;
                let rq = match self.mode {
                    CutMode::RoundTiesEven => q.round_ties_even(),
                    CutMode::Round => q.round(),
                    CutMode::Floor => q.floor(),
                    CutMode::Ceil => q.ceil(),
                    CutMode::Trunc => q.trunc(),
                    _ => q,
                };
                rq * step
            } else {
                f
            };

            if let Some(ref max) = self.parameter {
                let max_f = max.to_f64().unwrap_or(f64::MAX);
                let neg_max_f = -max_f;
                if rounded >= neg_max_f && rounded <= max_f {
                    tuple[idx] = Field::Float(rounded);
                    return Some(tuple);
                }
                match self.mode {
                    CutMode::Drop => None,
                    CutMode::Zero => {
                        tuple[idx] = Field::Float(0.0);
                        Some(tuple)
                    }
                    CutMode::Saturate
                    | CutMode::Trunc
                    | CutMode::Round
                    | CutMode::RoundTiesEven
                    | CutMode::Floor => {
                        let val = if rounded > max_f { max_f } else { neg_max_f };
                        tuple[idx] = Field::Float(val);
                        Some(tuple)
                    }
                    CutMode::Ceil => {
                        let val = if rounded < neg_max_f {
                            neg_max_f
                        } else {
                            max_f
                        };
                        tuple[idx] = Field::Float(val);
                        Some(tuple)
                    }
                    CutMode::Wrap => {
                        let span = max_f * 2.0;
                        let shifted = rounded + max_f;
                        let rem = shifted.rem_euclid(span);
                        tuple[idx] = Field::Float(rem - max_f);
                        Some(tuple)
                    }
                }
            } else {
                tuple[idx] = Field::Float(rounded);
                Some(tuple)
            }
        } else {
            // Integer field (UInt or Int)
            let bi = tuple[idx].as_bigint();
            let is_unsigned = matches!(tuple[idx], Field::UInt(_));

            if is_true_rounding {
                if let Some(ref step_bi) = self.parameter {
                    if step_bi > &BigInt::one() {
                        let rem = &bi % step_bi;
                        let base = &bi - &rem;
                        let val = match self.mode {
                            CutMode::Floor => {
                                if rem < BigInt::zero() {
                                    base - step_bi
                                } else {
                                    base
                                }
                            }
                            CutMode::Ceil => {
                                if rem > BigInt::zero() {
                                    base + step_bi
                                } else {
                                    base
                                }
                            }
                            CutMode::Trunc => base,
                            CutMode::Round | CutMode::RoundTiesEven => {
                                let half = step_bi / 2;
                                if rem.abs() >= half {
                                    if bi >= BigInt::zero() {
                                        base + step_bi
                                    } else {
                                        base - step_bi
                                    }
                                } else {
                                    base
                                }
                            }
                            _ => bi,
                        };
                        set_bigint_field(&mut tuple, idx, val);
                        return Some(tuple);
                    }
                }
            }

            if let Some(ref max) = self.parameter {
                let neg_max = -max;
                if bi >= neg_max && bi <= *max {
                    set_bigint_field(&mut tuple, idx, bi);
                    return Some(tuple);
                }
                match self.mode {
                    CutMode::Drop => None,
                    CutMode::Zero => {
                        set_bigint_field(&mut tuple, idx, BigInt::zero());
                        Some(tuple)
                    }
                    CutMode::Saturate
                    | CutMode::Trunc
                    | CutMode::Round
                    | CutMode::RoundTiesEven
                    | CutMode::Floor => {
                        let val = if bi > *max { max.clone() } else { neg_max };
                        set_bigint_field(&mut tuple, idx, val);
                        Some(tuple)
                    }
                    CutMode::Ceil => {
                        let val = if bi < neg_max { neg_max } else { max.clone() };
                        set_bigint_field(&mut tuple, idx, val);
                        Some(tuple)
                    }
                    CutMode::Wrap => {
                        let val = if is_unsigned {
                            let span = max + 1;
                            ((&bi % &span) + &span) % &span
                        } else {
                            let span = max * 2 + 1;
                            let shifted = bi + max;
                            let rem = ((shifted % &span) + &span) % &span;
                            rem - max
                        };
                        set_bigint_field(&mut tuple, idx, val);
                        Some(tuple)
                    }
                }
            } else {
                Some(tuple)
            }
        }
    }
}

/// Clamps field values to [min, max] or [-limit, limit] with overflow modes (saturate, wrap, drop, zero).
pub struct ClampManipulator {
    field: isize,
    min: BigInt,
    max: BigInt,
    min_f: f64,
    max_f: f64,
    mode: CutMode,
}

fn parse_clamp_bound(s: &str) -> Result<(BigInt, f64), BddError> {
    let trimmed = s.trim();
    if let Some(bi) = crate::field::parse_radix_bigint(trimmed) {
        let fl = bi.to_f64().unwrap_or(0.0);
        Ok((bi, fl))
    } else if let Ok(fl) = trimmed.parse::<f64>() {
        let bi = BigInt::from(fl as i64);
        Ok((bi, fl))
    } else {
        Err(BddError::ManipulatorArgumentError(format!(
            "Parameter in --clamp must be a valid number (got '{}')",
            s
        )))
    }
}

impl ClampManipulator {
    pub fn new(arg: &str) -> Result<Self, BddError> {
        let (spec, mode_str) = if let Some(idx) = arg.find(':') {
            (&arg[..idx], Some(&arg[idx + 1..]))
        } else {
            (arg, None)
        };

        let mode = if let Some(m) = mode_str {
            m.parse::<CutMode>()?
        } else {
            CutMode::Saturate
        };

        let parts: Vec<&str> = spec.split(',').collect();
        let field = parts[0].trim().parse::<isize>().map_err(|_| {
            BddError::ManipulatorArgumentError("Field in --clamp must be a number".to_string())
        })?;

        let (min, max, min_f, max_f) = match parts.len() {
            2 => {
                // FIELD,LIMIT
                let (lim_bi, lim_f) = parse_clamp_bound(parts[1])?;
                let abs_bi = lim_bi.abs();
                let abs_f = lim_f.abs();
                (-abs_bi.clone(), abs_bi, -abs_f, abs_f)
            }
            3 => {
                // FIELD,MIN,MAX
                let (min_val, min_f) = parse_clamp_bound(parts[1])?;
                let (max_val, max_f) = parse_clamp_bound(parts[2])?;
                if min_f > max_f {
                    return Err(BddError::ManipulatorArgumentError(
                        "MIN cannot be greater than MAX in --clamp".to_string(),
                    ));
                }
                (min_val, max_val, min_f, max_f)
            }
            _ => {
                return Err(BddError::ManipulatorArgumentError(
                    "Argument for --clamp must be FIELD,LIMIT[:MODE] or FIELD,MIN,MAX[:MODE]".to_string(),
                ));
            }
        };

        Ok(Self {
            field,
            min,
            max,
            min_f,
            max_f,
            mode,
        })
    }
}

impl TupleManipulator for ClampManipulator {
    fn manipulate(&self, mut tuple: Vec<Field>) -> Option<Vec<Field>> {
        let idx = match resolve_index(tuple.len(), self.field) {
            Some(i) => i,
            None => {
                crate::diag::warn(format!("Field {} mentioned in --clamp missing", self.field));
                return Some(tuple);
            }
        };

        if let Field::Float(f) = tuple[idx] {
            let min_f = self.min_f;
            let max_f = self.max_f;
            if f >= min_f && f <= max_f {
                return Some(tuple);
            }
            match self.mode {
                CutMode::Drop => None,
                CutMode::Zero => {
                    tuple[idx] = Field::Float(0.0);
                    Some(tuple)
                }
                CutMode::Saturate | CutMode::Trunc | CutMode::Round | CutMode::RoundTiesEven | CutMode::Floor => {
                    let val = if f > max_f { max_f } else { min_f };
                    tuple[idx] = Field::Float(val);
                    Some(tuple)
                }
                CutMode::Ceil => {
                    let val = if f < min_f { min_f } else { max_f };
                    tuple[idx] = Field::Float(val);
                    Some(tuple)
                }
                CutMode::Wrap => {
                    let span = max_f - min_f;
                    if span > 0.0 {
                        let rem = (f - min_f).rem_euclid(span);
                        tuple[idx] = Field::Float(min_f + rem);
                    }
                    Some(tuple)
                }
            }
        } else {
            let bi = tuple[idx].as_bigint();
            if bi >= self.min && bi <= self.max {
                set_bigint_field(&mut tuple, idx, bi);
                return Some(tuple);
            }
            match self.mode {
                CutMode::Drop => None,
                CutMode::Zero => {
                    set_bigint_field(&mut tuple, idx, BigInt::zero());
                    Some(tuple)
                }
                CutMode::Saturate | CutMode::Trunc | CutMode::Round | CutMode::RoundTiesEven | CutMode::Floor => {
                    let val = if bi > self.max { self.max.clone() } else { self.min.clone() };
                    set_bigint_field(&mut tuple, idx, val);
                    Some(tuple)
                }
                CutMode::Ceil => {
                    let val = if bi < self.min { self.min.clone() } else { self.max.clone() };
                    set_bigint_field(&mut tuple, idx, val);
                    Some(tuple)
                }
                CutMode::Wrap => {
                    let span = &self.max - &self.min + 1u32;
                    if span > BigInt::zero() {
                        let diff = bi - &self.min;
                        let rem = ((diff % &span) + &span) % &span;
                        set_bigint_field(&mut tuple, idx, &self.min + rem);
                    }
                    Some(tuple)
                }
            }
        }
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
            crate::diag::warn(format!(
                "Field {} mentioned in --remove-right missing",
                self.field
            ));
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
        // If not hex/binary and positive, bdd treats it as "number of bits to invert (from right)"
        // If hex/binary or negative, it is the mask directly.
        let (field, parameter) = parse_fp(arg, "--xor")?;
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
            crate::diag::warn(format!("Field {} mentioned in --xor missing", self.field));
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
            crate::diag::warn(format!("Field {} mentioned in --abs missing", self.field));
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
            crate::diag::warn(format!("Field {} mentioned in --sign missing", self.field));
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
                crate::diag::warn(format!("Division by zero in --div on field {}", self.field));
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
            "--clamp" => handle_opt!(ClampManipulator),
            "--round" | "--cut-maxint" => handle_opt!(RoundManipulator),
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

pub fn build_manipulator_from_spec(spec: &str) -> Result<Box<dyn TupleManipulator>, BddError> {
    let s = spec.trim();
    if s.is_empty() {
        return Err(BddError::CliError(
            "Empty manipulator specification".to_string(),
        ));
    }

    let (name, arg) = if let Some(paren_idx) = s.find('(') {
        let op = s[..paren_idx].trim();
        let rest = s[paren_idx + 1..].trim();
        let arg_str = rest.strip_suffix(')').unwrap_or(rest).trim();
        (op, arg_str)
    } else if let Some(colon_idx) = s.find(':') {
        (s[..colon_idx].trim(), s[colon_idx + 1..].trim())
    } else if let Some(eq_idx) = s.find('=') {
        (s[..eq_idx].trim(), s[eq_idx + 1..].trim())
    } else if let Some(sp_idx) = s.find(char::is_whitespace) {
        (s[..sp_idx].trim(), s[sp_idx..].trim())
    } else {
        (s, "")
    };

    let norm_name = name.to_lowercase().replace('_', "-");
    let field_or_single = |val: &str| -> String {
        if !val.contains(',') && !val.is_empty() {
            format!("0,{}", val)
        } else {
            val.to_string()
        }
    };
    let unary_field = |val: &str| -> String {
        if val.is_empty() {
            "0".to_string()
        } else {
            val.to_string()
        }
    };

    match norm_name.as_str() {
        "rearrange" => Ok(Box::new(RearrangeManipulator::new(arg)?)),
        "clamp" => Ok(Box::new(ClampManipulator::new(&field_or_single(arg))?)),
        "round" | "cut-maxint" => Ok(Box::new(RoundManipulator::new(&field_or_single(arg))?)),
        "remove-right" => Ok(Box::new(RemoveRightManipulator::new(&field_or_single(arg))?)),
        "shift-right" => Ok(Box::new(ShiftRightManipulator::new(&field_or_single(arg))?)),
        "shift-left" => Ok(Box::new(ShiftLeftManipulator::new(&field_or_single(arg))?)),
        "xor" => Ok(Box::new(XorManipulator::new(&field_or_single(arg))?)),
        "and" => Ok(Box::new(AndManipulator::new(&field_or_single(arg))?)),
        "or" => Ok(Box::new(OrManipulator::new(&field_or_single(arg))?)),
        "not" => Ok(Box::new(NotManipulator::new(&unary_field(arg))?)),
        "abs" => Ok(Box::new(AbsManipulator::new(&unary_field(arg))?)),
        "sign" => Ok(Box::new(SignManipulator::new(&unary_field(arg))?)),
        "add" => Ok(Box::new(AddManipulator::new(&field_or_single(arg))?)),
        "sub" => Ok(Box::new(SubManipulator::new(&field_or_single(arg))?)),
        "mul" => Ok(Box::new(MulManipulator::new(&field_or_single(arg))?)),
        "div" => Ok(Box::new(DivManipulator::new(&field_or_single(arg))?)),
        "mod" => Ok(Box::new(ModManipulator::new(&field_or_single(arg))?)),
        "filter" => Ok(Box::new(FilterManipulator::new(arg)?)),
        _ => Err(BddError::CliError(format!(
            "Unknown manipulator '{}'",
            spec
        ))),
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

    #[test]
    fn test_cut_maxint_modes() {
        // 1. Default / Saturate
        let sat = CutMaxintManipulator::new("0,10").unwrap();
        let sat_res = sat
            .manipulate(vec![Field::UInt(BigUint::from(15u32))])
            .unwrap();
        assert_eq!(sat_res[0], Field::UInt(BigUint::from(10u32)));

        let sat_neg = sat.manipulate(vec![Field::Int(BigInt::from(-20))]).unwrap();
        assert_eq!(sat_neg[0], Field::Int(BigInt::from(-10)));

        // 2. Wrap mode (unsigned)
        let wrap = CutMaxintManipulator::new("0,10,wrap").unwrap();
        let wrap_u = wrap
            .manipulate(vec![Field::UInt(BigUint::from(11u32))])
            .unwrap();
        // 11 % 11 = 0
        assert_eq!(wrap_u[0], Field::UInt(BigUint::from(0u32)));

        let wrap_u2 = wrap
            .manipulate(vec![Field::UInt(BigUint::from(12u32))])
            .unwrap();
        // 12 % 11 = 1
        assert_eq!(wrap_u2[0], Field::UInt(BigUint::from(1u32)));

        // 3. Zero mode
        let zero = CutMaxintManipulator::new("0,10,zero").unwrap();
        let zero_res = zero
            .manipulate(vec![Field::UInt(BigUint::from(15u32))])
            .unwrap();
        assert_eq!(zero_res[0], Field::UInt(BigUint::from(0u32)));

        let zero_in = zero
            .manipulate(vec![Field::UInt(BigUint::from(7u32))])
            .unwrap();
        assert_eq!(zero_in[0], Field::UInt(BigUint::from(7u32)));

        // 4. Drop mode (checked)
        let drop_m = CutMaxintManipulator::new("0,10,drop").unwrap();
        assert!(drop_m
            .manipulate(vec![Field::UInt(BigUint::from(8u32))])
            .is_some());
        assert!(drop_m
            .manipulate(vec![Field::UInt(BigUint::from(12u32))])
            .is_none());

        // 5. Float rounding modes (preserves float field type)
        let even = RoundManipulator::new("0,100,round_ties_even").unwrap();
        // 2.5 ties to even 2.0
        let r1 = even.manipulate(vec![Field::Float(2.5)]).unwrap();
        assert_eq!(r1[0], Field::Float(2.0));
        // 3.5 ties to even 4.0
        let r2 = even.manipulate(vec![Field::Float(3.5)]).unwrap();
        assert_eq!(r2[0], Field::Float(4.0));

        let floor_m = RoundManipulator::new("0,100,floor").unwrap();
        let rf = floor_m.manipulate(vec![Field::Float(2.9)]).unwrap();
        assert_eq!(rf[0], Field::Float(2.0));

        let ceil_m = RoundManipulator::new("0,100,ceil").unwrap();
        let rc = ceil_m.manipulate(vec![Field::Float(2.1)]).unwrap();
        assert_eq!(rc[0], Field::Float(3.0));

        let trunc_m = RoundManipulator::new("0,100,trunc").unwrap();
        let rt = trunc_m.manipulate(vec![Field::Float(-2.9)]).unwrap();
        assert_eq!(rt[0], Field::Float(-2.0));

        // 6. Round without explicit limit (mode-only)
        let even_unbounded = RoundManipulator::new("0,round_ties_even").unwrap();
        let ru1 = even_unbounded.manipulate(vec![Field::Float(2.5)]).unwrap();
        assert_eq!(ru1[0], Field::Float(2.0));

        let colon_mode = RoundManipulator::new("0:floor").unwrap();
        let ru2 = colon_mode.manipulate(vec![Field::Float(2.9)]).unwrap();
        assert_eq!(ru2[0], Field::Float(2.0));

        // 7. Precision step quantization on floats
        let step_m = RoundManipulator::new("0,0.01:round").unwrap();
        let rs = step_m.manipulate(vec![Field::Float(12.3456)]).unwrap();
        assert_eq!(rs[0], Field::Float(12.35));

        // 8. Step quantization on integers
        let int_step = RoundManipulator::new("0,10:round").unwrap();
        let ris1 = int_step
            .manipulate(vec![Field::Int(BigInt::from(47))])
            .unwrap();
        assert_eq!(ris1[0], Field::UInt(BigUint::from(50u32)));
        let ris2 = int_step
            .manipulate(vec![Field::Int(BigInt::from(44))])
            .unwrap();
        assert_eq!(ris2[0], Field::UInt(BigUint::from(40u32)));
    }

    #[test]
    fn test_clamp_manipulator() {
        // [min, max] range clamping
        let clamp_m = ClampManipulator::new("0,0,255:saturate").unwrap();
        let c1 = clamp_m
            .manipulate(vec![Field::Int(BigInt::from(300))])
            .unwrap();
        assert_eq!(c1[0], Field::UInt(BigUint::from(255u32)));
        let c2 = clamp_m
            .manipulate(vec![Field::Int(BigInt::from(-50))])
            .unwrap();
        assert_eq!(c2[0], Field::UInt(BigUint::zero()));

        // Wrap mode
        let wrap_m = ClampManipulator::new("0,0,10:wrap").unwrap();
        let cw = wrap_m
            .manipulate(vec![Field::Int(BigInt::from(12))])
            .unwrap();
        // 12 wrapped in 0..10 (span 11) is 1
        assert_eq!(cw[0], Field::UInt(BigUint::from(1u32)));

        // Float clamping
        let f_clamp = ClampManipulator::new("0,-1.0,1.0:saturate").unwrap();
        let cf = f_clamp.manipulate(vec![Field::Float(2.5)]).unwrap();
        assert_eq!(cf[0], Field::Float(1.0));
    }
}
