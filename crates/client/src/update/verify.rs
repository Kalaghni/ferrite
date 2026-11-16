//! Checking a download before anything is installed from it.
//!
//! Every release carries a `SHA256SUMS` file (the output of `sha256sum`,
//! made by CI from the very files it uploads). The package is installed
//! only if the release lists it there, its SHA-256 matches, and its size is
//! the size GitHub's release API reports for it. This catches a truncated,
//! corrupted or mismatched download; it is not a signature, since the sums
//! come from the same release as the package.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

/// The name of the checksum list in every release.
pub const SUMS: &str = "SHA256SUMS";
/// A sums file bigger than this is not one of ours.
pub const SUMS_MAX: u64 = 64 * 1024;

/// One file of a release, as the release API describes it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
}

#[derive(serde::Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

/// The assets in the API's answer for one release.
pub fn parse_assets(json: &str) -> Result<Vec<Asset>, String> {
    serde_json::from_str::<Release>(json).map(|r| r.assets).map_err(|_| "GitHub's release list could not be read".to_string())
}

/// The lines of a `sha256sum` listing: `<64 hex>  <name>`, or
/// `<64 hex> *<name>` for one made in binary mode. Blank lines are skipped;
/// anything else that does not fit makes the whole file unusable, as does a
/// name listed twice with different sums.
pub fn parse_sums(text: &str) -> Result<Vec<(String, [u8; 32])>, String> {
    let bad = || format!("{SUMS} is not in the expected format");
    let mut out: Vec<(String, [u8; 32])> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let (hex, rest) = line.split_at_checked(64).ok_or_else(bad)?;
        let hash = parse_hex(hex).ok_or_else(bad)?;
        let name = rest.strip_prefix("  ").or_else(|| rest.strip_prefix(" *")).ok_or_else(bad)?;
        if name.is_empty() {
            return Err(bad());
        }
        match out.iter().find(|(n, _)| n == name) {
            Some((_, h)) if *h != hash => return Err(format!("{SUMS} lists {name} twice with different sums")),
            Some(_) => {}
            None => out.push((name.to_string(), hash)),
        }
    }
    Ok(out)
}

/// The sum `text` gives for the file `name`.
pub fn expected_sum(text: &str, name: &str) -> Result<[u8; 32], String> {
    parse_sums(text)?
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, h)| h)
        .ok_or_else(|| format!("{name} is not listed in the release's {SUMS}"))
}

fn parse_hex(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = digit(b[2 * i])? << 4 | digit(b[2 * i + 1])?;
    }
    Some(out)
}

pub fn hex(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-256 and length of a file.
pub fn sha256_file(path: &Path) -> std::io::Result<([u8; 32], u64)> {
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut len = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        len += n as u64;
    }
    Ok((hasher.finalize().into(), len))
}

/// Is the downloaded `path` exactly the file the release describes?
pub fn check_download(path: &Path, size: u64, sum: &[u8; 32]) -> Result<(), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (got, len) = sha256_file(path).map_err(|e| format!("cannot read the download: {e}"))?;
    if len != size {
        return Err(format!("{name} is {len} bytes, the release says {size}; not installing"));
    }
    if got != *sum {
        let (got, want) = (hex(&got), hex(sum));
        return Err(format!(
            "{name} does not match the release's {SUMS} (sha256 {}..., expected {}...); not installing",
            &got[..12],
            &want[..12]
        ));
    }
    Ok(())
}

