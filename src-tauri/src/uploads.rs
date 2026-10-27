//! Uploads ohne Bytes im JSON (Plan W7, E10).
//!
//! Der Browser lädt eine Datei über `POST /api/upload/{art}` (multipart,
//! gestreamt) nach `<data>/tmp/uploads/<uuid>/datei`; der Server prüft Größe
//! je Art, Typ per Magic Bytes und freien Platz und gibt ein [`UploadTicket`]
//! zurück. Die Commands nehmen nur noch die `uploadId`, lesen die Datei über
//! [`UploadStore::read`] und geben sie nach Erfolg mit [`UploadStore::consume`]
//! frei (ein fehlgeschlagener Versuch kann dieselbe Datei erneut verwenden).
//! Nicht verbrauchte Uploads verfallen nach [`UPLOAD_TTL`], Reste beim
//! Serverstart und durch den stündlichen Sweeper (`<data>/tmp`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use uuid::Uuid;

use crate::downloads::safe_file_name;
use crate::error::{Error, Result};

pub const UPLOAD_TTL: Duration = Duration::from_secs(3600);
const DIR: &str = "uploads";
const MIB: u64 = 1024 * 1024;
/// Höchstzahl offener Uploads (Schutz gegen Volllaufen zwischen zwei Aufräumläufen).
pub const MAX_PENDING: usize = 100;
/// Standard für `KLEIN_BUCH_MIN_FREE_BYTES`: 2 GiB.
pub const DEFAULT_MIN_FREE_BYTES: u64 = 2 * 1024 * MIB;
/// Standard für `KLEIN_BUCH_MAX_RESTORE_BYTES`: 4 GiB.
pub const DEFAULT_MAX_RESTORE_BYTES: u64 = 4 * 1024 * MIB;
pub const ENV_MIN_FREE_BYTES: &str = "KLEIN_BUCH_MIN_FREE_BYTES";
pub const ENV_MAX_RESTORE_BYTES: &str = "KLEIN_BUCH_MAX_RESTORE_BYTES";

/// Upload-Art (`{art}` im Pfad).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UploadKind {
    /// Beleg zu Kosten oder Privatbewegung.
    Receipt,
    /// Anhang an einen Beleg/Kontakt.
    Attachment,
    /// Unterschriebener Vertrag (Angebot annehmen).
    Contract,
    /// Rechtsdokument (AGB, Datenschutz) — nur PDF.
    LegalDocument,
    /// Eingangs-E-Rechnung (XRechnung-XML oder ZUGFeRD-PDF).
    EInvoice,
    /// Firmenlogo.
    Logo,
    /// Unterschrift für Angebote.
    Signature,
    /// Backup-Datei (`.kbk`) für den Restore (W7c).
    Restore,
}

/// Am Inhalt erkannter Dateityp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Pdf,
    Png,
    Jpeg,
    Webp,
    Gif,
    Tiff,
    Xml,
    Svg,
    Backup,
}

impl FileType {
    pub fn mime(self) -> &'static str {
        match self {
            FileType::Pdf => "application/pdf",
            FileType::Png => "image/png",
            FileType::Jpeg => "image/jpeg",
            FileType::Webp => "image/webp",
            FileType::Gif => "image/gif",
            FileType::Tiff => "image/tiff",
            FileType::Xml => "application/xml",
            FileType::Svg => "image/svg+xml",
            FileType::Backup => "application/octet-stream",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            FileType::Pdf => "pdf",
            FileType::Png => "png",
            FileType::Jpeg => "jpg",
            FileType::Webp => "webp",
            FileType::Gif => "gif",
            FileType::Tiff => "tif",
            FileType::Xml => "xml",
            FileType::Svg => "svg",
            FileType::Backup => "kbk",
        }
    }

    fn label(self) -> &'static str {
        match self {
            FileType::Pdf => "PDF",
            FileType::Png => "PNG",
            FileType::Jpeg => "JPEG",
            FileType::Webp => "WebP",
            FileType::Gif => "GIF",
            FileType::Tiff => "TIFF",
            FileType::Xml => "XML (UTF-8)",
            FileType::Svg => "SVG",
            FileType::Backup => "Klein.Buch-Backup (.kbk)",
        }
    }
}

