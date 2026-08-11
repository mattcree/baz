//! Small, host-owned desktop actions. Baz never fetches the URL itself.

/// Open a Wikipedia search for `artist` in the listener's browser.
pub fn look_up_artist(artist: &str) -> Result<(), String> {
    open(&format!(
        "https://en.wikipedia.org/wiki/Special:Search?search={}",
        encode_query(artist)
    ))
}

fn encode_query(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

#[cfg(target_os = "linux")]
fn open(url: &str) -> Result<(), String> {
    use std::collections::HashMap;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedObjectPath, Value};

    let connection = Connection::session().map_err(|error| error.to_string())?;
    let portal = Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.OpenURI",
    )
    .map_err(|error| error.to_string())?;
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    let _: OwnedObjectPath = portal
        .call("OpenURI", &("", url, options))
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open(url: &str) -> Result<(), String> {
    command("open", &[url])
}

#[cfg(target_os = "windows")]
fn open(url: &str) -> Result<(), String> {
    command("cmd", &["/C", "start", "", url])
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn command(program: &str, arguments: &[&str]) -> Result<(), String> {
    std::process::Command::new(program)
        .args(arguments)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::encode_query;

    #[test]
    fn wikipedia_query_is_utf8_percent_encoded() {
        assert_eq!(encode_query("AC/DC & Björk"), "AC%2FDC%20%26%20Bj%C3%B6rk");
    }
}
