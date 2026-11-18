//! pointer.rs — the text git holds in place of a large file, as git-lfs's spec writes it.
//! What a reader gets wrong: a parse refuses a text that is not exactly a pointer (an unknown key before `oid`, a
//! second `oid`, a size that is not a number), so a plain file that merely resembles one is never taken for one.

const VERSION: &str = "https://git-lfs.github.com/spec/v1";

/// A pointer is never larger than this; a bigger blob is not read as one.
pub const MAX_POINTER: usize = 1024;

/// A SHA-256 as git-lfs writes it: 64 lowercase hex digits, nothing a path could be made of.
pub fn valid_oid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pointer {
    /// The hex SHA-256 of the file, without the `sha256:` prefix.
    pub oid: String,
    pub size: u64,
}

impl Pointer {
    pub fn text(&self) -> String {
        format!(
            "version {VERSION}\noid sha256:{}\nsize {}\n",
            self.oid, self.size
        )
    }

    /// The pointer a blob holds, or none when the blob is anything else.
    pub fn parse(bytes: &[u8]) -> Option<Pointer> {
        if bytes.len() > MAX_POINTER {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?;
        let mut lines = text.lines();
        if lines.next()? != format!("version {VERSION}") {
            return None;
        }
        let (mut oid, mut size) = (None, None);
        for line in lines {
            let (key, value) = line.split_once(' ')?;
            match key {
                "oid" if oid.is_none() => {
                    let hex = value.strip_prefix("sha256:")?;
                    if !valid_oid(hex) {
                        return None;
                    }
                    oid = Some(hex.to_string());
                }
                "size" if size.is_none() => size = Some(value.parse::<u64>().ok()?),
                "oid" | "size" => return None,
                _ if oid.is_none() => return None,
                _ => {}
            }
        }
        Some(Pointer {
            oid: oid?,
            size: size?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OID: &str = "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393";

    #[test]
    fn a_pointer_is_written_as_the_spec_has_it_and_read_back() {
        let p = Pointer {
            oid: OID.into(),
            size: 12345,
        };
        assert_eq!(
            p.text(),
            format!("version https://git-lfs.github.com/spec/v1\noid sha256:{OID}\nsize 12345\n")
        );
        assert_eq!(Pointer::parse(p.text().as_bytes()), Some(p));
    }

    #[test]
    fn what_is_not_exactly_a_pointer_is_not_read_as_one() {
        let ok = format!("version https://git-lfs.github.com/spec/v1\noid sha256:{OID}\nsize 1\n");
        assert!(Pointer::parse(ok.as_bytes()).is_some());
        let before = format!(
            "version https://git-lfs.github.com/spec/v1\next-0-x y\noid sha256:{OID}\nsize 1\n"
        );
        assert_eq!(
            Pointer::parse(before.as_bytes()),
            None,
            "an unknown key before oid"
        );
        let after = format!("{ok}x-note hello\n");
        assert!(
            Pointer::parse(after.as_bytes()).is_some(),
            "a key after oid is the spec's extension room"
        );
        assert_eq!(
            Pointer::parse(format!("{ok}oid sha256:{OID}\n").as_bytes()),
            None,
            "two oids"
        );
        assert_eq!(
            Pointer::parse(b"version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 1\n"),
            None
        );
        assert_eq!(
            Pointer::parse(
                format!("version https://git-lfs.github.com/spec/v1\noid sha256:{OID}\nsize x\n")
                    .as_bytes()
            ),
            None
        );
        assert_eq!(Pointer::parse(b"RIFF\0\0"), None);
        assert_eq!(Pointer::parse(&vec![b'a'; 2000]), None);
    }
}