const DOCS: &[FileType] = &[
    FileType::Pdf,
    FileType::Png,
    FileType::Jpeg,
    FileType::Webp,
    FileType::Gif,
    FileType::Tiff,
    FileType::Xml,
];
const IMAGES: &[FileType] = &[
    FileType::Png,
    FileType::Jpeg,
    FileType::Webp,
    FileType::Gif,
    FileType::Svg,
];

impl UploadKind {
    pub fn from_slug(slug: &str) -> Option<Self> {
        Some(match slug {
            "beleg" => UploadKind::Receipt,
            "anhang" => UploadKind::Attachment,
            "vertrag" => UploadKind::Contract,
            "rechtsdokument" => UploadKind::LegalDocument,
            "erechnung" => UploadKind::EInvoice,
            "logo" => UploadKind::Logo,
            "unterschrift" => UploadKind::Signature,
            "restore" => UploadKind::Restore,
            _ => return None,
        })
    }

    pub fn slug(self) -> &'static str {
        match self {
            UploadKind::Receipt => "beleg",
            UploadKind::Attachment => "anhang",
            UploadKind::Contract => "vertrag",
            UploadKind::LegalDocument => "rechtsdokument",
            UploadKind::EInvoice => "erechnung",
            UploadKind::Logo => "logo",
            UploadKind::Signature => "unterschrift",
            UploadKind::Restore => "restore",
        }
    }

    /// Größenlimit je Art (Plan W7).
    pub fn max_bytes(self, limits: &UploadLimits) -> u64 {
        match self {
            UploadKind::Receipt
            | UploadKind::Attachment
            | UploadKind::Contract
            | UploadKind::LegalDocument => 20 * MIB,
            UploadKind::EInvoice => 10 * MIB,
            UploadKind::Logo | UploadKind::Signature => 2 * MIB,
            UploadKind::Restore => limits.max_restore_bytes,
        }
    }

    /// Zulässige Typen je Art.
    pub fn allowed(self) -> &'static [FileType] {
        match self {
            UploadKind::Receipt | UploadKind::Attachment | UploadKind::Contract => DOCS,
            UploadKind::LegalDocument => &[FileType::Pdf],
            UploadKind::EInvoice => &[FileType::Pdf, FileType::Xml],
            UploadKind::Logo | UploadKind::Signature => IMAGES,
            UploadKind::Restore => &[FileType::Backup],
        }
    }

    /// Meldung bei unzulässigem Typ; `whole` (falls gelesen) erlaubt einen
    /// genaueren Hinweis bei XML in falscher Kodierung (R3-006).
    pub fn type_error(self, whole: Option<&[u8]>) -> String {
        if let Some(bytes) = whole {
            let body = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
            let looks_xml = body
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| *b == b'<');
            if looks_xml {
                if let Err(e) = std::str::from_utf8(bytes) {
                    return format!(
                        "Die XML-Datei ist nicht UTF-8-codiert (erster ungültiger Byte bei \
                         Position {}). EN-16931 / KoSIT verlangen UTF-8. Bitte die Datei beim \
                         Absender als UTF-8 neu exportieren.",
                        e.valid_up_to()
                    );
                }
            }
        }
        let list: Vec<&str> = self.allowed().iter().map(|t| t.label()).collect();
        format!(
            "Dateityp nicht erlaubt. Erlaubt sind: {} (geprüft am Inhalt, nicht an der Endung).",
            list.join(", ")
        )
    }
}

/// Grenzen aus der Umgebung (`KLEIN_BUCH_MIN_FREE_BYTES`,
/// `KLEIN_BUCH_MAX_RESTORE_BYTES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadLimits {
    pub min_free_bytes: u64,
    pub max_restore_bytes: u64,
}

impl Default for UploadLimits {
    fn default() -> Self {
        Self {
            min_free_bytes: DEFAULT_MIN_FREE_BYTES,
            max_restore_bytes: DEFAULT_MAX_RESTORE_BYTES,
        }
    }
}

impl UploadLimits {
    /// Liest die Grenzen; ungültige Werte brechen den Start ab.
    pub fn from_env_with(get: &impl Fn(&str) -> Option<std::ffi::OsString>) -> Result<Self> {
        let read = |name: &str, default: u64| -> Result<u64> {
            match get(name).filter(|v| !v.is_empty()) {
                None => Ok(default),
                Some(v) => v.to_string_lossy().trim().parse::<u64>().map_err(|_| {
                    Error::Config(format!(
                        "{name}={} ist keine Byte-Zahl — Start abgebrochen.",
                        v.to_string_lossy()
                    ))
                }),
            }
        };
        Ok(Self {
            min_free_bytes: read(ENV_MIN_FREE_BYTES, DEFAULT_MIN_FREE_BYTES)?,
            max_restore_bytes: read(ENV_MAX_RESTORE_BYTES, DEFAULT_MAX_RESTORE_BYTES)?,
        })
    }
}

