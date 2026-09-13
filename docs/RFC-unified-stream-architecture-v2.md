# RFC: Unified Stream Architecture v2 (Linear Pipeline Unification & Ambiguity Analysis)

- **Status**: Proposed / Architecture Specification
- **Target Version**: `bdd` v0.6.0
- **Author**: Esa Turtiainen & DeepMind Advanced Agentic Coding Team
- **Date**: September 2026
- **Supercedes / Extends**: [`RFC-stream-arrows-unification.md`](RFC-stream-arrows-unification.md)

---

## 1. Executive Summary

In current `bdd` (v0.5), executing a bitstream pipeline often requires coordinating across three distinct syntactic mechanisms:
1. **CLI Flags**: `--input-ones`, `--input-zeros`, `--output-hex`, `--output-json`.
2. **Stream Arrow Expression**: Positional container/unit sizing (`"8 -> 16"` or `"188B[11:13] -> 13"`).
3. **Pattern Expressions**: Positional or flagged tuple unpacker and packer strings (`4U4U`, `16U`).

This separation creates cognitive friction and redundancy. If we unify the entire pipeline into a single linear arrow expression:

$$\text{\texttt{"ones -> 8 -> 4U4U -> 16U -> 16 -> hex"}}$$

a fundamental architectural question arises:
> **Are the intermediate unit sizes (`8` and `16`) necessary, or should they be omitted because their bit widths are already known from the patterns (`4U4U` = 8 bits, `16U` = 16 bits)? If omitted (`"ones -> 4U4U -> 16U -> hex"`), does upstream stages inferring information from downstream stages cause confusion or ambiguity?**

This RFC presents the **Unified Stream Architecture v2**, establishes the formal **Dataflow Type System** governing stream stages, answers the downstream inference question, and provides an **exhaustive ambiguity analysis** with deterministic resolution rules.

---

## 2. The Core Dilemma: Redundancy vs. Downstream Inference

### 2.1 The Redundancy of `8` and `16`

In the expression:
```
ones -> 8 -> 4U4U -> 16U -> 16 -> hex
```
- `ones`: Infinite bit fountain emitting binary `1`s.
- `8`: Bit slicer requesting 8-bit blocks.
- `4U4U`: Tuple unpacker dividing an 8-bit block into two 4-bit fields `(f0: 4U, f1: 4U)`. Total width: $4 + 4 = 8$ bits.
- `16U`: Tuple packer assembling fields into a 16-bit word. Total width: $16$ bits.
- `16`: Output unit framer declaring 16-bit output blocks.
- `hex`: Output formatter rendering 16-bit units as 4-digit hexadecimal words.

Here, `8` and `16` are **100% mathematically and structurally redundant**:
- `4U4U` cannot unpack anything other than 8 bits.
- `16U` cannot emit anything other than 16 bits.
- Specifying `8` immediately before `4U4U` is identical to writing `let x: u8 = (5: u8)`.

### 2.2 Is Downstream Inference Reasonable or Confusing?

The user asks:
> *"Is this reasonable because previous entries in the stream get information from later entries what they assume? It may be confusing very easily."*

#### Data Plane vs. Control Plane (The "Measuring Cup" Analogy)

In Unix pipelines (`cat | grep | cut | sort`), data strictly flows left-to-right. However, in stream processing, we must distinguish between:
1. **The Data Plane (Information Flow)**: Raw bits strictly flow **left-to-right**:
   $$\text{ones} \xrightarrow{\text{bits}} \text{4U4U} \xrightarrow{\text{tuple}} \text{16U} \xrightarrow{\text{bits}} \text{hex}$$
2. **The Control Plane (Constraint / Schema Propagation)**:
   - A bit source like `ones`, `zeros`, `rand`, or `file('stream.bin')` is an **unstructured bit fountain**. It has no concept of a "unit" or "record".
   - It is not that `ones` "looks ahead into the future" to guess what `4U4U` wants.
   - Rather, **`4U4U` is the extractor (the measuring cup)** attached to the fountain. The fountain merely supplies raw bits whenever the extractor pulls them.

#### When Downstream Inference is Intuitive
When reading `"ones -> 4U4U -> 16U -> hex"`, the developer's mental model is:
> *"Take the `ones` stream, unpack it as `4U4U`, repack it as `16U`, and print as `hex`."*

This is completely natural and requires no mental back-tracking.