/// May a download come from (or end up at) this URL? HTTPS only, and only
/// GitHub itself or the hosts it serves release files from
/// (objects.githubusercontent.com, release-assets.githubusercontent.com).
pub fn allowed_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").to_ascii_lowercase();
    // No user names, no ports, nothing but a plain host name.
    if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return false;
    }
    host == "github.com" || host == "api.github.com" || host.ends_with(".githubusercontent.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"; // "test"
    const B: &str = "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"; // "foo"

    #[test]
    fn sums_are_read_from_sha256sum_output() {
        let text = format!(
            "{A}  ferrite-v0.12.0-windows-x64.zip\n{B}  ferrite-v0.12.0-windows-x64-setup.exe\r\n\n{A} *ferrite-v0.12.0-linux-x64.tar.gz\n"
        );
        let sums = parse_sums(&text).unwrap();
        assert_eq!(sums.len(), 3);
        assert_eq!(hex(&expected_sum(&text, "ferrite-v0.12.0-windows-x64.zip").unwrap()), A);
        assert_eq!(hex(&expected_sum(&text, "ferrite-v0.12.0-windows-x64-setup.exe").unwrap()), B);
        assert_eq!(hex(&expected_sum(&text, "ferrite-v0.12.0-linux-x64.tar.gz").unwrap()), A);
        // Upper-case hex is the same sum.
        assert_eq!(expected_sum(&format!("{}  x.zip\n", A.to_uppercase()), "x.zip"), expected_sum(&format!("{A}  x.zip\n"), "x.zip"));
    }

    #[test]
    fn a_file_missing_from_the_sums_is_refused() {
        let text = format!("{A}  ferrite-v0.12.0-linux-x64.tar.gz\n");
        let e = expected_sum(&text, "ferrite-v0.12.0-windows-x64.zip").unwrap_err();
        assert!(e.contains("not listed"), "{e}");
        // A name that only starts the same is a different file.
        assert!(expected_sum(&text, "ferrite-v0.12.0-linux-x64.tar").is_err());
        assert!(expected_sum("", "ferrite-v0.12.0-linux-x64.tar.gz").is_err());
    }

    #[test]
    fn malformed_sums_are_refused() {
        for text in [
            format!("{}  x.zip", &A[..63]),       // short hash
            format!("{}g  x.zip", &A[..63]),      // not hex
            format!("{A} x.zip"),                 // one space
            format!("{A}  "),                     // no name
            "<html>Not Found</html>".to_string(), // an error page
            format!("{A}  x.zip\n{B}  x.zip\n"),  // two different sums
            format!("é{}  x.zip", &A[..62]),      // not ASCII where the hash is
        ] {
            assert!(parse_sums(&text).is_err(), "accepted {text:?}");
        }
        // The same line twice is harmless.
        assert!(parse_sums(&format!("{A}  x.zip\n{A}  x.zip\n")).is_ok());
    }

    #[test]
    fn a_download_must_match_its_sum_and_size() {
        let dir = std::env::temp_dir().join(format!("ferrite-verify-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ferrite-v0.12.0-windows-x64.zip");
        std::fs::write(&file, b"test").unwrap();
        let good = parse_hex(A).unwrap();
        let other = parse_hex(B).unwrap();
        assert_eq!(sha256_file(&file).unwrap(), (good, 4));
        assert_eq!(check_download(&file, 4, &good), Ok(()));
        let e = check_download(&file, 4, &other).unwrap_err();
        assert!(e.contains("does not match"), "{e}");
        let e = check_download(&file, 5, &good).unwrap_err();
        assert!(e.contains("4 bytes, the release says 5"), "{e}");
        assert!(check_download(&dir.join("missing.zip"), 4, &good).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn release_assets_are_read_from_the_api_answer() {
        let json = r#"{"tag_name":"v0.12.0","assets":[
            {"name":"ferrite-v0.12.0-windows-x64.zip","size":21609641,"uploader":{"login":"x"},"browser_download_url":"https://github.com/a"},
            {"name":"SHA256SUMS","size":420}]}"#;
        let assets = parse_assets(json).unwrap();
        assert_eq!(assets[0], Asset { name: "ferrite-v0.12.0-windows-x64.zip".into(), size: 21609641 });
        assert_eq!(assets[1], Asset { name: "SHA256SUMS".into(), size: 420 });
        assert!(parse_assets(r#"{"message":"Not Found"}"#).is_err());
        assert!(parse_assets("rate limited").is_err());
    }

    #[test]
    fn only_https_github_urls_are_allowed() {
        for ok in [
            "https://github.com/Kalaghni/ferrite/releases/download/v0.12.0/SHA256SUMS",
            "https://api.github.com/repos/Kalaghni/ferrite/releases/tags/v0.12.0",
            "https://objects.githubusercontent.com/github-production-release-asset/1?X-Amz=1",
            "https://release-assets.githubusercontent.com/github-production-release-asset/1",
            "https://GitHub.com/x",
        ] {
            assert!(allowed_url(ok), "refused {ok}");
        }
        for bad in [
            "http://github.com/Kalaghni/ferrite/releases/download/v0.12.0/SHA256SUMS",
            "http://objects.githubusercontent.com/x",
            "https://github.com.evil.example/x",
            "https://evil.example/github.com/x",
            "https://githubusercontent.com.evil.example/x",
            "https://evilgithubusercontent.com/x",
            "https://user@github.com/x",
            "https://github.com:8443/x",
            "file:///C:/Windows/notepad.exe",
            "",
        ] {
            assert!(!allowed_url(bad), "allowed {bad}");
        }
    }
}