/// Erkennt den Typ an den ersten Bytes. `whole` ist der vollständige Inhalt
/// (für XML/SVG: UTF-8-Prüfung); ohne ihn wird XML nicht erkannt.
pub fn sniff(head: &[u8], whole: Option<&[u8]>) -> Option<FileType> {
    if head.starts_with(b"%PDF-") {
        return Some(FileType::Pdf);
    }
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(FileType::Png);
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(FileType::Jpeg);
    }
    if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(FileType::Webp);
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some(FileType::Gif);
    }
    if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        return Some(FileType::Tiff);
    }
    if head.starts_with(crate::backup::manifest::MAGIC.as_bytes()) {
        return Some(FileType::Backup);
    }
    let text = std::str::from_utf8(whole?).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).trim_start();
    if !text.starts_with('<') {
        return None;
    }
    // Wurzelelement: erstes `<name` nach Prolog, Kommentaren und DOCTYPE.
    let mut rest = text;
    loop {
        let start = rest.find('<')?;
        rest = &rest[start..];
        if rest.starts_with("<?") {
            rest = &rest[rest.find("?>")? + 2..];
        } else if rest.starts_with("<!--") {
            rest = &rest[rest.find("-->")? + 3..];
        } else if rest.starts_with("<!") {
            rest = &rest[rest.find('>')? + 1..];
        } else {
            break;
        }
    }
    let name: String = rest[1..]
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/')
        .collect();
    let local = name.rsplit(':').next().unwrap_or("");
    // HTML/XHTML ist kein zulässiges XML (aktive Inhalte im Browser): Wurzel
    // html/body/script, XHTML-Namensraum, `<script`, Zeichenreferenzen in
    // Namensraum-Angaben (Gegenprüfung W7).
    let lower = text.to_ascii_lowercase();
    let suspicious_ns = lower.match_indices("xmlns").any(|(i, _)| {
        lower[i..]
            .chars()
            .take(200)
            .collect::<String>()
            .contains("&#")
    });
    if ["html", "body", "script"]
        .iter()
        .any(|r| local.eq_ignore_ascii_case(r))
        || lower.contains("www.w3.org/1999/xhtml")
        || lower.contains("<script")
        || prefixed_script(&lower)
        || suspicious_ns
    {
        return None;
    }
    // Eingebettetes SVG zählt als SVG (nur für Logo/Unterschrift zulässig).
    if local.eq_ignore_ascii_case("svg") || lower.contains("www.w3.org/2000/svg") {
        Some(FileType::Svg)
    } else if !local.is_empty() {
        Some(FileType::Xml)
    } else {
        None
    }
}

/// `<prefix:script` (Element mit Namensraum-Präfix).
fn prefixed_script(lower: &str) -> bool {
    lower.match_indices(":script").any(|(i, _)| {
        let before = &lower[..i];
        match before.rfind('<') {
            Some(lt) => {
                let prefix = &before[lt + 1..];
                !prefix.is_empty()
                    && prefix
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            }
            None => false,
        }
    })
}

/// Freier Platz (Bytes) auf dem Dateisystem von `path`.
pub fn free_bytes(path: &Path) -> Result<u64> {
    let st = rustix::fs::statvfs(path)
        .map_err(|e| Error::Config(format!("Freier Platz nicht ermittelbar: {e}")))?;
    Ok(st.f_bavail.saturating_mul(st.f_frsize))
}

/// Was der Upload-Endpunkt dem Browser zurückgibt.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UploadTicket {
    pub upload_id: String,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: u64,
}

/// Ein gelesener Upload (oder intern erzeugte Bytes, z. B. Ablage-Ordner, Tests).
#[derive(Clone)]
pub struct UploadedFile {
    pub bytes: Vec<u8>,
    /// Bereinigter Original-Dateiname.
    pub file_name: String,
    /// MIME-Typ aus der Inhaltsprüfung.
    pub mime: &'static str,
    pub file_type: FileType,
}