#### When Downstream Inference Becomes Confusing
Confusion occurs **only when there is a mismatch or missing information**:
1. **Conflicting Declarations**:
   $$\text{\texttt{"ones -> 12 -> 4U4U -> hex"}}$$
   The user explicitly wrote `12`, but `4U4U` requires 8 bits!
   - Did the user want a 12-bit container with 4 trailing bits discarded?
   - Or was `12` a typo?
   - *If `bdd` silently lets `4U4U` override `12`*, the user is confused.
2. **Missing Unit Dimensions**:
   $$\text{\texttt{"ones -> hex"}}$$
   There is no pattern here! How many bits should `hex` print per word?
   - 8 bits (`FF`)?
   - 16 bits (`FFFF`)?
   - 32 bits (`FFFFFFFF`)?
   - If the system silently assumes 8 bits, the user who wanted 32 bits is confused.

---

## 3. The Formal Pipeline Type System

To eliminate all ambiguity, `bdd` v0.6 models pipeline stages as a **strongly-typed finite state machine**.

### 3.1 The Three Pipeline Data Types

At any point between two stages `A -> B`, the in-flight data is in exactly one of three states:

```
┌─────────────────┐       ┌─────────────────┐       ┌─────────────────┐
│   Bitstream     │       │   BitUnit(N)    │       │     Tuple       │
│  (continuous)   │       │  (N-bit block)  │       │ (typed fields)  │
└─────────────────┘       └─────────────────┘       └─────────────────┘
```

1. **`Bitstream`**: Continuous, unfragmented stream of bits without boundaries (from files, sockets, or synthetic generators).
2. **`BitUnit(N)`**: Discrete physical block of exactly $N$ bits (produced by slicers or container extractors).
3. **`Tuple`**: Ordered sequence of strongly-typed field values `(Field0, Field1, ...)` (produced by unpackers or text input).

### 3.2 Stage Transition Matrix

Every stage in an arrow pipeline belongs to one of seven functional categories:

| Category | Input State | Output State | Examples | Grammar Rules |
|---|---|---|---|---|
| **Source** | `None` | `Bitstream` | `ones`, `zeros`, `rand`, `counter`, `netlink`, `file('in')`, `stdin` | Allowed only as Stage 0 |
| **Slicer** | `Bitstream` | `BitUnit(N)` | `8`, `16`, `188B[11:13]`, `8[2:4]`, `[2:4:2]` | Takes raw bits, frames into $N$-bit units |
| **Unpacker** | `BitUnit(N)` or `Bitstream` | `Tuple` | `4U4U`, `16H`, `sync:11u,ver:2u`, `ipv4-header` | Splits bits into tuple fields. If input is `Bitstream`, auto-slices $\sum \text{bits}$ |
| **Manipulator** | `Tuple` | `Tuple` | `add(10)`, `xor(0xFF)`, `clamp(0,255)`, `filter(w==2)`, `rearrange(1,0)` | In-flight field arithmetic and filtering |
| **Packer** | `Tuple` | `BitUnit(M)` | `16U`, `8E`, `8U,8U`, `4*8B` | Assembles tuple fields into an $M$-bit unit |
| **Framer** | `BitUnit(M)` | `BitUnit(K)` or `Bitstream` | `188B[11:13]`, `5B : 8[2:4] + 8`, `16` | Embeds unit into container frame or adds gaps |
| **Sink** | `Tuple` or `BitUnit(M)` | `Terminal` | `json`, `csv`, `tuples`, `hex`, `bits`, `raw`, `file('out')` | Formats data or writes to destination |

---

## 4. Exhaustive Ambiguity Catalog & Resolutions

We have carefully evaluated all possible syntactic, semantic, and dimensional ambiguities:

### Ambiguity 1: Bare Unit Number vs. Pattern
- **Problem**: Could `16` be confused with a pattern?
- **Analysis**:
  - In `bdd` pattern grammar, every field requires a type letter (`U, u, S, s, M, m, F, f, D, d, H, h, Y, y, E, e, Q, q, C, c, K, k, x, X, z, o, r`).
  - `16` has no type letter. It is strictly a **Unit / Container Sizer**.
  - `16U` has type letter `U`. It is strictly a **Pattern Specifier**.
- **Resolution**: **Zero Lexical Ambiguity.** `16` $\to$ Slicer/Framer. `16U` $\to$ Pattern.

---

