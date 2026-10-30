use super::*;
use ed25519_dalek::{Signer, SigningKey};

const TEST_SECRET: [u8; 32] = [7; 32];

fn test_key() -> String {
    let public = SigningKey::from_bytes(&TEST_SECRET).verifying_key();
    base64::engine::general_purpose::STANDARD.encode(public.as_bytes())
}

fn sign(message: &[u8]) -> Vec<u8> {
    SigningKey::from_bytes(&TEST_SECRET)
        .sign(message)
        .to_bytes()
        .to_vec()
}

#[test]
fn versions_compare_as_semver() {
    assert!(is_newer("v0.2.0", "0.1.0"));
    assert!(is_newer("0.10.0", "0.9.3"));
    assert!(is_newer("1.0.0", "1.0.0-beta.2"));
    assert!(!is_newer("v0.1.0", "0.1.0"));
    assert!(!is_newer("0.1.0", "0.2.0"));
    assert!(!is_newer("nightly", "0.1.0"));
}

#[test]
fn releases_are_read_from_githubs_answer() {
    let json = r#"{"tag_name":"v0.3.0","body":" Notes \n","html_url":"https://example.org/r",
        "draft":false,"prerelease":false,
        "assets":[{"name":"SHA256SUMS.txt","browser_download_url":"https://example.org/s","size":10}]}"#;
    let release = parse_release(json).unwrap().unwrap();
    assert_eq!(release.version, "0.3.0");
    assert_eq!(release.notes, "Notes");
    assert_eq!(release.asset(CHECKSUMS).unwrap().size, 10);
    assert!(
        parse_release(&json.replace(r#""prerelease":false"#, r#""prerelease":true"#))
            .unwrap()
            .is_none()
    );
    assert!(parse_release("not json").is_err());
}

#[test]
fn notes_cover_every_version_since_the_installed_one() {
    let json = r#"[
        {"tag_name":"v0.1.12","body":"Twelve","draft":false,"prerelease":false,"assets":[]},
        {"tag_name":"v0.1.13-rc.1","body":"Candidate","draft":false,"prerelease":true,"assets":[]},
        {"tag_name":"v0.1.10","body":"Ten","draft":false,"prerelease":false,"assets":[]},
        {"tag_name":"v0.1.11","body":"Eleven","draft":false,"prerelease":false,"assets":[]},
        {"tag_name":"v0.1.9","body":"Nine","draft":false,"prerelease":false,"assets":[]}
    ]"#;
    let releases = parse_releases(json).unwrap();
    assert_eq!(releases.len(), 4, "pre-releases are left out");
    assert_eq!(
        notes_since(&releases, "0.1.9"),
        "## 0.1.12\n\nTwelve\n\n## 0.1.11\n\nEleven\n\n## 0.1.10\n\nTen"
    );
    assert_eq!(
        notes_since(&releases, "0.1.11"),
        "Twelve",
        "one version needs no heading"
    );
    assert_eq!(notes_since(&releases, "0.1.12"), "");
}

#[test]
fn signatures_from_ci_verify_with_the_built_in_key() {
    // Made with: openssl pkeyutl -sign -rawin -inkey <release key> (as release.yml does).
    let sums = include_bytes!("../tests-data/sample-sums.txt");
    let sig = include_bytes!("../tests-data/sample-sums.txt.sig");
    assert!(verify(sums, sig, PUBLIC_KEY));
    assert!(!verify(b"something else", sig, PUBLIC_KEY));
    assert!(!verify(sums, sig, &test_key()));
    assert!(!verify(sums, &sig[..10], PUBLIC_KEY));
}

fn local_release(dir: &Path, files: &[(&str, &[u8])]) -> Release {
    let assets = files
        .iter()
        .map(|(name, data)| {
            let path = dir.join(name);
            std::fs::write(&path, data).unwrap();
            Asset {
                name: name.to_string(),
                url: format!("file://{}", path.display()),
                size: data.len() as u64,
            }
        })
        .collect();
    Release {
        version: "9.9.9".into(),
        notes: String::new(),
        page: RELEASES_PAGE.into(),
        assets,
    }
}

fn hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn a_package_is_downloaded_only_when_the_signed_checksum_matches() {
    let dir = tempfile::tempdir().unwrap();
    let package = b"the installer".as_slice();
    let sums = format!("{}  setup.exe\n", hex(package));
    let release = local_release(
        dir.path(),
        &[
            ("setup.exe", package),
            (CHECKSUMS, sums.as_bytes()),
            (SIGNATURE, &sign(sums.as_bytes())),
        ],
    );
    let digest = signed_digest(&release, "setup.exe", &test_key()).unwrap();
    let dest = dir.path().join("downloaded.exe");
    let mut seen = 0;
    download(
        release.asset("setup.exe").unwrap(),
        &digest,
        &dest,
        &AtomicBool::new(false),
        |done, total| {
            assert_eq!(total, package.len() as u64);
            seen = done;
        },
    )
    .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), package);
    assert_eq!(seen, package.len() as u64);

    assert!(signed_digest(&release, "setup.exe", PUBLIC_KEY).is_err());
    std::fs::write(dir.path().join("setup.exe"), b"tampered").unwrap();
    let dest = dir.path().join("second.exe");
    let error = download(
        release.asset("setup.exe").unwrap(),
        &digest,
        &dest,
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap_err();
    assert!(error.contains("checksum"), "{error}");
    assert!(!dest.exists() && !dest.with_extension("part").exists());
}

#[test]
fn releases_without_signed_checksums_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let release = local_release(dir.path(), &[("setup.exe", b"x")]);
    assert!(signed_digest(&release, "setup.exe", &test_key()).is_err());
}