impl std::fmt::Debug for UploadedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadedFile")
            .field("file_name", &self.file_name)
            .field("mime", &self.mime)
            .field("len", &self.bytes.len())
            .finish()
    }
}

impl UploadedFile {
    /// Prüft Bytes wie ein Upload der Art `kind` (Typ, Größe, nicht leer).
    pub fn from_bytes(kind: UploadKind, file_name: &str, bytes: Vec<u8>) -> Result<Self> {
        if bytes.is_empty() {
            return Err(Error::Domain("Die Datei ist leer.".into()));
        }
        if bytes.len() as u64 > kind.max_bytes(&UploadLimits::default()) {
            return Err(Error::Domain("Die Datei ist zu groß.".into()));
        }
        let head = &bytes[..bytes.len().min(512)];
        let file_type = sniff(head, Some(&bytes))
            .filter(|t| kind.allowed().contains(t))
            .ok_or_else(|| Error::Domain(kind.type_error(Some(&bytes))))?;
        Ok(Self {
            file_name: upload_file_name(file_name, file_type),
            mime: file_type.mime(),
            file_type,
            bytes,
        })
    }
}

/// Bereinigter Dateiname; ohne Namen `datei.<ext>`.
pub fn upload_file_name(name: &str, file_type: FileType) -> String {
    safe_file_name(name, &format!("datei.{}", file_type.ext()))
}

#[derive(Debug, Clone)]
struct UploadEntry {
    path: PathBuf,
    kind: UploadKind,
    file_name: String,
    file_type: FileType,
    size: u64,
    expires: Instant,
}

/// Ablage der Uploads (im `AppCtx`).
pub struct UploadStore {
    root: PathBuf,
    limits: Mutex<UploadLimits>,
    entries: Mutex<HashMap<Uuid, UploadEntry>>,
}