### Ambiguity 2: Unpacker vs. Packer in the Same Arrow Chain
- **Problem**: In `"ones -> 4U4U -> 16U -> hex"`, how does the engine know `4U4U` is an *unpacker* while `16U` is a *packer*?
- **Analysis**:
  - `4U4U` receives a `Bitstream` (or `BitUnit`). The only valid transition from `Bitstream` via a pattern is **Unpacking** (`Bitstream -> Tuple`).
  - `16U` receives a `Tuple`. The only valid transition from `Tuple` via a pattern is **Packing** (`Tuple -> BitUnit`).
  - If a user specifies two patterns in sequence with no intermediate sink:
    $$\text{Pattern}_1 \to \text{Unpacker (Bitstream $\to$ Tuple)}$$
    $$\text{Pattern}_2 \to \text{Packer (Tuple $\to$ BitUnit)}$$
- **Resolution**: **Completely Deterministic via Pipeline Type State.** The first pattern is the unpacker; the pattern following manipulators is the packer.

---

### Ambiguity 3: Dimensional Conflict Between Slicer and Pattern
- **Problem**: What if a user writes `"ones -> 12 -> 4U4U -> hex"`?
  The Slicer declares `12` bits, but the Unpacker `4U4U` consumes $4 + 4 = 8$ bits.
- **Analysis**:
  - Silently ignoring `12` violates the principle of least astonishment.
  - Silently truncating 4 bits without an explicit discard rule is hazardous in binary protocols.
- **Resolution**: **Strict Dimension Checking Error.**
  If an explicit unit slicer precedes a pattern, its bit width **MUST equal** the pattern's total bit width:
  ```text
  [bdd] Error: Pipeline dimension mismatch in stage '12 -> 4U4U':
        Slicer specifies 12 bits, but pattern '4U4U' requires 8 bits.
        To discard 4 bits, use pattern '4U4U4x' or container slice '12[0:8]'.
  ```
  If the unit size matches (`8 -> 4U4U`), it is accepted as an explicit assertion.

---

### Ambiguity 4: Unit Sinks Consuming Unpacked Tuples Directly
- **Problem**: What if a user writes `"ones -> 4U4U -> hex"` without an explicit packer like `8U` or `16U`?
  - `4U4U` produces a `Tuple(f0, f1)`.
  - `hex` is a **Unit Sink** (`BitUnit -> Terminal`), not a Tuple Sink.
- **Analysis**:
  - Should `bdd` produce an error (`"Cannot pass tuple to hex sink without packing"`)?
  - Or should `bdd` auto-pack the tuple using its natural field layout?
- **Resolution**: **Auto-Packing for Unit Sinks.**
  When a `Tuple` directly enters a Unit Sink (`hex`, `bits`, `integers`, `raw`), `bdd` automatically packs the fields using their source layout (`4U4U` $\to$ 8 bits).
  - For `ones -> 4U4U -> hex`: outputs `ff` (8-bit byte).
  - For `ones -> 4U4U -> json`: outputs `{"fields":[15,15]}` (tuple preserved!).
  - For `ones -> 4U4U -> 16U -> hex`: outputs `000f` (explicitly packed to 16-bit word).

---

### Ambiguity 5: Reserved Keyword vs. File Name
- **Problem**: A user has a file named `hex` or `json`. Does `"hex -> 8 -> stdout"` read from the file `hex` or use a sink?
- **Analysis**:
  - Stage 0 is always a **Source** or **Slicer**.
  - If `hex` were a sink, it is illegal at Stage 0.
- **Resolution**:
  - Reserved keywords (`zeros`, `ones`, `rand`, `counter`, `netlink`, `stdin`, `stdout`, `hex`, `bits`, `json`, `csv`, `tuples`, `raw`, `visual`) are keywords in their respective positions (Source at Stage 0, Sink at Stage $N-1$).
  - Files matching keyword names MUST use standard file syntax: `file('hex')` or `'./hex'`.

---

### Ambiguity 6: Dual-Role Keywords (`tuples`)
- **Problem**: `tuples` can be an input source (`--input-tuples`) OR an output sink (`--output-tuples`).
- **Resolution**:
  - `tuples` at Stage 0: **Source** (reads comma-separated text lines from stdin).
  - `tuples` at Stage $N-1$: **Sink** (emits comma-separated text lines to stdout).
  - Both in one pipeline: `"tuples -> 8U8U -> add(1) -> 8U8U -> tuples"`. Unambiguous!

---

