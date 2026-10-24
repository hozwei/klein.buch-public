#!/usr/bin/env python3
"""Smoke-Test des Klein.Buch-Images über HTTP (Plan W11, Image-CI).

Gegen einen laufenden Container (KLEIN_BUCH_INSECURE_HTTP=1, Adresse aus
SMOKE_URL): Einrichtung mit Setup-Token + TOTP, Verkäufer und Kunde anlegen,
Rechnung festschreiben (echter KoSIT + Mustang), ungültige XRechnung beim
Rechnungseingang abgelehnt, ZUGFeRD-PDF beim Rechnungseingang extrahiert.
Nur Standardbibliothek. Aufruf: image_smoke.py <setup-token>
"""

import base64
import hashlib
import hmac
import json
import os
import struct
import sys
import time
import urllib.error
import urllib.request
import uuid

URL = os.environ.get("SMOKE_URL", "http://localhost:8080")
PASS = "smoke-pferd-batterie-klammer-2026"
COOKIE = "__Host-kb_session"


def fail(msg):
    print(f"SMOKE FEHLER: {msg}", flush=True)
    sys.exit(1)


def request(method, path, body=None, cookie=None, headers=None, raw=False):
    data = None
    h = {"X-KleinBuch-Request": "1"}
    if headers:
        h.update(headers)
    if body is not None and not isinstance(body, (bytes, bytearray)):
        data = json.dumps(body).encode()
        h["Content-Type"] = "application/json"
    elif body is not None:
        data = bytes(body)
    if cookie:
        h["Cookie"] = f"{COOKIE}={cookie}"
    req = urllib.request.Request(URL + path, data=data, headers=h, method=method)
    try:
        with urllib.request.urlopen(req, timeout=180) as r:
            payload = r.read()
            return r.status, r.headers, payload if raw else (json.loads(payload) if payload else None)
    except urllib.error.HTTPError as e:
        payload = e.read()
        try:
            parsed = json.loads(payload)
        except ValueError:
            parsed = payload.decode(errors="replace")
        return e.code, e.headers, parsed


def session_from(headers):
    for v in headers.get_all("Set-Cookie") or []:
        first = v.split(";", 1)[0]
        if first.startswith(COOKIE + "="):
            return first.split("=", 1)[1]
    fail("kein Sitzungs-Cookie")


