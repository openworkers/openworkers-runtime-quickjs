use rquickjs::{Ctx, Function, Object, Result};
use url::Url;

/// The WHATWG URL components, as the JS `URL` class exposes them
fn components<'js>(ctx: &Ctx<'js>, url: &Url) -> Result<Object<'js>> {
    let parts = Object::new(ctx.clone())?;

    let host = match (url.host_str(), url.port()) {
        (Some(h), Some(p)) => format!("{}:{}", h, p),
        (Some(h), None) => h.to_string(),
        (None, _) => String::new(),
    };

    let search = match url.query() {
        Some("") | None => String::new(),
        Some(q) => format!("?{}", q),
    };

    let hash = match url.fragment() {
        Some("") | None => String::new(),
        Some(f) => format!("#{}", f),
    };

    parts.set("href", url.as_str())?;
    parts.set("origin", url.origin().ascii_serialization())?;
    parts.set("protocol", format!("{}:", url.scheme()))?;
    parts.set("username", url.username())?;
    parts.set("password", url.password().unwrap_or(""))?;
    parts.set("host", host)?;
    parts.set("hostname", url.host_str().unwrap_or(""))?;
    parts.set(
        "port",
        url.port().map(|p| p.to_string()).unwrap_or_default(),
    )?;
    parts.set("pathname", url.path())?;
    parts.set("search", search)?;
    parts.set("hash", hash)?;

    Ok(parts)
}

/// Apply one WHATWG setter; invalid values are ignored, as the spec requires
fn apply(url: &mut Url, part: &str, value: &str) {
    match part {
        "protocol" => {
            let scheme = value.split(':').next().unwrap_or("");
            let _ = url.set_scheme(scheme);
        }
        "username" => {
            let _ = url.set_username(value);
        }
        "password" => {
            let _ = url.set_password(if value.is_empty() { None } else { Some(value) });
        }
        "host" => {
            let (hostname, port) = match value.split_once(':') {
                Some((h, p)) => (h, Some(p)),
                None => (value, None),
            };

            if url.set_host(Some(hostname)).is_err() {
                return;
            }

            if let Some(port) = port {
                let _ = url.set_port(port.parse().ok());
            }
        }
        "hostname" => {
            let _ = url.set_host(Some(value));
        }
        "port" => {
            let _ = url.set_port(if value.is_empty() {
                None
            } else {
                match value.parse() {
                    Ok(p) => Some(p),
                    Err(_) => return,
                }
            });
        }
        "pathname" => {
            if !url.cannot_be_a_base() {
                url.set_path(value);
            }
        }
        "search" => {
            let query = value.strip_prefix('?').unwrap_or(value);
            url.set_query(if query.is_empty() { None } else { Some(query) });
        }
        "hash" => {
            let fragment = value.strip_prefix('#').unwrap_or(value);
            url.set_fragment(if fragment.is_empty() {
                None
            } else {
                Some(fragment)
            });
        }
        _ => {}
    }
}

/// Setup the `URL` and `URLSearchParams` globals
pub fn setup_url(ctx: &Ctx<'_>) -> Result<()> {
    let globals = ctx.globals();

    // Returns the components, or null when the input is not a valid URL
    fn parse<'js>(
        ctx: Ctx<'js>,
        input: String,
        base: Option<String>,
    ) -> Result<Option<Object<'js>>> {
        let url = match base {
            Some(base) => Url::parse(&base).and_then(|base| base.join(&input)),
            None => Url::parse(&input),
        };

        match url {
            Ok(url) => components(&ctx, &url).map(Some),
            Err(_) => Ok(None),
        }
    }
    globals.set("__url_parse", Function::new(ctx.clone(), parse)?)?;

    fn set<'js>(
        ctx: Ctx<'js>,
        href: String,
        part: String,
        value: String,
    ) -> Result<Option<Object<'js>>> {
        let Ok(mut url) = Url::parse(&href) else {
            return Ok(None);
        };

        apply(&mut url, &part, &value);

        components(&ctx, &url).map(Some)
    }
    globals.set("__url_set", Function::new(ctx.clone(), set)?)?;

    let parse_query = Function::new(ctx.clone(), |input: String| {
        url::form_urlencoded::parse(input.as_bytes())
            .map(|(name, value)| vec![name.into_owned(), value.into_owned()])
            .collect::<Vec<Vec<String>>>()
    })?;
    globals.set("__urlencoded_parse", parse_query)?;

    let serialize_query = Function::new(ctx.clone(), |pairs: Vec<Vec<String>>| {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());

        for pair in &pairs {
            serializer.append_pair(&pair[0], &pair[1]);
        }

        serializer.finish()
    })?;
    globals.set("__urlencoded_serialize", serialize_query)?;

    ctx.eval::<(), _>(URL_JS)?;

    Ok(())
}

