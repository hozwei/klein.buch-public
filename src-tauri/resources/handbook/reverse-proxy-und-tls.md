---
slug: reverse-proxy-und-tls
title: Reverse-Proxy und TLS
category: betrieb
order: 20
keywords: [reverse-proxy, proxy, tls, https, zertifikat, public-url, trusted-proxies, x-forwarded-for, caddy, nginx, domain]
---

# Reverse-Proxy und TLS

Im Container spricht Klein.Buch nur unverschlüsseltes HTTP auf Port
8080. Die Verschlüsselung übernimmt ein Reverse-Proxy davor. Das ist
ein Webserver, der die Anfragen aus dem Internet annimmt, das
Zertifikat für Deine Adresse hält und die Anfragen an Klein.Buch
weiterreicht. TLS (Transport Layer Security) ist das Verfahren hinter
dem "s" in https: Es verschlüsselt die Verbindung zwischen Browser und
Proxy. Welchen Proxy Du nimmst (zum Beispiel Caddy, nginx oder
Traefik), ist Klein.Buch gleich, solange er die Regeln unten einhält.

## Eine eigene Adresse, kein Unterpfad

`KLEIN_BUCH_PUBLIC_URL` ist die vollständige Adresse, unter der Du
Klein.Buch im Browser öffnest, mit `https://` und ohne alles dahinter,
zum Beispiel `https://buch.example.org`. Klein.Buch läuft nur an der
Wurzel einer Adresse. Seine Seiten und Schnittstellen verweisen mit
festen Pfaden ab `/` aufeinander (etwa `/api/…` oder `/help/…`), und die
Anmelde-Cookies gelten für die ganze Adresse. Ein Unterpfad wie
`https://example.org/buchhaltung` funktioniert deshalb nicht. Eine
Adresse mit Pfad, Abfrage-Teil oder Benutzername lehnt Klein.Buch schon
beim Containerstart ab. Nimm eine eigene Subdomain, also einen Namen vor Deiner
Domain wie `buch.` in `buch.example.org`. Ein anderer Port als 443 ist
erlaubt (`https://buch.example.org:8443`), dann muss der Proxy auf
diesem Port lauschen.

Jede Anfrage muss genau diese Adresse nennen. Passt der Name nicht,
antwortet Klein.Buch mit "Unbekannter Host." (HTTP-Status 421). Bei
ändernden Anfragen muss auch die Herkunft (der `Origin`-Header des
Browsers) passen, sonst kommt "Anfrage von fremder Herkunft abgelehnt."
(403). Über die IP-Adresse des Servers oder einen zweiten Namen
erreichst Du Klein.Buch also nicht.

## Vertraute Proxys

Hinter einem Proxy sieht Klein.Buch als Gegenstelle immer nur den
Proxy. Wer wirklich anfragt und ob der Browser HTTPS benutzt hat,
erfährt es aus drei Kopfzeilen, die der Proxy mitschickt:
`X-Forwarded-For` (Adresse des Browsers), `X-Forwarded-Proto` (`https`
oder `http`) und `X-Forwarded-Host` (der aufgerufene Name). Weil jeder
solche Kopfzeilen fälschen kann, glaubt Klein.Buch ihnen nur, wenn die
Verbindung von einer Adresse aus `KLEIN_BUCH_TRUSTED_PROXIES` kommt.

Du trägst dort eine oder mehrere Adressen oder Adressbereiche ein,
durch Komma oder Leerzeichen getrennt. Ein Bereich wird als CIDR
geschrieben: Adresse, Schrägstrich, Länge des festen Teils.
`198.51.100.10/32` ist genau eine Adresse, `198.51.100.0/24` sind die
256 Adressen von `198.51.100.0` bis `198.51.100.255`. Eine Adresse ohne
Schrägstrich gilt als einzelne Adresse. Als Adresse des Browsers nimmt
Klein.Buch in `X-Forwarded-For` von rechts den ersten Eintrag, der nicht
zu Deinen vertrauten Proxys gehört.

Welche Adresse Du einträgst, hängt davon ab, woher die Verbindungen des
Proxys im Container ankommen. Läuft der Proxy als Container im selben
Docker-Netz, ist es die Adresse dieses Containers (gib ihm eine feste
Adresse oder trag den Bereich des Netzes ein, wenn dort sonst nichts
läuft). Läuft der Proxy direkt auf dem Server und spricht den
veröffentlichten Port `127.0.0.1:8080` an, sieht Klein.Buch meist die
Gateway-Adresse des Docker-Netzes; sie steht in der Ausgabe von
`docker network inspect` unter "Gateway". Läuft der Proxy auf einem
anderen Rechner, ist es dessen Adresse im lokalen Netz; dann gibst Du
Port 8080 nur auf der internen Netzwerkkarte frei, nie nach außen.

Fehlt die richtige Adresse, lehnt Klein.Buch Einrichtung und Anmeldung
ab mit "Anmeldung nur über HTTPS (Reverse-Proxy) oder lokal möglich.",
weil es nicht weiß, ob der Browser HTTPS benutzt hat. "Lokal" heißt
dabei: aus dem Container selbst heraus. Im Alltag führt der Weg also
immer über den Proxy mit HTTPS.

