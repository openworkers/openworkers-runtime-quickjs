use base64::Engine;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use rquickjs::{Ctx, Function, Result};

/// Forgiving-base64 per the Infra standard: padding is optional and the trailing
/// bits of a partial group are discarded rather than rejected
const FORGIVING: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::RequireNone)
        .with_decode_allow_trailing_bits(true),
);

fn is_ascii_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

/// Decode a base64 string into one byte per code unit, or None if it is not base64
fn decode(input: &str) -> Option<String> {
    let mut data: String = input.chars().filter(|c| !is_ascii_whitespace(*c)).collect();

    if data.len().is_multiple_of(4) {
        for _ in 0..2 {
            if data.ends_with('=') {
                data.pop();
            }
        }
    }

    if data.len() % 4 == 1 {
        return None;
    }

    if !data
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/')
    {
        return None;
    }

    let bytes = FORGIVING.decode(data).ok()?;

    Some(bytes.iter().map(|b| *b as char).collect())
}

/// Encode one byte per code unit, or None if a code unit does not fit in a byte
fn encode(input: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(input.len());

    for c in input.chars() {
        bytes.push(u8::try_from(c as u32).ok()?);
    }

    Some(FORGIVING.encode(bytes))
}

/// Setup the `atob` and `btoa` globals
pub fn setup_base64(ctx: &Ctx<'_>) -> Result<()> {
    let globals = ctx.globals();

    globals.set(
        "__atob",
        Function::new(ctx.clone(), |input: String| decode(&input))?,
    )?;
    globals.set(
        "__btoa",
        Function::new(ctx.clone(), |input: String| encode(&input))?,
    )?;

    ctx.eval::<(), _>(BASE64_JS)?;

    Ok(())
}

const BASE64_JS: &str = r#"
    globalThis.atob = function atob(data) {
        const decoded = __atob(String(data));

        if (decoded === undefined) {
            throw new TypeError('atob: the string contains invalid characters');
        }

        return decoded;
    };

    globalThis.btoa = function btoa(data) {
        const encoded = __btoa(String(data));

        if (encoded === undefined) {
            throw new TypeError('btoa: a code unit is out of the Latin-1 range');
        }

        return encoded;
    };
"#;
