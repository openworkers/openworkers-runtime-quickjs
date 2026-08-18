use ring::{digest, rand};
use rquickjs::{Ctx, Function, Object, Result};

/// Setup the `crypto` global
pub fn setup_crypto(ctx: &Ctx<'_>) -> Result<()> {
    let globals = ctx.globals();

    let crypto = Object::new(ctx.clone())?;

    let get_random_values = Function::new(ctx.clone(), |array: rquickjs::TypedArray<'_, u8>| {
        let Some(data) = array.as_bytes() else {
            return Err(rquickjs::Error::new_from_js_message(
                "object",
                "Uint8Array",
                "Buffer is detached",
            ));
        };

        let len = data.len();
        let mut bytes = vec![0u8; len];

        // Throw rather than leave the caller with a buffer we did not randomize
        rand::SecureRandom::fill(&rand::SystemRandom::new(), &mut bytes).map_err(|_| {
            rquickjs::Error::new_from_js_message("object", "Uint8Array", "System RNG failed")
        })?;

        let raw = array.as_raw().unwrap();
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), raw.ptr.as_ptr(), len);
        }

        Ok::<_, rquickjs::Error>(())
    })?;
    crypto.set("_getRandomValues", get_random_values)?;

    let random_uuid = Function::new(ctx.clone(), || {
        let uuid = uuid::Uuid::new_v4().to_string();
        Ok::<_, rquickjs::Error>(uuid)
    })?;
    crypto.set("randomUUID", random_uuid)?;

    let subtle = Object::new(ctx.clone())?;

    let native_digest = Function::new(
        ctx.clone(),
        |algo: String, data: rquickjs::TypedArray<'_, u8>| {
            let bytes = data.as_bytes().unwrap_or(&[]);

            let algorithm = match algo.to_uppercase().as_str() {
                "SHA-1" => &digest::SHA1_FOR_LEGACY_USE_ONLY,
                "SHA-256" => &digest::SHA256,
                "SHA-384" => &digest::SHA384,
                "SHA-512" => &digest::SHA512,
                _ => {
                    return Err(rquickjs::Error::new_from_js_message(
                        "string",
                        "algorithm",
                        &format!("Unsupported algorithm: {}", algo),
                    ));
                }
            };

            let result = digest::digest(algorithm, bytes);
            let hex: String = result
                .as_ref()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect();

            Ok::<_, rquickjs::Error>(hex)
        },
    )?;
    subtle.set("__nativeDigest", native_digest)?;

    crypto.set("subtle", subtle)?;
    globals.set("crypto", crypto)?;

    ctx.eval::<(), _>(
        r#"
        (function() {
            const _native = crypto._getRandomValues;
            const QUOTA = 65536;

            // The views the spec allows; the float ones and DataView are rejected
            const integerViews = [
                globalThis.Int8Array,
                globalThis.Uint8Array,
                globalThis.Uint8ClampedArray,
                globalThis.Int16Array,
                globalThis.Uint16Array,
                globalThis.Int32Array,
                globalThis.Uint32Array,
                globalThis.BigInt64Array,
                globalThis.BigUint64Array
            ].filter(Boolean);

            crypto.getRandomValues = function(array) {
                if (!integerViews.some(view => array instanceof view)) {
                    throw new DOMException(
                        'crypto.getRandomValues expects an integer TypedArray',
                        'TypeMismatchError'
                    );
                }

                if (array.byteLength > QUOTA) {
                    throw new DOMException(
                        'crypto.getRandomValues accepts at most ' + QUOTA + ' bytes',
                        'QuotaExceededError'
                    );
                }

                _native(new Uint8Array(array.buffer, array.byteOffset, array.byteLength));

                return array;
            };
            delete crypto._getRandomValues;
        })();

        crypto.subtle.digest = function(algorithm, data) {
            return new Promise((resolve, reject) => {
                try {
                    let bytes;
                    if (data instanceof ArrayBuffer) {
                        bytes = new Uint8Array(data);
                    } else if (data instanceof Uint8Array) {
                        bytes = data;
                    } else {
                        reject(new Error('Data must be ArrayBuffer or Uint8Array'));
                        return;
                    }
                    const algoName = typeof algorithm === 'string' ? algorithm : algorithm.name;
                    const hexResult = crypto.subtle.__nativeDigest(algoName, bytes);
                    const len = hexResult.length / 2;
                    const buffer = new ArrayBuffer(len);
                    const view = new Uint8Array(buffer);
                    for (let i = 0; i < len; i++) {
                        view[i] = parseInt(hexResult.substr(i * 2, 2), 16);
                    }
                    resolve(buffer);
                } catch (e) {
                    reject(e);
                }
            });
        };
        "#,
    )?;

    Ok(())
}