const URL_JS: &str = r#"
    globalThis.URLSearchParams = class URLSearchParams {
        constructor(init) {
            this._list = [];
            this._url = null;

            if (init === undefined || init === null) {
                return;
            }

            if (init instanceof URLSearchParams) {
                this._list = init._list.map(entry => [entry[0], entry[1]]);
            } else if (typeof init === 'string') {
                this._list = __urlencoded_parse(init.startsWith('?') ? init.slice(1) : init);
            } else if (Array.isArray(init)) {
                for (const entry of init) {
                    if (entry.length !== 2) {
                        throw new TypeError('Each query pair must be an iterable [name, value] tuple');
                    }
                    this._list.push([String(entry[0]), String(entry[1])]);
                }
            } else if (typeof init === 'object') {
                for (const name of Object.keys(init)) {
                    this._list.push([name, String(init[name])]);
                }
            } else {
                this._list = __urlencoded_parse(String(init));
            }
        }

        _update() {
            if (this._url) {
                this._url._set('search', this.toString());
            }
        }

        append(name, value) {
            this._list.push([String(name), String(value)]);
            this._update();
        }

        delete(name, value) {
            name = String(name);
            const matchValue = value !== undefined ? String(value) : undefined;
            this._list = this._list.filter(entry =>
                entry[0] !== name || (matchValue !== undefined && entry[1] !== matchValue)
            );
            this._update();
        }

        get(name) {
            name = String(name);
            const entry = this._list.find(entry => entry[0] === name);
            return entry ? entry[1] : null;
        }

        getAll(name) {
            name = String(name);
            return this._list.filter(entry => entry[0] === name).map(entry => entry[1]);
        }

        has(name, value) {
            name = String(name);
            const matchValue = value !== undefined ? String(value) : undefined;
            return this._list.some(entry =>
                entry[0] === name && (matchValue === undefined || entry[1] === matchValue)
            );
        }

        set(name, value) {
            name = String(name);
            value = String(value);
            const index = this._list.findIndex(entry => entry[0] === name);

            if (index === -1) {
                this._list.push([name, value]);
            } else {
                this._list[index][1] = value;
                this._list = this._list.filter((entry, i) => i <= index || entry[0] !== name);
            }

            this._update();
        }

        sort() {
            this._list.sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
            this._update();
        }

        forEach(callback, thisArg) {
            for (const [name, value] of this._list) {
                callback.call(thisArg, value, name, this);
            }
        }

        *entries() {
            for (const [name, value] of this._list) {
                yield [name, value];
            }
        }

        *keys() {
            for (const [name] of this._list) {
                yield name;
            }
        }

        *values() {
            for (const [, value] of this._list) {
                yield value;
            }
        }

        [Symbol.iterator]() {
            return this.entries();
        }

        get size() {
            return this._list.length;
        }

        toString() {
            return __urlencoded_serialize(this._list);
        }
    };

    globalThis.URL = class URL {
        constructor(url, base) {
            const parts = __url_parse(String(url), base === undefined ? null : String(base));

            if (!parts) {
                throw new TypeError('Invalid URL: ' + String(url));
            }

            this._parts = parts;
            this._params = null;
        }

        _set(part, value) {
            const parts = __url_set(this._parts.href, part, String(value));

            if (parts) {
                this._parts = parts;
            }
        }

        _reload() {
            if (this._params) {
                this._params._list = __urlencoded_parse(this._parts.search.slice(1));
            }
        }

        get href() { return this._parts.href; }
        get origin() { return this._parts.origin; }
        get protocol() { return this._parts.protocol; }
        get username() { return this._parts.username; }
        get password() { return this._parts.password; }
        get host() { return this._parts.host; }
        get hostname() { return this._parts.hostname; }
        get port() { return this._parts.port; }
        get pathname() { return this._parts.pathname; }
        get search() { return this._parts.search; }
        get hash() { return this._parts.hash; }

        set href(value) {
            const parts = __url_parse(String(value), null);

            if (!parts) {
                throw new TypeError('Invalid URL: ' + String(value));
            }

            this._parts = parts;
            this._reload();
        }

        set protocol(value) { this._set('protocol', value); }
        set username(value) { this._set('username', value); }
        set password(value) { this._set('password', value); }
        set host(value) { this._set('host', value); }
        set hostname(value) { this._set('hostname', value); }
        set port(value) { this._set('port', value); }
        set pathname(value) { this._set('pathname', value); }
        set hash(value) { this._set('hash', value); }

        set search(value) {
            this._set('search', value);
            this._reload();
        }

        get searchParams() {
            if (!this._params) {
                this._params = new URLSearchParams(this._parts.search);
                this._params._url = this;
            }

            return this._params;
        }

        toString() { return this._parts.href; }

        toJSON() { return this._parts.href; }
    };
"#;
