#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub version: u32,
    pub width: u32,
    pub height: u32,
}

const SUPPORTED_VERSION: u32 = 1;

pub fn parse_header(src: &str) -> Result<(Header, &str), String> {
    let (first, rest) = src.split_once('\n').unwrap_or((src, ""));
    let first = first.trim_end();

    let fields = first
        .strip_prefix("~fold ")
        .ok_or("a Fold file must start with a header like `~fold v1 256x256`")?;

    let (version, size) = fields
        .split_once(' ')
        .ok_or("header needs a version and a size, like `~fold v1 256x256`")?;

    let version: u32 = version
        .strip_prefix('v')
        .and_then(|v| v.parse().ok())
        .ok_or(format!(
            "bad version `{version}`, expected something like `v1`"
        ))?;

    if version > SUPPORTED_VERSION {
        return Err(format!(
            "this file needs Fold v{version}; this engine supports up to v{SUPPORTED_VERSION}"
        ));
    }

    let (w, h) = size
        .split_once('x')
        .ok_or("size needs to be in the form `WIDTHxHEIGHT`, like `256x256`")?;

    let width: u32 = w
        .parse()
        .map_err(|_| format!("bad width `{w}`, expected a positive integer"))?;
    let height: u32 = h
        .parse()
        .map_err(|_| format!("bad height `{h}`, expected a positive integer"))?;

    if width == 0 || height == 0 {
        return Err("canvas size must be greater than zero".into());
    }

    Ok((
        Header {
            version,
            width,
            height,
        },
        rest,
    ))
}
