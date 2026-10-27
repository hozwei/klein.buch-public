//! tasks:W8-TEST-ADDRESSES (Lauf F7a): Tests nutzen nur Dokumentations-Adressen.
//!
//! Testdateien werden mit dem öffentlichen Repo veröffentlicht. Darum dürfen
//! Mailadressen, Hostnamen und IP-Adressen in Tests nur aus den dafür
//! reservierten Bereichen kommen: Domains nach RFC 2606/6761 (`example.org`,
//! `example.com`, `example.net` samt Subdomains, TLDs `.example`, `.invalid`,
//! `.test`, `.localhost` und die IDN-Test-TLD `xn--zckzah`) und IPv4 aus den
//! Dokumentationsnetzen (RFC 5737) bzw. nicht routbaren Bereichen (Loopback,
//! privat, Link-Local, …), die SSRF-Tests brauchen.
//!
//! Die Prüfung ist positiv: Jede gefundene Adresse muss in einen erlaubten
//! Bereich fallen; eine Liste verbotener Namen gibt es bewusst nicht.
//! Geprüft werden `src-tauri/tests/**`, die Testmodule in `src-tauri/src/**`
//! (ab der ersten Zeile `#[cfg(test)]`) und die Frontend-Tests (`src/**/*.test.ts`,
//! `tests/**`). Eine Zeile mit `testadresse-ausnahme` (samt Begründung) wird
//! übersprungen.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

/// Echte Hosts, die Tests als Namensraum oder Ziel bewusst nennen (keine
/// Testadressen): XML-Namensräume, Projektseiten, die Versionsprüfung.
const REAL_HOSTS_ALLOWED: &[&str] = &[
    "www.w3.org",
    "github.com",
    "api.github.com",
    "www.xoev.de",
    "svelte.dev",
];

/// TLDs, bei denen ein String-Literal wie `"host.tld"` als Hostname gilt.
const HOST_TLDS: &[&str] = &[
    "de", "com", "net", "org", "eu", "io", "info", "at", "ch", "biz", "dev", "app",
];

fn domain_allowed(domain: &str) -> bool {
    let d = domain.trim_end_matches('.').to_ascii_lowercase();
    for base in ["example.org", "example.com", "example.net"] {
        if d == base || d.ends_with(&format!(".{base}")) {
            return true;
        }
    }
    let tld = d.rsplit('.').next().unwrap_or("");
    matches!(
        tld,
        "example" | "invalid" | "test" | "localhost" | "xn--zckzah"
    ) || d == "localhost"
}

fn ipv4_allowed(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        // 100.64.0.0/10 (CGNAT), 198.18.0.0/15 (Benchmark), 240.0.0.0/4 (reserviert)
        || (o[0] == 100 && (64..128).contains(&o[1]))
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
        || o[0] >= 240
}

fn is_tld(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    (s.len() >= 2 && s.chars().all(|c| c.is_ascii_alphabetic()))
        || (lower.starts_with("xn--")
            && lower.len() > 4
            && lower.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !lower.ends_with('-'))
}

fn is_domain_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

/// Mailadressen einer Zeile (`local@domain`, TLD aus Buchstaben oder IDN-A-Label).
fn emails(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        if *c != '@' {
            continue;
        }
        let mut s = i;
        while s > 0 && (chars[s - 1].is_ascii_alphanumeric() || "._%+-".contains(chars[s - 1])) {
            s -= 1;
        }
        let mut e = i + 1;
        while e < chars.len() && is_domain_char(chars[e]) {
            e += 1;
        }
        if s == i || e == i + 1 {
            continue;
        }
        let domain: String = chars[i + 1..e].iter().collect();
        let domain = domain.trim_end_matches('.');
        if domain.ends_with('-') {
            continue;
        }
        let Some((_, tld)) = domain.rsplit_once('.') else {
            continue;
        };
        if is_tld(tld) {
            let local: String = chars[s..i].iter().collect();
            out.push(format!("{local}@{domain}"));
        }
    }
    out
}

/// Hosts aus URLs einer Zeile (`scheme://[user@]host[:port]/…`).
fn url_hosts(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (idx, _) in line.match_indices("://") {
        let rest = &line[idx + 3..];
        let end = rest
            .find(|c: char| "/\"' )<>\\?#`,;]".contains(c) || c.is_whitespace())
            .unwrap_or(rest.len());
        let authority = &rest[..end];
        let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host = host.split(':').next().unwrap_or("");
        if host.is_empty() || host.contains(['{', '$', '%', '[']) || !host.contains('.') {
            continue;
        }
        out.push(host.to_string());
    }
    out
}

/// IPv4-Literale einer Zeile (vier Zahlen 0–255, nicht Teil einer längeren Folge).
fn ipv4s(line: &str) -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    for token in line.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 4 || parts.iter().any(|p| p.is_empty() || p.len() > 3) {
            continue;
        }
        if let Ok(ip) = token.parse::<Ipv4Addr>() {
            out.push(ip);
        }
    }
    out
}

