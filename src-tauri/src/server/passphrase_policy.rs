//! Prüfung **neuer** Passphrasen (ASVS V2.1.2/V2.1.7, NIST SP 800-63B
//! §5.1.1.2; Lauf F4a). Reine Funktionen ohne I/O.
//!
//! Gilt bei der Einrichtung und beim Passphrase-Wechsel; bestehende
//! Passphrasen bleiben gültig (Anmeldung, Re-Auth und Restore prüfen nur
//! gegen die Datenbank bzw. das Backup). Kein Netzabruf (local-first).
//!
//! Abgelehnt wird (nach der Mindestlänge in `server::auth`):
//! - länger als [`MAX_CHARS`] Zeichen (Obergrenze gegen Missbrauch; die KDF
//!   verarbeitet jede Länge, ASVS V2.1.2 verlangt ≥ 64 erlaubte Zeichen),
//! - ein Eintrag der eingebetteten Sperrliste (`passphrase_blocklist.txt`) oder
//!   eine Passphrase, die ein bekanntes Beispiel enthält (z. B. das Beispiel
//!   der Anmelde-Maske),
//! - triviale Muster: höchstens drei verschiedene Zeichen, eine Wiederholung
//!   (`passwortpasswort`), überwiegend Tastatur-, Buchstaben- oder
//!   Zahlenfolgen,
//! - was nach Abzug von Kontextwörtern (Anwendung, „Passwort“, Firmenname und
//!   E-Mail aus dem Profil) übrig bleibt, ist kürzer als [`MIN_REMAINDER`]
//!   Zeichen oder trivial (`Klein.Buch-Passwort-2026`, `Firma GmbH 2026!`).
//!
//! Verglichen wird immer die normalisierte Form: Kleinbuchstaben, nur
//! Buchstaben und Ziffern.

use std::collections::HashSet;
use std::sync::LazyLock;

/// Höchstlänge neuer Passphrasen in Zeichen.
pub const MAX_CHARS: usize = 1024;
/// Mindestlänge des Rests nach Abzug der Kontextwörter (normalisiert).
pub const MIN_REMAINDER: usize = 8;
/// Anteil aufeinanderfolgender Zeichenpaare in einer Folge (Tastatur,
/// Alphabet, Ziffern), ab dem eine Passphrase als trivial gilt.
const SEQUENCE_SHARE: f64 = 0.8;

/// Warum eine neue Passphrase abgelehnt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weakness {
    TooLong,
    /// Sperrliste bzw. bekanntes Beispiel.
    Common,
    /// Wiederholung, Folge, zu wenige verschiedene Zeichen.
    Trivial,
    /// Im Wesentlichen Anwendungs-, Firmen- oder E-Mail-Name.
    Context,
}

