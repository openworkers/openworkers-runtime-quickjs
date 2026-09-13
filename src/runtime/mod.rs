pub mod base64;
pub mod crypto;
pub mod text;
pub mod timers;
pub mod url;

pub use base64::setup_base64;
pub use crypto::setup_crypto;
pub use text::setup_text;
pub use timers::setup_timers;
pub use url::setup_url;

/// The bytes behind a `Uint8Array`, or `None` if its buffer was detached.
///
/// Reading them is only sound while no JavaScript runs, since the guest can
/// detach or resize the buffer; every caller here is inside a native function.
pub fn typed_array_bytes<'a>(view: &'a rquickjs::TypedArray<'_, u8>) -> Option<&'a [u8]> {
    unsafe { view.as_bytes() }
}