/// String-Literale, die nur aus einem Hostnamen mit gängiger TLD bestehen (`"smtp.host.tld"`).
fn literal_hosts(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut parts = line.split('"');
    parts.next();
    // jedes zweite Stück liegt zwischen Anführungszeichen
    while let Some(inner) = parts.next() {
        parts.next();
        if inner.contains('.')
            && !inner.starts_with(['.', '-'])
            && inner.chars().all(is_domain_char)
            && inner
                .rsplit('.')
                .next()
                .is_some_and(|t| HOST_TLDS.contains(&t.to_ascii_lowercase().as_str()))
        {
            out.push(inner.to_string());
        }
    }
    out
}

fn line_findings(line: &str) -> Vec<String> {
    if line.contains("testadresse-ausnahme") {
        return Vec::new();
    }
    let mut bad = Vec::new();
    for mail in emails(line) {
        let domain = mail.rsplit_once('@').map(|(_, d)| d).unwrap_or("");
        if !domain_allowed(domain) {
            bad.push(format!("Mailadresse {mail}"));
        }
    }
    for host in url_hosts(line).into_iter().chain(literal_hosts(line)) {
        if let Ok(ip) = host.parse::<Ipv4Addr>() {
            if !ipv4_allowed(ip) {
                bad.push(format!("IP {ip}"));
            }
        } else if !domain_allowed(&host)
            && !REAL_HOSTS_ALLOWED.contains(&host.to_ascii_lowercase().as_str())
        {
            bad.push(format!("Host {host}"));
        }
    }
    for ip in ipv4s(line) {
        if !ipv4_allowed(ip) {
            bad.push(format!("IP {ip}"));
        }
    }
    bad
}

fn collect(dir: &Path, filter: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if name != "node_modules" && name != "target" {
                collect(&path, filter, out);
            }
        } else if filter(&path) {
            out.push(path);
        }
    }
}

#[test]
fn scanner_recognises_addresses() {
    // Negativbeispiele zusammengesetzt, damit keine vollständige fremde Adresse
    // im veröffentlichten Text steht (und die Datei sich selbst nicht meldet).
    let mail = concat!("info@firma", ".de");
    assert_eq!(
        line_findings(&format!(r#"email: "{mail}".into(),"#)),
        vec![format!("Mailadresse {mail}")]
    );
    assert!(line_findings(r#"email: "info@kunde.example.org".into(),"#).is_empty());
    assert!(line_findings(r#"to: "a@b.invalid", host: "x.test""#).is_empty());
    let web = concat!("www.firma", ".com");
    assert_eq!(
        line_findings(&format!(r#"url: "https://{web}/pfad","#)),
        vec![format!("Host {web}")]
    );
    assert!(line_findings(r#"url: "sftp://kb@backup.example.org:22/x""#).is_empty());
    let smtp = concat!("smtp.firma", ".de");
    assert_eq!(
        line_findings(&format!(r#"host: "{smtp}","#)),
        vec![format!("Host {smtp}")]
    );
    let ip = concat!("8.8", ".8.8");
    assert_eq!(
        line_findings(&format!("let ip = \"{ip}\";")),
        vec![format!("IP {ip}")]
    );
    assert!(line_findings("let ip = \"192.0.2.10\"; let b = \"10.0.0.1\";").is_empty());
    assert!(line_findings("let v = \"0.15.0\"; datei = \"default.typ\";").is_empty());
    assert!(line_findings("format!(\"{user}@{host}\")").is_empty());
    assert!(line_findings("assert!(!is_valid_email(\"info@example.xn--\"));").is_empty());
    assert!(line_findings(&format!("x(\"{mail}\") // testadresse-ausnahme: Beispiel")).is_empty());
}

#[test]
fn tests_use_only_documentation_addresses() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = crate_dir.join("..").canonicalize().expect("Repo-Wurzel");
    let mut files = Vec::new();
    collect(
        &crate_dir.join("tests"),
        &|p| p.extension().is_some_and(|e| e == "rs"),
        &mut files,
    );
    let tests_end = files.len();
    collect(
        &crate_dir.join("src"),
        &|p| p.extension().is_some_and(|e| e == "rs"),
        &mut files,
    );
    let src_end = files.len();
    collect(
        &root.join("src"),
        &|p| p.to_string_lossy().ends_with(".test.ts"),
        &mut files,
    );
    collect(&root.join("tests"), &|_| true, &mut files);
    assert!(tests_end > 50, "Testdateien gefunden: {tests_end}");
    assert!(
        src_end - tests_end > 50,
        "Quelldateien gefunden: {}",
        src_end - tests_end
    );

    let mut problems = Vec::new();
    for (n, path) in files.iter().enumerate() {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        // In src-tauri/src nur die Testmodule (ab der ersten `#[cfg(test)]`-Zeile)
        let start = if (tests_end..src_end).contains(&n) {
            match text.lines().position(|l| l.trim() == "#[cfg(test)]") {
                Some(i) => i,
                None => continue,
            }
        } else {
            0
        };
        for (i, line) in text.lines().enumerate().skip(start) {
            for finding in line_findings(line) {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                problems.push(format!("{}:{}: {finding}", rel.display(), i + 1));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "Tests dürfen nur Dokumentations-Adressen nutzen (example.org/.com/.net, \
         .example/.invalid/.test, 192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24):\n{}",
        problems.join("\n")
    );
}