### Ambiguity 7: Incomplete Output Pattern Field Consumption & Discard Semantics
- **Problem**: A tuple has 2 fields `(15, 10)` (binary `1111`, `1010`), but the user writes `16U` on output.
- **Analysis of Bit Representation**:
  - `16U` specifies exactly **one** 16-bit integer field token.
  - When `bdd` maps tuple fields into the output pattern, `16U` consumes `field[0]` (value `15`).
  - It expands the 4-bit value `15` (`0b1111`) into a 16-bit unsigned big-endian integer:
    $$\texttt{0000 0000 0000 1111}_2 = \texttt{0x000F}$$
  - Notice the structure: it contains **12 zero (null) bits** followed by the **4 bits of Field 0**:
    $$\mathbf{12N4U} \quad (\text{or } 12z4U)$$
  - `field[1]` (`1010`) is completely unmapped and **discarded**!
  - It does **not** produce $\mathbf{8N4U4U}$ ($\texttt{0x00FA} = \texttt{0000 0000 1111 1010}_2$) because `16U` does not automatically glue or concatenate unconsumed fields.
- **Resolution**:
  - In v0.6, `bdd` requires explicit field mapping or a field concatenation operator when field counts differ.
  - If field count drops without an explicit discard token (`x`), emit a warning:
    `[bdd] Warning: Output pattern '16U' consumes 1 field; incoming tuple has 2 fields (field 1 discarded).`
  - To preserve both fields without concatenation: use `8z 4U 4U` (yields `8N4U4U`, `0x00FA`).
  - To combine both fields into a single 8-bit or 16-bit entity: use the **`glue`** operator (see Section 4.9).

---

### Ambiguity 8: Physical Container Slicing vs. Pattern Discards
- **Problem**: How to slice an unaligned 13-bit PID from a 188-byte MPEG-TS packet?
  - Option A (Container Slice): `188B[11:13]`
  - Option B (Pattern with Discards): `11x 13U 1480x`
- **Analysis**:
  - Both specify the exact same 1504-bit container cycle!
  - But `188B[11:13]` is far more readable and less error-prone than calculating $1504 - 11 - 13 = 1480$ bits.
- **Resolution**:
  - **Slicers** handle physical framing, offsets, and periodic container gaps (`188B[11:13]`).
  - **Patterns** handle field typing and unpacking (`13U`).
  - Combining them is fully supported and composable:
    $$\text{\texttt{"file('stream.ts') -> 188B[11:13] -> 13U -> hex"}}$$

---

### Ambiguity 9: Field Concatenation / Fusion ("Glueing" Fields)
- **Problem**: A user unpacks `4U4U` producing two 4-bit fields `f0 = 1111_2` (15) and `f1 = 1010_2` (10). They want to combine them by "glueing" them one after another into a single 8-bit field:
  $$\texttt{"1111"} + \texttt{"1010"} = \texttt{"11111010"} \quad (250 / \texttt{0xFA})$$
  and then format or manipulate that composite value.
- **Why Arithmetic `+` is Ambiguous**:
  - In mathematics and programming, `+` denotes arithmetic addition: $15 + 10 = 25$ (`0b00011001`).
  - Bit concatenation is **not** addition: $(\text{f0} \ll 4) \mid \text{f1} = 240 + 10 = 250$ (`0b11111010`).
  - Using bare `+` as an operator creates semantic confusion with arithmetic operators (`add(10)`).
- **The Solution: Field Fusion inside `rearrange(...)` via `|`**:
  Instead of needing a separate standalone concept, `rearrange(...)` naturally expands to support field fusion using the bitwise OR symbol `|`:
  ```bash
  rearrange(0, 1|2)
  ```
  - **Syntax & Semantics**:
    - `rearrange(0, 1|2)`: Emits field 0 unchanged, and joins field 1 (MSB) and field 2 (LSB) into a single composite field of width $W_1 + W_2$.
    - `rearrange(0|1)`: Fuses two 4-bit fields into one 8-bit field:
      ```bash
      "4U4U -> rearrange(0|1) -> 16U -> hex"   # Outputs 00fa (8N4U4U)!
      ```
    - `rearrange(2|1, 0)`: Reorders while fusing (field 2 as MSB, field 1 as LSB, followed by field 0).
    - `rearrange(0|1|2)`: Fuses three fields into one single word.
    - Named field support: `rearrange(tag, hi|lo)` cleanly preserves protocol semantics.
  - **Why `|` is the Ideal Delimiter**:
    1. Bitwise OR (`|`) is the literal hardware operation used to combine shifted bitfields: $(\text{f}_1 \ll W_2) \mid \text{f}_2$.
    2. Inside `rearrange(...)`, there is no arithmetic, making `|` completely unambiguous.
    3. It handles reordering, field dropping, and field fusion in a single, cohesive operator.
  - **Shorthand Operators**:
    - **`glue(f0, f1, ...)`** (alias **`concat`**): In-place shorthand when a user wants to fuse fields without listing all other field indices:
      `"4U4U -> glue(0, 1) -> 16U -> hex"`
    - **`split(field, w0, w1, ...)`**: The symmetric inverse. Splits an $N$-bit field into multiple sub-fields:
      `"16U -> split(0, 4, 12) -> json"`