impl UploadStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("tmp").join(DIR),
            limits: Mutex::new(UploadLimits::default()),
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, UploadEntry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn limits(&self) -> UploadLimits {
        *self.limits.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn set_limits(&self, limits: UploadLimits) {
        *self.limits.lock().unwrap_or_else(PoisonError::into_inner) = limits;
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Neuer Upload-Platz: ID und Zieldatei. `Err`, wenn zu viele offen sind.
    pub fn begin(&self) -> Result<(Uuid, PathBuf)> {
        self.sweep();
        if self.lock().len() >= MAX_PENDING {
            return Err(Error::Domain(
                "Zu viele offene Uploads — bitte später erneut versuchen.".into(),
            ));
        }
        let id = Uuid::new_v4();
        let dir = self.root.join(id.simple().to_string());
        std::fs::create_dir_all(&dir)?;
        Ok((id, dir.join("datei")))
    }

    /// Registriert eine fertig geschriebene, geprüfte Datei.
    pub fn register(
        &self,
        id: Uuid,
        path: &Path,
        kind: UploadKind,
        file_name: &str,
        file_type: FileType,
        size: u64,
    ) -> UploadTicket {
        let file_name = upload_file_name(file_name, file_type);
        self.lock().insert(
            id,
            UploadEntry {
                path: path.to_path_buf(),
                kind,
                file_name: file_name.clone(),
                file_type,
                size,
                expires: Instant::now() + UPLOAD_TTL,
            },
        );
        UploadTicket {
            upload_id: id.to_string(),
            file_name,
            mime_type: file_type.mime().to_string(),
            size_bytes: size,
        }
    }

    /// Schreibt Bytes als Upload der Art `kind` (Tests, interne Aufrufer);
    /// gleiche Prüfungen wie der Endpunkt.
    pub fn put_bytes(
        &self,
        kind: UploadKind,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<UploadTicket> {
        let checked = UploadedFile::from_bytes(kind, file_name, bytes.to_vec())?;
        let (id, path) = self.begin()?;
        std::fs::write(&path, bytes)?;
        Ok(self.register(
            id,
            &path,
            kind,
            &checked.file_name,
            checked.file_type,
            bytes.len() as u64,
        ))
    }

    fn entry(&self, id: &str, kinds: &[UploadKind]) -> Result<UploadEntry> {
        self.sweep();
        let missing = || {
            Error::Domain(
                "Upload nicht gefunden oder abgelaufen — bitte die Datei erneut auswählen.".into(),
            )
        };
        let uuid = Uuid::parse_str(id).map_err(|_| missing())?;
        let entry = self.lock().get(&uuid).cloned().ok_or_else(missing)?;
        if !kinds.contains(&entry.kind) {
            return Err(Error::Domain(format!(
                "Upload der Art „{}“ passt hier nicht.",
                entry.kind.slug()
            )));
        }
        Ok(entry)
    }

    /// Liest einen Upload (bleibt bis [`Self::consume`] bzw. Ablauf liegen).
    pub fn read(&self, id: &str, kinds: &[UploadKind]) -> Result<UploadedFile> {
        let entry = self.entry(id, kinds)?;
        let bytes = std::fs::read(&entry.path)?;
        if bytes.len() as u64 != entry.size {
            return Err(Error::Domain(
                "Upload unvollständig — bitte erneut hochladen.".into(),
            ));
        }
        Ok(UploadedFile {
            bytes,
            file_name: entry.file_name,
            mime: entry.file_type.mime(),
            file_type: entry.file_type,
        })
    }

    /// Pfad, Größe und Dateiname eines Uploads, ohne ihn zu lesen (Restore).
    pub fn path_of(&self, id: &str, kinds: &[UploadKind]) -> Result<(PathBuf, u64, String)> {
        let entry = self.entry(id, kinds)?;
        Ok((entry.path, entry.size, entry.file_name))
    }

    /// Gibt einen verbrauchten Upload frei (löscht die Datei).
    pub fn consume(&self, id: &str) {
        let Ok(uuid) = Uuid::parse_str(id) else {
            return;
        };
        let removed = self.lock().remove(&uuid);
        if let Some(entry) = removed {
            Self::remove_files(&entry.path);
        }
    }

    /// Löscht das Verzeichnis eines Uploads.
    pub fn remove_files(path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Entfernt abgelaufene Einträge samt Dateien, dazu nicht registrierte
    /// Verzeichnisse (abgebrochene Vorgänge), die älter als [`UPLOAD_TTL`] sind.
    pub fn sweep(&self) {
        let now = Instant::now();
        let (expired, known): (Vec<PathBuf>, Vec<String>) = {
            let mut map = self.lock();
            let gone: Vec<Uuid> = map
                .iter()
                .filter(|(_, e)| e.expires <= now)
                .map(|(id, _)| *id)
                .collect();
            let expired = gone
                .iter()
                .filter_map(|id| map.remove(id).map(|e| e.path))
                .collect();
            (
                expired,
                map.keys().map(|id| id.simple().to_string()).collect(),
            )
        };
        for path in expired {
            Self::remove_files(&path);
        }
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > UPLOAD_TTL);
            if old && !known.contains(&name) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// Alle Einträge und Dateien löschen (beim Sperren der Datenbank).
    pub fn clear(&self) {
        self.lock().clear();
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// Anzahl offener Uploads.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_detects_by_content() {
        assert_eq!(sniff(b"%PDF-1.7\n", None), Some(FileType::Pdf));
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n....", None), Some(FileType::Png));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0], None), Some(FileType::Jpeg));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 ", None), Some(FileType::Webp));
        assert_eq!(sniff(b"GIF89a..", None), Some(FileType::Gif));
        assert_eq!(sniff(b"II*\0....", None), Some(FileType::Tiff));
        let xml = "\u{feff}<?xml version=\"1.0\"?>\n<!-- x --><rsm:CrossIndustryInvoice/>";
        assert_eq!(
            sniff(xml.as_bytes(), Some(xml.as_bytes())),
            Some(FileType::Xml)
        );
        let svg =
            "<?xml version=\"1.0\"?><!DOCTYPE svg><svg xmlns=\"http://www.w3.org/2000/svg\"/>";
        assert_eq!(
            sniff(svg.as_bytes(), Some(svg.as_bytes())),
            Some(FileType::Svg)
        );
        // HTML mit .pdf-Endung, Nicht-UTF-8-XML, Text
        assert_eq!(sniff(b"<html><script>", None), None);
        let html = b"<!DOCTYPE html><HTML><body/></HTML>";
        assert_eq!(sniff(html, Some(html)), None);
        let xhtml = b"<?xml version=\"1.0\"?><x:html xmlns:x=\"http://www.w3.org/1999/xhtml\"/>";
        assert_eq!(sniff(xhtml, Some(xhtml)), None);
        let embedded = b"<a xmlns:h=\"http://www.w3.org/1999/xhtml\"><h:script/></a>";
        assert_eq!(sniff(embedded, Some(embedded)), None);
        // Gegenprüfung W7: Zeichenreferenz im Namensraum, HTML-Fragment, SVG
        let charref = b"<a xmlns:h=\"http://www.w3.org/1999/&#120;html\"><h:x/></a>";
        assert_eq!(sniff(charref, Some(charref)), None);
        let body = b"<!DOCTYPE html><body><p>x</p></body>";
        assert_eq!(sniff(body, Some(body)), None);
        let inner_svg = b"<a xmlns:s=\"http://www.w3.org/2000/svg\"><s:svg/></a>";
        assert_eq!(sniff(inner_svg, Some(inner_svg)), Some(FileType::Svg));
        let einvoice = b"<?xml version=\"1.0\"?><rsm:CrossIndustryInvoice xmlns:rsm=\"urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100\"/>";
        assert_eq!(sniff(einvoice, Some(einvoice)), Some(FileType::Xml));
        let latin1 = b"<?xml version=\"1.0\"?><a>M\xfcller</a>";
        assert_eq!(sniff(latin1, Some(latin1)), None);
        assert_eq!(sniff(b"hallo", Some(b"hallo")), None);
    }

    #[test]
    fn kinds_have_limits_and_types() {
        let l = UploadLimits::default();
        assert_eq!(UploadKind::Receipt.max_bytes(&l), 20 * MIB);
        assert_eq!(UploadKind::EInvoice.max_bytes(&l), 10 * MIB);
        assert_eq!(UploadKind::Logo.max_bytes(&l), 2 * MIB);
        assert_eq!(UploadKind::Restore.max_bytes(&l), 4 * 1024 * MIB);
        assert!(UploadKind::Logo.allowed().contains(&FileType::Svg));
        assert!(!UploadKind::Receipt.allowed().contains(&FileType::Svg));
        assert_eq!(UploadKind::LegalDocument.allowed(), &[FileType::Pdf]);
        for slug in [
            "beleg",
            "anhang",
            "vertrag",
            "rechtsdokument",
            "erechnung",
            "logo",
            "unterschrift",
            "restore",
        ] {
            assert_eq!(UploadKind::from_slug(slug).unwrap().slug(), slug);
        }
        assert!(UploadKind::from_slug("../beleg").is_none());
    }

    #[test]
    fn limits_from_env() {
        let none = |_: &str| None;
        assert_eq!(
            UploadLimits::from_env_with(&none).unwrap(),
            UploadLimits::default()
        );
        let bad = |n: &str| (n == ENV_MIN_FREE_BYTES).then(|| "2G".into());
        assert!(UploadLimits::from_env_with(&bad).is_err());
        let ok = |n: &str| (n == ENV_MAX_RESTORE_BYTES).then(|| "1048576".into());
        assert_eq!(
            UploadLimits::from_env_with(&ok).unwrap().max_restore_bytes,
            MIB
        );
    }

    #[test]
    fn store_reads_until_consumed() {
        let dir = tempfile::tempdir().unwrap();
        let store = UploadStore::new(dir.path());
        let t = store
            .put_bytes(
                UploadKind::Receipt,
                "../x/\"Beleg\"\r\n.pdf",
                b"%PDF-1.7 test",
            )
            .unwrap();
        assert_eq!(t.file_name, "Beleg.pdf");
        assert_eq!(t.mime_type, "application/pdf");
        let f = store.read(&t.upload_id, &[UploadKind::Receipt]).unwrap();
        assert_eq!(f.bytes, b"%PDF-1.7 test");
        assert!(
            store.read(&t.upload_id, &[UploadKind::Logo]).is_err(),
            "falsche Art"
        );
        assert!(
            store.read(&t.upload_id, &[UploadKind::Receipt]).is_ok(),
            "bleibt liegen"
        );
        store.consume(&t.upload_id);
        assert!(store.read(&t.upload_id, &[UploadKind::Receipt]).is_err());
        assert!(store.is_empty());
        assert!(store
            .put_bytes(UploadKind::Receipt, "x.pdf", b"<html>")
            .is_err());
        assert!(store.read("kein-uuid", &[UploadKind::Receipt]).is_err());
    }
}