Ist der Proxy eingetragen, schickt aber keine `X-Forwarded-For`-Zeile,
sehen alle Anfragen so aus, als kämen sie vom Proxy. Dann wird die
Wartezeit nach Fehlversuchen, die eigentlich je Adresse gilt, zu einer
gemeinsamen für alle: Nach fünf Fehlversuchen irgendeines Besuchers
muss jeder warten. Nur Dein eigener Browser bleibt frei, weil er nach
der ersten Anmeldung ein Gerätecookie hat. Auch fail2ban (siehe Kapitel
"Fehlersuche im Container") sähe dann nur die Adresse des Proxys.

Trag nie `0.0.0.0/0` oder `::/0` ein. Dann vertraut Klein.Buch jedem,
und die Regeln für HTTPS und Fehlversuche lassen sich umgehen.
Klein.Buch warnt in diesem Fall beim Containerstart im Log.

## Was der Proxy tun muss

1. HTTPS mit einem gültigen Zertifikat für Deine Adresse anbieten und
   Aufrufe über `http://` auf `https://` umleiten.
2. Den `Host`-Header unverändert weitergeben oder `X-Forwarded-Host` auf
   den öffentlichen Namen setzen.
3. `X-Forwarded-For` mit der Adresse des Browsers setzen oder ergänzen.
4. `X-Forwarded-Proto` auf `https` setzen. Der Proxy muss den Wert
   selbst setzen und darf keinen mitgeschickten Wert des Browsers
   durchreichen.
5. Große Uploads zulassen. Belege, Anhänge und Verträge dürfen bis zu
   20 MiB groß sein, E-Rechnungen bis zu 10 MiB, Logo und Unterschrift
   bis zu 2 MiB. Für eine Wiederherstellung lädst Du eine ganze
   Backup-Datei hoch; das Limit des Proxys muss mindestens so groß sein
   wie Dein größtes Backup. Klein.Buch selbst nimmt bis
   `KLEIN_BUCH_MAX_RESTORE_BYTES` an (Standard 4 GiB).
6. Keine eigenen Sicherheits-Kopfzeilen wie `Content-Security-Policy`
   oder `X-Frame-Options` hinzufügen. Klein.Buch setzt sie selbst,
   auch HSTS (die Anweisung an den Browser, die Adresse ein Jahr lang
   nur über HTTPS aufzurufen), sobald eine Anfrage über einen vertrauten
   Proxy mit HTTPS kommt.

Bei den Zeitlimits musst Du meist nichts ändern. Klein.Buch beantwortet
jede Anfrage innerhalb von 30 Sekunden; längere Arbeiten laufen im
Hintergrund weiter, und der Browser fragt das Ergebnis ab. Uploads
fließen gleichmäßig; Klein.Buch bricht erst ab, wenn 60 Sekunden lang
keine Daten kommen.

## Beispiel mit Caddy

Caddy holt das Zertifikat selbst, reicht den `Host`-Header weiter und
setzt die drei `X-Forwarded`-Zeilen von sich aus. Eine Grenze für die
Größe von Uploads hat Caddy ohne eigene Einstellung nicht.

```
buch.example.org {
	reverse_proxy 127.0.0.1:8080
}
```

## Beispiel mit nginx

Bei nginx musst Du die Kopfzeilen und die Upload-Grenze selbst setzen.
Ohne `client_max_body_size` erlaubt nginx nur 1 MB, und schon ein
gewöhnlicher Beleg scheitert. nginx legt große Uploads vor dem
Weiterreichen in einer temporären Datei ab; dort muss genug Platz sein.

```
server {
    listen 80;
    server_name buch.example.org;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl;
    server_name buch.example.org;

    ssl_certificate     /etc/ssl/buch.example.org/fullchain.pem;
    ssl_certificate_key /etc/ssl/buch.example.org/privkey.pem;

    # mindestens so groß wie das größte Backup (Wiederherstellung per Upload)
    client_max_body_size 4100m;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host              $host;
        proxy_set_header X-Forwarded-For   $remote_addr;
        proxy_set_header X-Forwarded-Proto https;
        proxy_set_header X-Forwarded-Host  $host;
    }
}
```

## Prüfen, ob alles stimmt

Öffne `https://buch.example.org`. Erscheint die Einrichtung oder die
Anmeldung, stimmt der Weg. "Unbekannter Host." bedeutet, dass Name in
der Anfrage und `KLEIN_BUCH_PUBLIC_URL` nicht übereinstimmen. "Anmeldung
nur über HTTPS (Reverse-Proxy) oder lokal möglich." bedeutet, dass
`KLEIN_BUCH_TRUSTED_PROXIES` nicht zur Adresse des Proxys passt oder
der Proxy `X-Forwarded-Proto` nicht setzt. Eine Überwachung von außen
kann `https://buch.example.org/healthz` abfragen: Die Adresse antwortet
ohne Anmeldung mit Status 200, solange Klein.Buch läuft, und verrät
sonst nichts.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