---

## 5. Formal Pipeline Syntax Grammar (BNF)

```ebnf
Pipeline        ::= Stage ("->" Stage)+
Stage           ::= SourceStage | SlicerStage | UnpackStage | ManipStage | PackStage | FramerStage | SinkStage

SourceStage     ::= "stdin" | "zeros" | "ones" | "rand" | "counter" [ "(" Params ")" ]
                  | "netlink" [ ":proc" ] | "tuples" | "file(" FilePath ")" | StringLiteral

SlicerStage     ::= [ Skip ":" ] UnitContainer [ "+" Gap ]
UnitContainer   ::= Number [ UnitSuffix ]
                  | Number [ UnitSuffix ] "[" Offset (":" | ",") Unit "]"
                  | "[" PreGap (":" | ",") Unit (":" | ",") PostGap "]"

UnpackStage     ::= PatternString | PresetName
ManipStage      ::= ManipName "(" [ ManipArgs ] ")" | "not" | "abs" | "sign"
PackStage       ::= PatternString | PresetName
FramerStage     ::= SlicerStage

SinkStage       ::= "stdout" | "raw" | "bin" | "hex" | "bits" | "integers"
                  | "json" [ ":object" ] | "csv" | "tuples" | "visual"
                  | "file(" FilePath ")"
```

---

## 6. Syntax Comparison & Evolution Table

| Scenario | Legacy CLI (v0.5) | Verbose Unified Pipeline | Minimalist Idiomatic Pipeline (v0.6) |
|---|---|---|---|
| **Synthetic Test Pattern to Hex** | `bdd -0 -u 8 --xor 0,0xAA -x -c 4` | `"zeros -> 8 -> xor(0xAA) -> 8 -> hex"` | `"zeros -> 8 -> xor(0xAA) -> hex"` |
| **Unaligned Tuple Transcoding** | `bdd 4U4U 16U < in.bin > out.bin` | `"stdin -> 8 -> 4U4U -> 16U -> 16 -> stdout"` | `"4U4U -> 16U"` |
| **Field Glueing & Packing** | *(Requires multi-step bit shift math)* | `"stdin -> 8 -> 4U4U -> rearrange(0|1) -> 16U -> 16 -> hex"` | `"4U4U -> rearrange(0|1) -> 16U -> hex"` |
| **MPEG-TS PID Extraction to File** | `bdd "188B[11:13] -> 13" -x < in.ts` | `"file('in.ts') -> 188B[11:13] -> 13U -> 13 -> hex"` | `"file('in.ts') -> 188B[11:13] -> hex"` |
| **Kernel Process Telemetry to JSON** | `sudo bdd --input-netlink --preset proc-event --filter 1,==,2 --output-json` | `"netlink -> proc-event -> filter(what == 2) -> json"` | `"netlink -> proc-event -> filter(what == 2) -> json"` |
| **FP16 to Blackwell FP4 Quantization** | `bdd --input-pattern=16H --output-pattern=4E < in > out` | `"stdin -> 16 -> 16H -> 4E -> 4 -> stdout"` | `"16H -> 4E"` |
| **IoT ADC 12-bit Calibration** | `bdd -u 12 --sub 0,512 --mul 0,2 --output-unit 8 < adc.raw` | `"file('adc.raw') -> 12 -> sub(512) -> mul(2) -> 8 -> raw"` | `"file('adc.raw') -> 12 -> sub(512) -> mul(2) -> 8"` |

---

## 7. Answers to Specific Design Questions