static BLOCKLIST: LazyLock<HashSet<String>> = LazyLock::new(|| {
    include_str!("passphrase_blocklist.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(normalize)
        .filter(|l| !l.is_empty())
        .collect()
});

/// Bekannte Beispiele, die auch als Teil nicht vorkommen dürfen.
const EXAMPLES: &[&str] = &[
    "korrektpferdbatterieklammer",
    "korrektpferdbatterieheftklammer",
    "correcthorsebatterystaple",
];

/// Kontextwörter der Anwendung (normalisiert).
const CONTEXT_WORDS: &[&str] = &[
    "kleinbuch",
    "buchhaltung",
    "datenpasswort",
    "passwort",
    "password",
    "passphrase",
    "kennwort",
    "geheim",
    "secret",
];

/// Tastaturreihen und -spalten, Alphabet und Ziffern (normalisiert); zwei
/// Zeichen gelten als Folge, wenn sie hier vorwärts oder rückwärts
/// nebeneinander stehen.
const RUNS: &[&str] = &[
    "01234567890",
    "abcdefghijklmnopqrstuvwxyza",
    "qwertzuiopüasdfghjklöäyxcvbnm",
    "qwertyuiopasdfghjklzxcvbnm",
    "azertyuiopqsdfghjklmwxcvbn",
    "1qay2wsx3edc4rfv5tgb6zhn7ujm8ik9ol0pöüä",
    "1qaz2wsx3edc4rfv5tgb6yhn7ujm8ik9ol0p",
    "1azq2wsx3edc4rfv5tgb6yhn7ujm8ik9ol0pm",
];

static ADJACENT: LazyLock<HashSet<(char, char)>> = LazyLock::new(|| {
    let mut set = HashSet::new();
    for run in RUNS {
        let chars: Vec<char> = run.chars().collect();
        for pair in chars.windows(2) {
            set.insert((pair[0], pair[1]));
            set.insert((pair[1], pair[0]));
        }
    }
    set
});

/// Kleinbuchstaben, nur Buchstaben und Ziffern.
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Trivial: höchstens drei verschiedene Zeichen, eine Wiederholung eines
/// kürzeren Stücks oder überwiegend Folgen.
fn trivial(norm: &str) -> bool {
    let chars: Vec<char> = norm.chars().collect();
    let n = chars.len();
    if n == 0 {
        return true;
    }
    let distinct: HashSet<char> = chars.iter().copied().collect();
    if distinct.len() <= 3 {
        return true;
    }
    let periodic = (1..=n / 2).any(|p| (p..n).all(|i| chars[i] == chars[i - p]));
    if periodic {
        return true;
    }
    if n >= 2 {
        let sequential = chars
            .windows(2)
            .filter(|w| w[0] == w[1] || ADJACENT.contains(&(w[0], w[1])))
            .count();
        if sequential as f64 / (n - 1) as f64 >= SEQUENCE_SHARE {
            return true;
        }
    }
    false
}

/// Kontextbegriffe aus dem Profil: ganzer Name, Wörter des Namens ab vier
/// Zeichen, E-Mail ganz, ihr lokaler Teil und die Domain ohne Endung.
pub fn profile_context(name: &str, email: &str) -> Vec<String> {
    let mut out = vec![normalize(name), normalize(email)];
    out.extend(
        name.split(|c: char| !c.is_alphanumeric())
            .map(normalize)
            .filter(|w| w.chars().count() >= 4),
    );
    if let Some((local, domain)) = email.split_once('@') {
        out.push(normalize(local));
        let host = domain.rsplit_once('.').map_or(domain, |(h, _)| h);
        out.push(normalize(host));
    }
    out.retain(|w| w.chars().count() >= 4);
    out.sort_by_key(|w| std::cmp::Reverse(w.chars().count()));
    out.dedup();
    out
}

/// Prüft eine neue Passphrase (Mindestlänge vorher). `context` sind weitere
/// normalisierte Begriffe (z. B. aus [`profile_context`]).
pub fn check(passphrase: &str, context: &[String]) -> Result<(), Weakness> {
    if passphrase.chars().count() > MAX_CHARS {
        return Err(Weakness::TooLong);
    }
    let norm = normalize(passphrase);
    if BLOCKLIST.contains(&norm) || EXAMPLES.iter().any(|e| norm.contains(e)) {
        return Err(Weakness::Common);
    }
    if trivial(&norm) {
        return Err(Weakness::Trivial);
    }
    // Längere Begriffe zuerst entfernen (Firmenname vor seinen Wörtern).
    let mut words: Vec<&str> = CONTEXT_WORDS.to_vec();
    words.extend(context.iter().map(String::as_str));
    words.sort_by_key(|w| std::cmp::Reverse(w.chars().count()));
    let mut rest = norm.clone();
    for w in words.iter().filter(|w| !w.is_empty()) {
        rest = rest.replace(w, "");
    }
    if rest != norm && (rest.chars().count() < MIN_REMAINDER || trivial(&rest)) {
        return Err(Weakness::Context);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocklist_is_loaded_and_normalized() {
        assert!(BLOCKLIST.len() > 200, "{}", BLOCKLIST.len());
        assert!(BLOCKLIST
            .iter()
            .all(|e| e.chars().all(char::is_alphanumeric) && *e == e.to_lowercase()));
    }

    #[test]
    fn common_and_examples_are_rejected() {
        for p in [
            "Correct Horse Battery Staple",
            "korrekt-pferd-batterie-klammer",
            "korrekt-pferd-batterie-klammer-2026",
            "Ich liebe dich für immer!",
            "Das ist ein langes Passwort",
            "the quick brown fox jumps over the lazy dog",
        ] {
            assert_eq!(check(p, &[]), Err(Weakness::Common), "{p}");
        }
    }

    #[test]
    fn trivial_patterns_are_rejected() {
        for p in [
            "aaaaaaaaaaaaaaaa",
            "abababababababab",
            "!!!!!!!!!!!!!!!!",
            "1234567890123456",
            "0987654321098765",
            "abcdefghijklmnop",
            "qwertzuiopasdfgh",
            "QWERTYUIOP123456",
            "1qay2wsx3edc4rfv5tgb",
            "hundhundhundhund",
            "sommer-sommer-sommer",
        ] {
            assert_eq!(check(p, &[]), Err(Weakness::Trivial), "{p}");
        }
    }

    #[test]
    fn context_words_need_a_real_remainder() {
        let ctx = profile_context("Anna Schmidt Fotografie", "info@schmidt-foto.example.org");
        for p in [
            "Klein.Buch-Passwort-2026",
            "passwort12345678",
            "Anna Schmidt Fotografie 2026!",
            "fotografie-schmidt-anna-1",
            "info@schmidt-foto.example.org",
        ] {
            assert!(check(p, &ctx).is_err(), "{p}");
        }
        assert_eq!(
            check("Anna Schmidt Fotografie 2026!", &ctx),
            Err(Weakness::Context)
        );
        // Ein echter Rest reicht, auch mit Kontextwort darin.
        for p in [
            "schmidt-tinte-gletscher-moped",
            "neue-lange-passphrase-zum-testen",
            "ganz neue lange Passphrase 2026",
        ] {
            assert_eq!(check(p, &ctx), Ok(()), "{p}");
        }
    }

    #[test]
    fn good_passphrases_pass_and_length_is_capped() {
        for p in [
            "tinte-gletscher-moped-ahorn",
            "Wolke Fahrrad Zimt Leuchtturm",
            "anderes-pferd-batterie-klammer",
            "smoke-pferd-batterie-klammer-2026",
            "Mühle Öl Straße Würfel Kiefer",
        ] {
            assert_eq!(check(p, &[]), Ok(()), "{p}");
        }
        let long = "wort ".repeat(MAX_CHARS / 5 + 1);
        assert_eq!(check(&long, &[]), Err(Weakness::TooLong));
        let ok_long = "Gletscher Moped ".repeat(60);
        assert!(ok_long.chars().count() <= MAX_CHARS);
        // Wiederholung bleibt Wiederholung, auch lang.
        assert_eq!(check(&ok_long, &[]), Err(Weakness::Trivial));
    }
}
