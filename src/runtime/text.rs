use rquickjs::{Ctx, Function, Result, TypedArray};

/// Length of the trailing bytes that could still grow into a valid sequence
fn incomplete_tail(bytes: &[u8]) -> usize {
    for back in 1..=3.min(bytes.len()) {
        let lead = bytes[bytes.len() - back];

        let expected = match lead {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => continue,
        };

        return if expected > back { back } else { 0 };
    }

    0
}

/// Setup the `TextEncoder` and `TextDecoder` globals
pub fn setup_text(ctx: &Ctx<'_>) -> Result<()> {
    let globals = ctx.globals();

    fn encode<'js>(ctx: Ctx<'js>, input: String) -> Result<TypedArray<'js, u8>> {
        TypedArray::new(ctx, input.into_bytes())
    }
    globals.set("__text_encode", Function::new(ctx.clone(), encode)?)?;

    // Undefined when fatal decoding hits invalid UTF-8
    fn decode(view: TypedArray<'_, u8>, fatal: bool) -> Option<String> {
        let bytes = view.as_bytes().unwrap_or(&[]);

        match fatal {
            true => String::from_utf8(bytes.to_vec()).ok(),
            false => Some(String::from_utf8_lossy(bytes).into_owned()),
        }
    }
    globals.set("__text_decode", Function::new(ctx.clone(), decode)?)?;

    fn tail(view: TypedArray<'_, u8>) -> usize {
        incomplete_tail(view.as_bytes().unwrap_or(&[]))
    }
    globals.set("__text_tail", Function::new(ctx.clone(), tail)?)?;

    ctx.eval::<(), _>(TEXT_JS)?;

    Ok(())
}

const TEXT_JS: &str = r#"
    globalThis.TextEncoder = class TextEncoder {
        get encoding() {
            return 'utf-8';
        }

        encode(input) {
            return __text_encode(input === undefined ? '' : String(input));
        }
    };

    globalThis.TextDecoder = class TextDecoder {
        constructor(label, options) {
            const encoding = String(label === undefined ? 'utf-8' : label).trim().toLowerCase();

            if (encoding !== 'utf-8' && encoding !== 'utf8' && encoding !== 'unicode-1-1-utf-8') {
                throw new RangeError('TextDecoder only supports utf-8, got: ' + label);
            }

            options = options || {};
            this.fatal = !!options.fatal;
            this.ignoreBOM = !!options.ignoreBOM;
            this._pending = new Uint8Array(0);
            this._started = false;
        }

        get encoding() {
            return 'utf-8';
        }

        decode(input, options) {
            const stream = !!(options && options.stream);

            if (input === undefined) {
                this._pending = new Uint8Array(0);
                this._started = false;
                return '';
            }

            let bytes;
            if (input instanceof Uint8Array) {
                bytes = input;
            } else if (input instanceof ArrayBuffer) {
                bytes = new Uint8Array(input);
            } else if (ArrayBuffer.isView(input)) {
                bytes = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
            } else {
                throw new TypeError('TextDecoder.decode expects a BufferSource');
            }

            if (this._pending.length > 0) {
                const joined = new Uint8Array(this._pending.length + bytes.length);
                joined.set(this._pending, 0);
                joined.set(bytes, this._pending.length);
                bytes = joined;
            }

            // A sequence split across chunks waits for the rest instead of decoding to U+FFFD
            const held = stream ? __text_tail(bytes) : 0;
            this._pending = held > 0 ? bytes.slice(bytes.length - held) : new Uint8Array(0);

            let text = __text_decode(bytes.subarray(0, bytes.length - held), this.fatal);

            if (text === undefined) {
                throw new TypeError('TextDecoder.decode: invalid utf-8');
            }

            if (!this.ignoreBOM && !this._started && text.charCodeAt(0) === 0xfeff) {
                text = text.slice(1);
            }

            this._started = stream;

            return text;
        }
    };
"#;