### Q1: Does it make sense to have unit patterns separately?
**Answer**: **No.** In v0.6, unit patterns (`4U4U`, `16U`, `16H`, `8E`) are first-class pipeline stages connected by `->`. Separate positional arguments (`bdd 4U4U 16U`) remain supported for 100% backwards compatibility, but the unified string `"4U4U -> 16U"` is the preferred, primary syntax.

### Q2: Are `8` and `16` needed in `"ones -> 8 -> 4U4U -> 16U -> 16 -> hex"`?
**Answer**: **No, they are completely redundant.**
- `4U4U` is inherently 8 bits.
- `16U` is inherently 16 bits.
Writing `"ones -> 4U4U -> 16U -> hex"` is cleaner, faster to type, and less error-prone.
If `8` or `16` is written, `bdd` validates that the dimensions match, allowing explicit assertion without penalty.

### Q3: Is downstream inference confusing to users?
**Answer**: **No, as long as dimension conflicts produce clear errors.**
A bitstream source has no natural boundaries; the unpacker pattern (`4U4U`) serves as the measuring lens that pulls 8 bits at a time. The only case where users would be confused is if an explicit slicer (`12`) conflicts with a pattern (`4U4U`). By enforcing a **strict dimension check error** on mismatches, confusion is completely eliminated.

### Q4: In `"stdin -> 8 -> 4U4U -> 16U -> 16 -> stdout"`, does this combine `8N4U4U` or `12N4U`?
**Answer**: **It produces `12N4U` (12 null/zero bits + 4 bits of field 0), NOT `8N4U4U`.**
- `4U4U` unpacks 8 bits into two 4-bit fields: `field[0] = 0b1111` (15) and `field[1] = 0b1010` (10).
- `16U` specifies a single 16-bit field token. It consumes `field[0]` (15), expands it into a 16-bit integer with 12 leading zeros (`0000 0000 0000 1111` = `0x000F`), and **discards** `field[1]`.
- Thus, the output word contains $12\text{ null bits} + 4\text{ bits of field 0} = \mathbf{12N4U}$.
- It does **not** combine `8N4U4U` (`0000 0000 1111 1010` = `0x00FA`). To get `8N4U4U` without glueing, the output pattern must explicitly specify `8z 4U 4U`.

### Q5: Could joining be something like `rearrange(0, 1|2)` combining fields 1 and 2?
**Answer**: **Yes, this is an extraordinarily elegant and ergonomic design.**
- By enhancing `rearrange(...)` with the bitwise OR symbol `|`, field selection, reordering, dropping, and bitwise concatenation are unified into a single expressive operator:
  $$\text{\texttt{"4U4U -> rearrange(0\|1) -> 16U -> hex"}}$$
  - `4U4U` unpacks `(f0: 4U = 1111, f1: 4U = 1010)`.
  - `rearrange(0|1)` concatenates `f0` (MSB) and `f1` (LSB) into a single 8-bit field `(f0: 8U = 11111010 = 250 / 0xFA)`.
  - When that single field enters `16U`, it expands into 16 bits with 8 leading zeros: `0x00FA` (`0000 0000 1111 1010`), achieving the exact `8N4U4U` combined result.
- Supports arbitrary combinations:
  - `rearrange(0, 1|2)`: Keeps field 0 separate, joins fields 1 and 2.
  - `rearrange(2|1, 0)`: Swaps byte/field order while joining.
  - `rearrange(tag, hi|lo)`: Works seamlessly with named fields.
  - In-place shorthand: `glue(1, 2)` (alias `concat(1, 2)`) acts as sugar when other fields don't need reordering.

---

## 8. Implementation Plan for `bdd` v0.6.0

1. **Pipeline AST & Parser (`src/stream_pipeline.rs`)**:
   - Parse `->` delimited strings into a strongly-typed `PipelineAst`.
   - Validate state transitions (`Bitstream` $\to$ `BitUnit` $\to$ `Tuple` $\to$ `BitUnit` $\to$ `Sink`).
   - Validate dimension equality when both slicer and pattern are present.
2. **Auto-Packing Engine (`src/sink.rs`, `src/engine.rs`)**:
   - Enable Unit Sinks (`hex`, `bits`, `raw`) to accept `Tuple` inputs directly by auto-packing via active field dimensions.
3. **Diagnostic Engine Integration (`src/explain.rs`)**:
   - Extend `--explain-pattern` to explain full unified pipelines:
     `bdd --explain "ones -> 4U4U -> 16U -> hex"` prints the complete stage transition graph and bit allocations.