def totp(secret_b32, step=None):
    key = base64.b32decode(secret_b32.upper() + "=" * (-len(secret_b32) % 8))
    step = int(time.time() // 30) if step is None else step
    h = hmac.new(key, struct.pack(">Q", step), hashlib.sha1).digest()
    o = h[-1] & 0x0F
    code = (struct.unpack(">I", h[o:o + 4])[0] & 0x7FFFFFFF) % 1_000_000
    return f"{code:06d}"


def cmd(name, args, cookie):
    status, _, body = request(
        "POST", f"/api/cmd/{name}", args, cookie, {"Idempotency-Key": str(uuid.uuid4())}
    )
    if status == 202:
        job = body["jobId"]
        for _ in range(600):
            s, _, b = request("GET", f"/api/jobs/{job}", cookie=cookie)
            if s != 200:
                fail(f"{name}: Job-Abfrage {s} {b}")
            if b.get("state") != "running":
                if b.get("state") != "done":
                    fail(f"{name}: {b}")
                return b.get("result")
            time.sleep(0.5)
        fail(f"{name}: Job läuft zu lange")
    if status != 200:
        fail(f"{name}: {status} {body}")
    return body


def upload(art, name, data, cookie):
    boundary = "kb-smoke-" + uuid.uuid4().hex
    body = (
        f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n"
        f"Content-Type: application/octet-stream\r\n\r\n"
    ).encode() + data + f"\r\n--{boundary}--\r\n".encode()
    status, _, b = request(
        "POST", f"/api/upload/{art}", body, cookie,
        {"Content-Type": f"multipart/form-data; boundary={boundary}"},
    )
    if status != 200:
        fail(f"Upload {art}: {status} {b}")
    return b["uploadId"]


def main():
    token = sys.argv[1]
    # Einrichtung (W6): Token → TOTP-Geheimnis → Passphrase + Code
    s, _, offer = request("POST", "/api/auth/setup", {"setupToken": token, "step": "totp"})
    if s != 200:
        fail(f"Setup Schritt 1: {s} {offer}")
    s, h, done = request(
        "POST", "/api/auth/setup",
        {"setupToken": token, "passphrase": PASS, "totpCode": totp(offer["secret"])},
    )
    if s != 200:
        fail(f"Setup Schritt 2: {s} {done}")
    cookie = session_from(h)
    print("Einrichtung ok", flush=True)

    today = time.strftime("%Y-%m-%d")
    cmd("seller_profile_upsert", {"input": {
        "name": "Beispiel IT-Service", "legalForm": None, "street": "Beispielweg 1",
        "postalCode": "12345", "city": "Beispielstadt", "countryCode": "DE",
        "taxNumber": "12/345/67890", "vatId": None, "email": "rechnung@example.org",
        "phone": "+49 30 1234567", "iban": None, "bic": None, "logoFilename": None,
        "isKleinunternehmer": True, "defaultPdfTemplate": None, "defaultCurrency": None,
        "confirmWaiveParagraph19": None}}, cookie)
    contact = cmd("contacts_create", {"input": {
        "contactType": "customer", "name": "Beispiel Kunde GmbH", "legalForm": None,
        "vatId": None, "taxNumber": None, "street": "Kundenweg 2", "postalCode": "54321",
        "city": "Musterort", "countryCode": "DE", "email": "kunde@example.org", "phone": None,
        "iban": None, "bic": None, "acceptsEinvoice": False, "notes": None}}, cookie)
    draft = cmd("invoices_create_draft", {"args": {
        "contactId": contact["id"], "buyerReference": None, "input": {
            "direction": "issued", "invoiceDate": today, "deliveryDate": today, "dueDate": None,
            "currencyCode": "EUR", "items": [{
                "position": 1, "description": "Einrichtung Notebook", "quantity": 2,
                "unitCode": "HUR", "unitPriceCents": 4500, "taxRatePercent": 0,
                "taxCategoryCode": "E"}],
            "notes": None, "paymentNote": None, "pdfTemplate": "default",
            "isStornoFor": None, "cancelReason": None}}}, cookie)
    locked = cmd("invoices_lock_and_issue", {"id": draft["invoice"]["id"], "buyerReference": None}, cookie)
    if locked.get("validationStatus") not in ("passed", "warning"):
        fail(f"Festschreiben ohne gültige Prüfung: {locked.get('validationStatus')}")
    number = locked["invoice"]["invoiceNumber"]
    print(f"Rechnung festgeschrieben: {number} ({locked['validationStatus']})", flush=True)

    # Archivdateien holen (hash-geprüft, W7)
    s, _, pdf = request("GET", f"/files/archive/{locked['pdfArchiveId']}", cookie=cookie, raw=True)
    if s != 200 or not pdf.startswith(b"%PDF"):
        fail(f"PDF aus dem Archiv: {s}")
    if b"pdfaid:part>3<" not in pdf and b'pdfaid:part="3"' not in pdf:
        fail("PDF ist kein PDF/A-3")
    s, _, xml = request("GET", f"/files/archive/{locked['xmlArchiveId']}", cookie=cookie, raw=True)
    if s != 200 or b"CrossIndustryInvoice" not in xml:
        fail(f"XML aus dem Archiv: {s}")

    # ZUGFeRD-PDF beim Rechnungseingang: Mustang extrahiert, KoSIT prüft
    parsed = cmd("expenses_parse_einvoice", {"uploadId": upload("erechnung", "eingang.pdf", pdf, cookie)}, cookie)
    if not parsed.get("isPdf") or parsed["parsed"].get("invoiceNumber") != number:
        fail(f"ZUGFeRD nicht extrahiert: {parsed.get('parsed')}")
    if (parsed.get("validation") or {}).get("status") not in ("passed", "warning"):
        fail(f"ZUGFeRD-XML nicht gültig: {parsed.get('validation')}")
    print("ZUGFeRD extrahiert und gültig", flush=True)

    # Ungültige XRechnung beim Rechnungseingang: KoSIT lehnt ab
    broken = xml.replace(b"<ram:TypeCode>380</ram:TypeCode>", b"<ram:TypeCode>999</ram:TypeCode>")
    parsed = cmd("expenses_parse_einvoice", {"uploadId": upload("erechnung", "kaputt.xml", broken, cookie)}, cookie)
    validation = parsed.get("validation") or {}
    if validation.get("status") != "failed":
        fail(f"ungültige XRechnung nicht abgelehnt: {validation}")
    print(f"ungültige XRechnung abgelehnt ({validation.get('errorCount')} Fehler)", flush=True)
    print("SMOKE OK", flush=True)


if __name__ == "__main__":
    main()
