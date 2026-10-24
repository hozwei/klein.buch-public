# Java-Sidecar: Inventar der Drittkomponenten

Von Hand gepflegt (Plan W12, ADR 0039). Stand: 2026-10-24, für Mustang 2.23.0, KoSIT-Validator 1.6.2
und die Validator-Konfiguration XRechnung 2026-01-31 (`versions.json`, Schema 2).

Dieses Inventar listet jede Komponente, die in `lib/mustang.jar` und `lib/kosit-validator.jar`
eingebaut ist, mit Herkunft, Version, SPDX-Ausdruck, Quell-URL und der zugehörigen Hinweisdatei in
`notices/`. Jede Zeile beginnt mit der Maven-Koordinate `group:artifact:version` in Backticks.
`legal/check_java_inventory.py` liest alle `META-INF/maven/*/*/pom.properties` der beiden JARs und
bricht mit Exit 1 ab, wenn eine Koordinate hier fehlt. Bei jedem Versionswechsel in `versions.json`
das Skript laufen lassen, die Tabellen nachziehen und die Hinweise in `notices/` aus den neuen
Originalartefakten erneuern.

Abkürzungen: „M“ = in `mustang.jar`, „K“ = in `kosit-validator.jar`. „Bytegleich“ heißt: jede
`.class`-Datei des Originalartefakts von Maven Central liegt mit identischem Inhalt im Uber-JAR
(geprüft 2026-10-24 mit Python/`zipfile`; die einzige fehlende Datei ist jeweils `module-info.class`).
Alle Klassen und Ressourcen beider JARs sind damit einem der unten genannten Artefakte zugeordnet;
übrig bleiben nur die eigenen Klassen von Mustang-CLI (`org/mustangproject/commandline/*`, `logback.xml`).

## Ausgelieferte Dateien

| Datei im Image (`/opt/klein-buch/sidecar/…`) | Herkunft | SHA-256 | Lizenz |
|---|---|---|---|
| `lib/mustang.jar` | Mustang-CLI 2.23.0, [Maven Central](https://repo1.maven.org/maven2/org/mustangproject/Mustang-CLI/2.23.0/Mustang-CLI-2.23.0.jar) (Uber-JAR, unverändert) | `344c88b8d9bddccae23899a87d1ef31c4d38532383faa6303c381ee489cabe07` | Apache-2.0 (Bestandteile siehe Tabelle A) |
| `lib/kosit-validator.jar` | KoSIT-Validator 1.6.2, [GitHub-Release](https://github.com/itplr-kosit/validator/releases/download/v1.6.2/validator-1.6.2-standalone.jar) (Standalone-JAR, unverändert) | `244978514ad48f67c7573acfffc8f4fd73d81feda6f276710033f9913579857e` | Apache-2.0 (Bestandteile siehe Tabelle B) |
| `xrechnung-config/` | Validator-Konfiguration XRechnung 2026-01-31 (XRechnung 3.0.2), [Release-ZIP](https://github.com/itplr-kosit/validator-configuration-xrechnung/releases/download/v2026-01-31/xrechnung-3.0.2-validator-configuration-2026-01-31.zip) | ZIP: `6a5a5911a421b25fbc423f62f93f894df7b236f5d73ca4f84bb222a945082704` | Apache-2.0 und Bestandteile siehe Tabelle D |
| jlink-Laufzeit `jre/` (im Image-Build erzeugt) | Eclipse Temurin 21.0.12+8 (`eclipse-temurin:21.0.12_8-jdk`, Digest in `versions.json`), Quelle [adoptium/jdk21u, Tag jdk-21.0.12+8](https://github.com/adoptium/jdk21u/tree/jdk-21.0.12+8) | – | GPL-2.0-only WITH Classpath-exception-2.0 (Lizenztexte der Module unter `/opt/klein-buch/sidecar/jre/legal/`) |

Die Laufzeit ist kein Bestandteil der beiden JARs und deshalb hier nur der Vollständigkeit halber
genannt; ihren Quelltext und den jlink-Befehl enthält das Quellarchiv Dritter des Releases.

## A. `mustang.jar` – Komponenten mit `pom.properties` (83)

| Maven-Koordinate | Komponente | SPDX | Quelle (-sources.jar) | Hinweise in `notices/` |
|---|---|---|---|---|
| `cglib:cglib-nodep:3.2.5` | cglib (nur pom.properties, keine Klassen unter net/sf/cglib; cglib steckt umbenannt in assertj-core) | Apache-2.0 | [cglib-nodep-3.2.5-sources.jar](https://repo1.maven.org/maven2/cglib/cglib-nodep/3.2.5/cglib-nodep-3.2.5-sources.jar) | assertj-core-NOTICE.txt |
| `cglib:cglib:3.2.5` | cglib (nur pom.properties, keine Klassen unter net/sf/cglib; cglib steckt umbenannt in assertj-core) | Apache-2.0 | [cglib-3.2.5-sources.jar](https://repo1.maven.org/maven2/cglib/cglib/3.2.5/cglib-3.2.5-sources.jar) | assertj-core-NOTICE.txt |
| `ch.qos.logback:logback-core:1.5.18` | Logback Core | EPL-1.0 OR LGPL-2.1-only (genutzt: EPL-1.0) | [logback-core-1.5.18-sources.jar](https://repo1.maven.org/maven2/ch/qos/logback/logback-core/1.5.18/logback-core-1.5.18-sources.jar) | logback-core-LICENSE.txt |
| `com.fasterxml.jackson.core:jackson-annotations:2.17.3` | Jackson | Apache-2.0 | [jackson-annotations-2.17.3-sources.jar](https://repo1.maven.org/maven2/com/fasterxml/jackson/core/jackson-annotations/2.17.3/jackson-annotations-2.17.3-sources.jar) | jackson-databind-NOTICE.txt |
| `com.fasterxml.jackson.core:jackson-core:2.17.3` | Jackson Core (enthält FastDoubleParser, fast_float, bigint) | Apache-2.0 AND MIT AND BSD-2-Clause | [jackson-core-2.17.3-sources.jar](https://repo1.maven.org/maven2/com/fasterxml/jackson/core/jackson-core/2.17.3/jackson-core-2.17.3-sources.jar) | jackson-core-NOTICE.txt, jackson-core-FastDoubleParser-NOTICE.txt |
| `com.fasterxml.jackson.core:jackson-databind:2.17.3` | Jackson | Apache-2.0 | [jackson-databind-2.17.3-sources.jar](https://repo1.maven.org/maven2/com/fasterxml/jackson/core/jackson-databind/2.17.3/jackson-databind-2.17.3-sources.jar) | jackson-databind-NOTICE.txt |
| `com.google.code.findbugs:jsr305:3.0.2` | JSR-305-Annotationen (FindBugs) | Apache-2.0 | [jsr305-3.0.2-sources.jar](https://repo1.maven.org/maven2/com/google/code/findbugs/jsr305/3.0.2/jsr305-3.0.2-sources.jar) | apache-2.0-LICENSE.txt |
| `com.helger.cii:ph-cii-d16b:3.1.0` | ph-cii (mit UN/CEFACT-Schemata D16B) | Apache-2.0 | [ph-cii-d16b-3.1.0-sources.jar](https://repo1.maven.org/maven2/com/helger/cii/ph-cii-d16b/3.1.0/ph-cii-d16b-3.1.0-sources.jar) | ph-cii-d16b-NOTICE.txt, uncefact-d16b-NOTICE.txt |
| `com.helger.commons:ph-commons:11.2.6` | ph-commons | Apache-2.0 | [ph-commons-11.2.6-sources.jar](https://repo1.maven.org/maven2/com/helger/commons/ph-commons/11.2.6/ph-commons-11.2.6-sources.jar) | ph-commons-NOTICE.txt |
| `com.helger.commons:ph-datetime:11.2.6` | ph-commons-Modul | Apache-2.0 | [ph-datetime-11.2.6-sources.jar](https://repo1.maven.org/maven2/com/helger/commons/ph-datetime/11.2.6/ph-datetime-11.2.6-sources.jar) | ph-xml-NOTICE.txt |
| `com.helger.commons:ph-jaxb-adapter:11.1.11` | ph-jaxb-adapter | Apache-2.0 | [ph-jaxb-adapter-11.1.11-sources.jar](https://repo1.maven.org/maven2/com/helger/commons/ph-jaxb-adapter/11.1.11/ph-jaxb-adapter-11.1.11-sources.jar) | ph-jaxb-adapter-NOTICE.txt |
| `com.helger.commons:ph-jaxb:11.2.6` | ph-jaxb | Apache-2.0 | [ph-jaxb-11.2.6-sources.jar](https://repo1.maven.org/maven2/com/helger/commons/ph-jaxb/11.2.6/ph-jaxb-11.2.6-sources.jar) | ph-jaxb-NOTICE.txt |
| `com.helger.commons:ph-xml:11.2.6` | ph-commons-Modul | Apache-2.0 | [ph-xml-11.2.6-sources.jar](https://repo1.maven.org/maven2/com/helger/commons/ph-xml/11.2.6/ph-xml-11.2.6-sources.jar) | ph-xml-NOTICE.txt |
| `com.helger.schematron:ph-schematron-api:8.0.6` | ph-schematron | Apache-2.0 | [ph-schematron-api-8.0.6-sources.jar](https://repo1.maven.org/maven2/com/helger/schematron/ph-schematron-api/8.0.6/ph-schematron-api-8.0.6-sources.jar) | ph-xml-NOTICE.txt |
| `com.helger.schematron:ph-schematron-xslt:8.0.6` | ph-schematron | Apache-2.0 | [ph-schematron-xslt-8.0.6-sources.jar](https://repo1.maven.org/maven2/com/helger/schematron/ph-schematron-xslt/8.0.6/ph-schematron-xslt-8.0.6-sources.jar) | ph-xml-NOTICE.txt |
| `com.helger.ubl:ph-ubl21:9.0.3` | ph-ubl (UBL 2.1, mit OASIS-Schemata) | Apache-2.0 | [ph-ubl21-9.0.3-sources.jar](https://repo1.maven.org/maven2/com/helger/ubl/ph-ubl21/9.0.3/ph-ubl21-9.0.3-sources.jar) | ph-ubl21-NOTICE.txt, oasis-ubl-2.1-NOTICE.txt |
| `com.helger.ubl:ph-ubl22:9.0.3` | ph-ubl (mit OASIS-Schemata) | Apache-2.0 | [ph-ubl22-9.0.3-sources.jar](https://repo1.maven.org/maven2/com/helger/ubl/ph-ubl22/9.0.3/ph-ubl22-9.0.3-sources.jar) | ph-cii-d16b-NOTICE.txt, oasis-ubl-2.1-NOTICE.txt |
| `com.helger.ubl:ph-ubl23:9.0.3` | ph-ubl (mit OASIS-Schemata) | Apache-2.0 | [ph-ubl23-9.0.3-sources.jar](https://repo1.maven.org/maven2/com/helger/ubl/ph-ubl23/9.0.3/ph-ubl23-9.0.3-sources.jar) | ph-cii-d16b-NOTICE.txt, oasis-ubl-2.1-NOTICE.txt |
| `com.helger.ubl:ph-ubl24:9.0.3` | ph-ubl (mit OASIS-Schemata) | Apache-2.0 | [ph-ubl24-9.0.3-sources.jar](https://repo1.maven.org/maven2/com/helger/ubl/ph-ubl24/9.0.3/ph-ubl24-9.0.3-sources.jar) | ph-cii-d16b-NOTICE.txt, oasis-ubl-2.1-NOTICE.txt |
| `com.helger.xsd:ph-xsds-ccts-cct-schemamodule:3.0.0` | ph-xsds (W3C-/ETSI-Schemata) | Apache-2.0 | [ph-xsds-ccts-cct-schemamodule-3.0.0-sources.jar](https://repo1.maven.org/maven2/com/helger/xsd/ph-xsds-ccts-cct-schemamodule/3.0.0/ph-xsds-ccts-cct-schemamodule-3.0.0-sources.jar) | ph-cii-d16b-NOTICE.txt |
| `com.helger.xsd:ph-xsds-xades132:3.0.0` | ph-xsds (W3C-/ETSI-Schemata) | Apache-2.0 | [ph-xsds-xades132-3.0.0-sources.jar](https://repo1.maven.org/maven2/com/helger/xsd/ph-xsds-xades132/3.0.0/ph-xsds-xades132-3.0.0-sources.jar) | ph-cii-d16b-NOTICE.txt |
| `com.helger.xsd:ph-xsds-xades141:3.0.0` | ph-xsds (W3C-/ETSI-Schemata) | Apache-2.0 | [ph-xsds-xades141-3.0.0-sources.jar](https://repo1.maven.org/maven2/com/helger/xsd/ph-xsds-xades141/3.0.0/ph-xsds-xades141-3.0.0-sources.jar) | ph-cii-d16b-NOTICE.txt |
| `com.helger.xsd:ph-xsds-xml:3.0.0` | ph-xsds (W3C-/ETSI-Schemata) | Apache-2.0 | [ph-xsds-xml-3.0.0-sources.jar](https://repo1.maven.org/maven2/com/helger/xsd/ph-xsds-xml/3.0.0/ph-xsds-xml-3.0.0-sources.jar) | ph-cii-d16b-NOTICE.txt |
| `com.helger.xsd:ph-xsds-xmldsig:3.0.0` | ph-xsds (W3C-/ETSI-Schemata) | Apache-2.0 | [ph-xsds-xmldsig-3.0.0-sources.jar](https://repo1.maven.org/maven2/com/helger/xsd/ph-xsds-xmldsig/3.0.0/ph-xsds-xmldsig-3.0.0-sources.jar) | ph-cii-d16b-NOTICE.txt |
| `com.helger.xsd:ph-xsds-xmldsig11:3.0.0` | ph-xsds (W3C-/ETSI-Schemata) | Apache-2.0 | [ph-xsds-xmldsig11-3.0.0-sources.jar](https://repo1.maven.org/maven2/com/helger/xsd/ph-xsds-xmldsig11/3.0.0/ph-xsds-xmldsig11-3.0.0-sources.jar) | ph-cii-d16b-NOTICE.txt |
| `com.helger:en16931-cii2ubl:2.3.1` | EN16931 CII-zu-UBL-Konverter | Apache-2.0 | [en16931-cii2ubl-2.3.1-sources.jar](https://repo1.maven.org/maven2/com/helger/en16931-cii2ubl/2.3.1/en16931-cii2ubl-2.3.1-sources.jar) | en16931-cii2ubl-NOTICE.txt |
| `com.sun.istack:istack-commons-runtime:4.1.1` | istack commons runtime (Eclipse EE4J) | BSD-3-Clause | [istack-commons-runtime-4.1.1-sources.jar](https://repo1.maven.org/maven2/com/sun/istack/istack-commons-runtime/4.1.1/istack-commons-runtime-4.1.1-sources.jar) | jaxb-runtime-NOTICE.txt |
| `com.thoughtworks.qdox:qdox:1.12` | QDox | Apache-2.0 | [qdox-1.12-sources.jar](https://repo1.maven.org/maven2/com/thoughtworks/qdox/qdox/1.12/qdox-1.12-sources.jar) | apache-2.0-LICENSE.txt |
| `commons-cli:commons-cli:1.10.0` | Apache Commons CLI | Apache-2.0 | [commons-cli-1.10.0-sources.jar](https://repo1.maven.org/maven2/commons-cli/commons-cli/1.10.0/commons-cli-1.10.0-sources.jar) | commons-cli-NOTICE.txt |
| `commons-io:commons-io:2.20.0` | Apache Commons IO | Apache-2.0 | [commons-io-2.20.0-sources.jar](https://repo1.maven.org/maven2/commons-io/commons-io/2.20.0/commons-io-2.20.0-sources.jar) | commons-io-NOTICE.txt |
| `commons-logging:commons-logging:1.3.0` | Apache Commons Logging | Apache-2.0 | [commons-logging-1.3.0-sources.jar](https://repo1.maven.org/maven2/commons-logging/commons-logging/1.3.0/commons-logging-1.3.0-sources.jar) | commons-logging-NOTICE.txt |
| `jakarta.activation:jakarta.activation-api:2.1.3` | Jakarta Activation API | BSD-3-Clause | [jakarta.activation-api-2.1.3-sources.jar](https://repo1.maven.org/maven2/jakarta/activation/jakarta.activation-api/2.1.3/jakarta.activation-api-2.1.3-sources.jar) | jakarta.activation-api-NOTICE.txt |
| `jakarta.xml.bind:jakarta.xml.bind-api:4.0.2` | Jakarta XML Binding API | BSD-3-Clause | [jakarta.xml.bind-api-4.0.2-sources.jar](https://repo1.maven.org/maven2/jakarta/xml/bind/jakarta.xml.bind-api/4.0.2/jakarta.xml.bind-api-4.0.2-sources.jar) | jakarta.xml.bind-api-NOTICE.txt |
| `net.bytebuddy:byte-buddy:1.12.23` | Byte Buddy (enthält ASM) | Apache-2.0 AND BSD-3-Clause | [byte-buddy-1.12.23-sources.jar](https://repo1.maven.org/maven2/net/bytebuddy/byte-buddy/1.12.23/byte-buddy-1.12.23-sources.jar) | byte-buddy-NOTICE.txt |
| `org.apache.pdfbox:fontbox:3.0.7` | Apache FontBox | Apache-2.0 (Fremdbestandteile siehe fontbox-LICENSE.txt) | [fontbox-3.0.7-sources.jar](https://repo1.maven.org/maven2/org/apache/pdfbox/fontbox/3.0.7/fontbox-3.0.7-sources.jar) | fontbox-NOTICE.txt, fontbox-LICENSE.txt |
| `org.apache.pdfbox:pdfbox-io:3.0.7` | Apache PDFBox IO | Apache-2.0 | [pdfbox-io-3.0.7-sources.jar](https://repo1.maven.org/maven2/org/apache/pdfbox/pdfbox-io/3.0.7/pdfbox-io-3.0.7-sources.jar) | pdfbox-io-NOTICE.txt |
| `org.apache.pdfbox:pdfbox:3.0.7` | Apache PDFBox (enthält Adobe-AFM/CMaps/Glyphlist, Schriften, ICC-Profile) | Apache-2.0 (Fremdbestandteile u. a. APAFML, Adobe-Glyph, BSD-3-Clause, OFL-1.1, siehe pdfbox-LICENSE.txt) | [pdfbox-3.0.7-sources.jar](https://repo1.maven.org/maven2/org/apache/pdfbox/pdfbox/3.0.7/pdfbox-3.0.7-sources.jar) | pdfbox-NOTICE.txt, pdfbox-LICENSE.txt |
| `org.apache.pdfbox:preflight:3.0.7` | Apache PDFBox Preflight | Apache-2.0 | [preflight-3.0.7-sources.jar](https://repo1.maven.org/maven2/org/apache/pdfbox/preflight/3.0.7/preflight-3.0.7-sources.jar) | preflight-NOTICE.txt |
| `org.apache.pdfbox:xmpbox:3.0.7` | Apache XmpBox | Apache-2.0 | [xmpbox-3.0.7-sources.jar](https://repo1.maven.org/maven2/org/apache/pdfbox/xmpbox/3.0.7/xmpbox-3.0.7-sources.jar) | xmpbox-NOTICE.txt |
| `org.apache.xmlgraphics:batik-anim:1.19` | Apache Batik | Apache-2.0 | [batik-anim-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-anim/1.19/batik-anim-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-awt-util:1.19` | Apache Batik | Apache-2.0 | [batik-awt-util-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-awt-util/1.19/batik-awt-util-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-bridge:1.19` | Apache Batik | Apache-2.0 | [batik-bridge-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-bridge/1.19/batik-bridge-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-codec:1.19` | Apache Batik | Apache-2.0 | [batik-codec-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-codec/1.19/batik-codec-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-constants:1.19` | Apache Batik | Apache-2.0 | [batik-constants-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-constants/1.19/batik-constants-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-css:1.19` | Apache Batik | Apache-2.0 | [batik-css-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-css/1.19/batik-css-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-dom:1.19` | Apache Batik | Apache-2.0 | [batik-dom-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-dom/1.19/batik-dom-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-ext:1.19` | Apache Batik | Apache-2.0 | [batik-ext-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-ext/1.19/batik-ext-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-extension:1.19` | Apache Batik | Apache-2.0 | [batik-extension-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-extension/1.19/batik-extension-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-gvt:1.19` | Apache Batik | Apache-2.0 | [batik-gvt-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-gvt/1.19/batik-gvt-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-i18n:1.19` | Apache Batik | Apache-2.0 | [batik-i18n-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-i18n/1.19/batik-i18n-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-parser:1.19` | Apache Batik | Apache-2.0 | [batik-parser-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-parser/1.19/batik-parser-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-script:1.19` | Apache Batik | Apache-2.0 | [batik-script-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-script/1.19/batik-script-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-shared-resources:1.19` | Apache Batik | Apache-2.0 | [batik-shared-resources-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-shared-resources/1.19/batik-shared-resources-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-svg-dom:1.19` | Apache Batik | Apache-2.0 | [batik-svg-dom-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-svg-dom/1.19/batik-svg-dom-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-svggen:1.19` | Apache Batik | Apache-2.0 | [batik-svggen-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-svggen/1.19/batik-svggen-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-transcoder:1.19` | Apache Batik | Apache-2.0 | [batik-transcoder-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-transcoder/1.19/batik-transcoder-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-util:1.19` | Apache Batik | Apache-2.0 | [batik-util-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-util/1.19/batik-util-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:batik-xml:1.19` | Apache Batik | Apache-2.0 | [batik-xml-1.19-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/batik-xml/1.19/batik-xml-1.19-sources.jar) | batik-NOTICE.txt |
| `org.apache.xmlgraphics:fop-core:2.11` | Apache FOP Core (enthält sRGB.icc, Zlib) | Apache-2.0 AND Zlib | [fop-core-2.11-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/fop-core/2.11/fop-core-2.11-sources.jar) | fop-NOTICE.txt, fop-core-sRGB-icc-LICENSE.txt |
| `org.apache.xmlgraphics:fop-events:2.11` | Apache FOP | Apache-2.0 | [fop-events-2.11-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/fop-events/2.11/fop-events-2.11-sources.jar) | fop-NOTICE.txt |
| `org.apache.xmlgraphics:fop-util:2.11` | Apache FOP | Apache-2.0 | [fop-util-2.11-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/fop-util/2.11/fop-util-2.11-sources.jar) | fop-NOTICE.txt |
| `org.apache.xmlgraphics:xmlgraphics-commons:2.11` | Apache XML Graphics Commons | Apache-2.0 | [xmlgraphics-commons-2.11-sources.jar](https://repo1.maven.org/maven2/org/apache/xmlgraphics/xmlgraphics-commons/2.11/xmlgraphics-commons-2.11-sources.jar) | xmlgraphics-commons-NOTICE.txt |
| `org.assertj:assertj-core:2.9.0` | AssertJ Core (enthält cglib 3.2.5 und ASM umbenannt) | Apache-2.0 AND BSD-3-Clause | [assertj-core-2.9.0-sources.jar](https://repo1.maven.org/maven2/org/assertj/assertj-core/2.9.0/assertj-core-2.9.0-sources.jar) | assertj-core-NOTICE.txt, byte-buddy-NOTICE.txt (ASM-Text) |
| `org.codehaus.janino:commons-compiler:3.1.12` | Janino | BSD-3-Clause | [commons-compiler-3.1.12-sources.jar](https://repo1.maven.org/maven2/org/codehaus/janino/commons-compiler/3.1.12/commons-compiler-3.1.12-sources.jar) | janino-LICENSE.txt |
| `org.codehaus.janino:janino:3.1.12` | Janino | BSD-3-Clause | [janino-3.1.12-sources.jar](https://repo1.maven.org/maven2/org/codehaus/janino/janino/3.1.12/janino-3.1.12-sources.jar) | janino-LICENSE.txt |
| `org.eclipse.angus:angus-activation:2.0.0` | Eclipse Angus Activation | BSD-3-Clause | [angus-activation-2.0.0-sources.jar](https://repo1.maven.org/maven2/org/eclipse/angus/angus-activation/2.0.0/angus-activation-2.0.0-sources.jar) | angus-activation-NOTICE.txt |
| `org.glassfish.jaxb:jaxb-core:4.0.2` | Eclipse Implementation of JAXB | BSD-3-Clause | [jaxb-core-4.0.2-sources.jar](https://repo1.maven.org/maven2/org/glassfish/jaxb/jaxb-core/4.0.2/jaxb-core-4.0.2-sources.jar) | jaxb-runtime-NOTICE.txt |
| `org.glassfish.jaxb:jaxb-runtime:4.0.2` | Eclipse Implementation of JAXB | BSD-3-Clause | [jaxb-runtime-4.0.2-sources.jar](https://repo1.maven.org/maven2/org/glassfish/jaxb/jaxb-runtime/4.0.2/jaxb-runtime-4.0.2-sources.jar) | jaxb-runtime-NOTICE.txt |
| `org.glassfish.jaxb:txw2:4.0.2` | Eclipse Implementation of JAXB | BSD-3-Clause | [txw2-4.0.2-sources.jar](https://repo1.maven.org/maven2/org/glassfish/jaxb/txw2/4.0.2/txw2-4.0.2-sources.jar) | jaxb-runtime-NOTICE.txt |
| `org.mustangproject:Mustang-CLI:2.23.0` | Mustang Project | Apache-2.0 | [Mustang-CLI-2.23.0-sources.jar](https://repo1.maven.org/maven2/org/mustangproject/Mustang-CLI/2.23.0/Mustang-CLI-2.23.0-sources.jar) | mustang-NOTICE.txt |
| `org.mustangproject:library:2.23.0` | Mustang Project | Apache-2.0 | [library-2.23.0-sources.jar](https://repo1.maven.org/maven2/org/mustangproject/library/2.23.0/library-2.23.0-sources.jar) | mustang-NOTICE.txt |
| `org.mustangproject:validator:2.23.0` | Mustang Project | Apache-2.0 | [validator-2.23.0-sources.jar](https://repo1.maven.org/maven2/org/mustangproject/validator/2.23.0/validator-2.23.0-sources.jar) | mustang-NOTICE.txt |
| `org.slf4j:slf4j-api:2.0.17` | SLF4J | MIT | [slf4j-api-2.0.17-sources.jar](https://repo1.maven.org/maven2/org/slf4j/slf4j-api/2.0.17/slf4j-api-2.0.17-sources.jar) | slf4j-LICENSE.txt |
| `org.slf4j:slf4j-simple:2.0.17` | SLF4J | MIT | [slf4j-simple-2.0.17-sources.jar](https://repo1.maven.org/maven2/org/slf4j/slf4j-simple/2.0.17/slf4j-simple-2.0.17-sources.jar) | slf4j-LICENSE.txt |
| `org.verapdf:core-jakarta:1.26.5` | veraPDF | GPL-3.0-or-later OR MPL-2.0 (genutzt: MPL-2.0) | [core-jakarta-1.26.5-sources.jar](https://repo1.maven.org/maven2/org/verapdf/core-jakarta/1.26.5/core-jakarta-1.26.5-sources.jar) | verapdf-LICENSE.txt, verapdf-NOTICE.txt |
| `org.verapdf:feature-reporting-jakarta:1.26.5` | veraPDF | GPL-3.0-or-later OR MPL-2.0 (genutzt: MPL-2.0) | [feature-reporting-jakarta-1.26.5-sources.jar](https://repo1.maven.org/maven2/org/verapdf/feature-reporting-jakarta/1.26.5/feature-reporting-jakarta-1.26.5-sources.jar) | verapdf-LICENSE.txt, verapdf-NOTICE.txt |
| `org.verapdf:metadata-fixer-jakarta:1.26.5` | veraPDF | GPL-3.0-or-later OR MPL-2.0 (genutzt: MPL-2.0) | [metadata-fixer-jakarta-1.26.5-sources.jar](https://repo1.maven.org/maven2/org/verapdf/metadata-fixer-jakarta/1.26.5/metadata-fixer-jakarta-1.26.5-sources.jar) | verapdf-LICENSE.txt, verapdf-NOTICE.txt |
| `org.verapdf:parser:1.26.1` | veraPDF | GPL-3.0-or-later OR MPL-2.0 (genutzt: MPL-2.0) | [parser-1.26.1-sources.jar](https://repo1.maven.org/maven2/org/verapdf/parser/1.26.1/parser-1.26.1-sources.jar) | verapdf-LICENSE.txt, verapdf-NOTICE.txt |
| `org.verapdf:pdf-model:1.26.1` | veraPDF | GPL-3.0-or-later OR MPL-2.0 (genutzt: MPL-2.0) | [pdf-model-1.26.1-sources.jar](https://repo1.maven.org/maven2/org/verapdf/pdf-model/1.26.1/pdf-model-1.26.1-sources.jar) | verapdf-LICENSE.txt, verapdf-NOTICE.txt |
| `org.verapdf:validation-model-jakarta:1.26.5` | veraPDF | GPL-3.0-or-later OR MPL-2.0 (genutzt: MPL-2.0) | [validation-model-jakarta-1.26.5-sources.jar](https://repo1.maven.org/maven2/org/verapdf/validation-model-jakarta/1.26.5/validation-model-jakarta-1.26.5-sources.jar) | verapdf-LICENSE.txt, verapdf-NOTICE.txt |
| `org.verapdf:verapdf-xmp-core-jakarta:1.26.5` | veraPDF XMP-Kern (aus Adobe XMP Toolkit) | (GPL-3.0-or-later OR MPL-2.0) AND BSD-3-Clause (genutzt: MPL-2.0) | [verapdf-xmp-core-jakarta-1.26.5-sources.jar](https://repo1.maven.org/maven2/org/verapdf/verapdf-xmp-core-jakarta/1.26.5/verapdf-xmp-core-jakarta-1.26.5-sources.jar) | verapdf-LICENSE.txt, verapdf-NOTICE.txt |
| `org.xmlunit:xmlunit-assertj:2.10.3` | XMLUnit | Apache-2.0 | [xmlunit-assertj-2.10.3-sources.jar](https://repo1.maven.org/maven2/org/xmlunit/xmlunit-assertj/2.10.3/xmlunit-assertj-2.10.3-sources.jar) | apache-2.0-LICENSE.txt |
| `org.xmlunit:xmlunit-core:2.10.3` | XMLUnit | Apache-2.0 | [xmlunit-core-2.10.3-sources.jar](https://repo1.maven.org/maven2/org/xmlunit/xmlunit-core/2.10.3/xmlunit-core-2.10.3-sources.jar) | apache-2.0-LICENSE.txt |

## B. `kosit-validator.jar` – Komponenten mit `pom.properties` (13)

| Maven-Koordinate | Komponente | SPDX | Quelle (-sources.jar) | Hinweise in `notices/` |
|---|---|---|---|---|
| `com.sun.istack:istack-commons-runtime:4.1.2` | istack commons runtime (Eclipse EE4J) | BSD-3-Clause | [istack-commons-runtime-4.1.2-sources.jar](https://repo1.maven.org/maven2/com/sun/istack/istack-commons-runtime/4.1.2/istack-commons-runtime-4.1.2-sources.jar) | jaxb-runtime-NOTICE.txt |
| `commons-io:commons-io:2.21.0` | Apache Commons IO | Apache-2.0 | [commons-io-2.21.0-sources.jar](https://repo1.maven.org/maven2/commons-io/commons-io/2.21.0/commons-io-2.21.0-sources.jar) | commons-io-NOTICE.txt |
| `jakarta.activation:jakarta.activation-api:2.1.4` | Jakarta Activation API | BSD-3-Clause | [jakarta.activation-api-2.1.4-sources.jar](https://repo1.maven.org/maven2/jakarta/activation/jakarta.activation-api/2.1.4/jakarta.activation-api-2.1.4-sources.jar) | jakarta.activation-api-NOTICE.txt |
| `jakarta.xml.bind:jakarta.xml.bind-api:4.0.4` | Jakarta XML Binding API | BSD-3-Clause | [jakarta.xml.bind-api-4.0.4-sources.jar](https://repo1.maven.org/maven2/jakarta/xml/bind/jakarta.xml.bind-api/4.0.4/jakarta.xml.bind-api-4.0.4-sources.jar) | jakarta.xml.bind-api-NOTICE.txt |
| `org.apache.commons:commons-lang3:3.20.0` | Apache Commons Lang | Apache-2.0 | [commons-lang3-3.20.0-sources.jar](https://repo1.maven.org/maven2/org/apache/commons/commons-lang3/3.20.0/commons-lang3-3.20.0-sources.jar) | commons-lang3-NOTICE.txt |
| `org.eclipse.angus:angus-activation:2.0.3` | Eclipse Angus Activation | BSD-3-Clause | [angus-activation-2.0.3-sources.jar](https://repo1.maven.org/maven2/org/eclipse/angus/angus-activation/2.0.3/angus-activation-2.0.3-sources.jar) | angus-activation-NOTICE.txt |
| `org.fusesource.jansi:jansi:2.4.2` | Jansi | Apache-2.0 | [jansi-2.4.2-sources.jar](https://repo1.maven.org/maven2/org/fusesource/jansi/jansi/2.4.2/jansi-2.4.2-sources.jar) | apache-2.0-LICENSE.txt |
| `org.glassfish.jaxb:jaxb-core:4.0.6` | Eclipse Implementation of JAXB | BSD-3-Clause | [jaxb-core-4.0.6-sources.jar](https://repo1.maven.org/maven2/org/glassfish/jaxb/jaxb-core/4.0.6/jaxb-core-4.0.6-sources.jar) | jaxb-runtime-NOTICE.txt |
| `org.glassfish.jaxb:jaxb-runtime:4.0.6` | Eclipse Implementation of JAXB | BSD-3-Clause | [jaxb-runtime-4.0.6-sources.jar](https://repo1.maven.org/maven2/org/glassfish/jaxb/jaxb-runtime/4.0.6/jaxb-runtime-4.0.6-sources.jar) | jaxb-runtime-NOTICE.txt |
| `org.glassfish.jaxb:txw2:4.0.6` | Eclipse Implementation of JAXB | BSD-3-Clause | [txw2-4.0.6-sources.jar](https://repo1.maven.org/maven2/org/glassfish/jaxb/txw2/4.0.6/txw2-4.0.6-sources.jar) | jaxb-runtime-NOTICE.txt |
| `org.kosit:validator:1.6.2` | KoSIT XML Validator | Apache-2.0 | [validator-1.6.2-sources.jar](https://repo1.maven.org/maven2/org/kosit/validator/1.6.2/validator-1.6.2-sources.jar) | validator-NOTICE.txt, validator-LICENSE.txt, validator-ui-LICENSE.txt |
| `org.slf4j:slf4j-api:2.0.17` | SLF4J | MIT | [slf4j-api-2.0.17-sources.jar](https://repo1.maven.org/maven2/org/slf4j/slf4j-api/2.0.17/slf4j-api-2.0.17-sources.jar) | slf4j-LICENSE.txt |
| `org.slf4j:slf4j-simple:2.0.17` | SLF4J | MIT | [slf4j-simple-2.0.17-sources.jar](https://repo1.maven.org/maven2/org/slf4j/slf4j-simple/2.0.17/slf4j-simple-2.0.17-sources.jar) | slf4j-LICENSE.txt |

## C. Ohne `pom.properties` eingebaute Komponenten

Diese Bibliotheken bringen keine `pom.properties` mit (Gradle-/Ant-Builds). Die Version ist jeweils
durch eine Versionskennung im JAR, das deklarierende POM und den Bytevergleich belegt.

| Maven-Koordinate | Komponente | in | SPDX | Quelle | Beleg | Hinweise in `notices/` |
|---|---|---|---|---|---|---|
| `net.sf.saxon:Saxon-HE:12.8` | Saxon-HE (XSLT/XPath/XQuery), `net/sf/saxon/**` | M | MPL-2.0 (XPath-Parser-Reste MIT, James Clark) | [Saxon-HE-12.8-sources.jar](https://repo1.maven.org/maven2/net/sf/saxon/Saxon-HE/12.8/Saxon-HE-12.8-sources.jar), [saxon12-8source.zip](https://github.com/Saxonica/Saxon-HE/releases/download/SaxonHE-12-8/saxon12-8source.zip) | `net/sf/saxon/Version.class` enthält „12.8“; Mustang `core-2.23.0.pom`: `version.net.sf.saxon=12.8`; 2600/2600 Klassen bytegleich mit `Saxon-HE-12.8.jar` | Saxon-HE-LICENSE.txt, Saxon-HE-NOTICE.txt |
| `net.sf.saxon:Saxon-HE:12.9` | Saxon-HE, `net/sf/saxon/**` | K | MPL-2.0 (XPath-Parser-Reste MIT, James Clark) | [Saxon-HE-12.9-sources.jar](https://repo1.maven.org/maven2/net/sf/saxon/Saxon-HE/12.9/Saxon-HE-12.9-sources.jar), [saxon12-9source.zip](https://github.com/Saxonica/Saxon-HE/releases/download/SaxonHE12-9/saxon12-9source.zip) | `Version.class` enthält „12.9“; `validator-1.6.2.pom`: `version.saxon-he=12.9`; 2600/2600 Klassen bytegleich mit `Saxon-HE-12.9.jar` | Saxon-HE-LICENSE.txt, Saxon-HE-NOTICE.txt |
| `org.xmlresolver:xmlresolver:5.3.3` | XML Resolver (Abhängigkeit von Saxon-HE), `org/xmlresolver/**` | M, K | Apache-2.0 | [xmlresolver-5.3.3-sources.jar](https://repo1.maven.org/maven2/org/xmlresolver/xmlresolver/5.3.3/xmlresolver-5.3.3-sources.jar) | `org/xmlresolver/BuildConfig.class` enthält „5.3.3“; Saxon-HE-POMs 12.8/12.9 deklarieren 5.3.3; 90/90 Klassen bytegleich | apache-2.0-LICENSE.txt |
| `org.xmlresolver:xmlresolver:jar:data:5.3.3` | XML Resolver, Datenpaket (W3C-DTDs/-Schemata unter `org/xmlresolver/www.w3.org/…`) | M, K | Apache-2.0 AND W3C | [xmlresolver-5.3.3-data.jar](https://repo1.maven.org/maven2/org/xmlresolver/xmlresolver/5.3.3/xmlresolver-5.3.3-data.jar) (Daten sind Quellform) | 372/372 Ressourcen bytegleich mit `xmlresolver-5.3.3-data.jar` | xmlresolver-data-LICENSE.txt |
| `org.mozilla:rhino:1.7.13` | Mozilla Rhino (JavaScript für veraPDF-Regeln), `org/mozilla/**` | M | MPL-2.0 AND BSD-3-Clause (V8-dtoa) | [rhino-1.7.13-sources.jar](https://repo1.maven.org/maven2/org/mozilla/rhino/1.7.13/rhino-1.7.13-sources.jar), Tag [Rhino1_7_13_Release](https://github.com/mozilla/rhino/tree/Rhino1_7_13_Release) | `verapdf-library-jakarta-1.26.5.pom` pinnt `rhino` 1.7.13 (genutzt von `core-jakarta`); 506/506 Klassen bytegleich mit `rhino-1.7.13.jar`; `LICENSE.txt` im Mustang-JAR identisch mit dem aus `rhino-1.7.13.jar` (die Versionskennung steht nur im fehlenden Manifest) | rhino-LICENSE.txt, rhino-NOTICE.txt |
| `org.dom4j:dom4j:2.1.5` | dom4j, `org/dom4j/**` | M | Plexus (BSD-artige dom4j-Lizenz; POM: „Plexus“) | [dom4j-2.1.5-sources.jar](https://repo1.maven.org/maven2/org/dom4j/dom4j/2.1.5/dom4j-2.1.5-sources.jar) | Mustang `core-2.23.0.pom`: `version.org.dom4j=2.1.5`; 184/184 Klassen bytegleich mit `dom4j-2.1.5.jar` | dom4j-LICENSE.txt |
| `net.java.dev.stax-utils:stax-utils:20070216` | StAX Utilities (Abhängigkeit von veraPDF `core-jakarta`), `javanet/staxutils/**` | M | BSD-3-Clause | [stax-utils-20070216-sources.jar](https://repo1.maven.org/maven2/net/java/dev/stax-utils/stax-utils/20070216/stax-utils-20070216-sources.jar) | `core-jakarta-1.26.5.pom` deklariert 20070216; 82/82 Klassen bytegleich | stax-utils-LICENSE.txt |
| `xml-apis:xml-apis-ext:1.3.04` | Apache XML Commons External (SVG-DOM, SMIL, SAC; Abhängigkeit von Batik), `org/w3c/dom/svg`, `org/w3c/dom/smil`, `org/w3c/css/sac`, `license/**` | M | Apache-2.0 AND W3C AND W3C-19980720 | [xml-commons-external-1.3.04-src.tar.gz](https://archive.apache.org/dist/xml/commons/source/xml-commons-external-1.3.04-src.tar.gz) (kein `-sources.jar` auf Maven Central) | `batik-1.19.pom`: `xmlapisext.version=1.3.04`; 192/192 Klassen und alle `license/*`-Dateien bytegleich mit `xml-apis-ext-1.3.04.jar` | xml-apis-ext-NOTICE.txt |
| `info.picocli:picocli:4.7.7` | picocli (Kommandozeile des Validators), `picocli/**` | K | Apache-2.0 | [picocli-4.7.7-sources.jar](https://repo1.maven.org/maven2/info/picocli/picocli/4.7.7/picocli-4.7.7-sources.jar) | `picocli/CommandLine.class` enthält „4.7.7“; `validator-1.6.2.pom` deklariert 4.7.7; 228/228 Klassen bytegleich | apache-2.0-LICENSE.txt |

Eingebettet in andere Artefakte (ohne eigene Zeile, Hinweise beim Trägerartefakt): ASM in
Byte Buddy und – über cglib – in AssertJ (BSD-3-Clause, Text in byte-buddy-NOTICE.txt);
FastDoubleParser, fast_float und bigint in jackson-core (MIT, BSD-2-Clause); die aus dem SVRL-Schema
generierten JAXB-Klassen `org/oclc/purl/dsdl/svrl/**` gehören zu `org.kosit:validator:1.6.2`.

## D. Eingebettete Daten, Schriften und Regelwerke

Keine Java-Bibliotheken, aber mitverbreitet. Herkunft per Dateivergleich mit den Originalartefakten
ermittelt.

| Bestandteil | Ort | Herkunft / Version | SPDX | Quelle | Hinweise in `notices/` |
|---|---|---|---|---|---|
| CEN-Schematron EN 16931, kompiliert (UBL, CII) | `xrechnung-config/resources/{ubl/2.1,cii/16b}/xsl/EN16931-*-validation.xsl` (und Kopien im Wurzelverzeichnis) | ConnectingEurope/eInvoicing-EN16931 `validation-1.3.15`, mit SchXslt 1.10.1 / Saxon-HE 12.8 übersetzt (laut `CHANGELOG.md` und `rdf:Description` der Dateien) | EUPL-1.2 | [Tag-Archiv validation-1.3.15](https://github.com/ConnectingEurope/eInvoicing-EN16931/archive/refs/tags/validation-1.3.15.tar.gz) | eInvoicing-EN16931-LICENSE.txt, schxslt-LICENSE.txt |
| CEN-Schematron EN 16931, kompiliert (UBL, CII) | M: `xslt/en16931schematron/EN16931-*-validation.xslt` (aus `org.mustangproject:validator:2.23.0`) | `validation-1.3.12` (Kopfkommentar „Schematron version 1.3.12 - Last update: 2024-04-06“) | EUPL-1.2 | [Tag-Archiv validation-1.3.12](https://github.com/ConnectingEurope/eInvoicing-EN16931/archive/refs/tags/validation-1.3.12.tar.gz) | eInvoicing-EN16931-LICENSE.txt |
| XRechnung-Schematron, kompiliert | `xrechnung-config/resources/xrechnung/3.0.2/xsl/XRechnung-*-validation.xsl` | itplr-kosit/xrechnung-schematron 2.5.0 (Kennung „Version 2.5.0“ in den Dateien) | Apache-2.0 | [Tag v2.5.0](https://github.com/itplr-kosit/xrechnung-schematron/tree/v2.5.0) | apache-2.0-LICENSE.txt |
| XRechnung-Schematron (XR 1.2–3.0), Quell- und kompilierte Form | M: `schematron/XR_*/**`, `xslt/XR_*/**` (aus `org.mustangproject:validator:2.23.0`) | xrechnung-schematron, XR_30 = 2.4.0 (Kennung „Version 2.4.0“) | Apache-2.0 | [Tag release-2.4.0](https://github.com/itplr-kosit/xrechnung-schematron/tree/release-2.4.0) | apache-2.0-LICENSE.txt, mustang-NOTICE.txt |
| XRechnung-Visualisierung (XSLT nach HTML/PDF) | M: `stylesheets/**`, `xrechnung-viewer.{css,js}` (aus `org.mustangproject:library:2.23.0`) | itplr-kosit/xrechnung-visualization und jcthiele/xrechnung-visualization-codelist-resolve, Stand wie in Mustang 2.23.0 (keine Versionskennung in den Dateien) | Apache-2.0 | [Mustang core-2.23.0, library/src/main/resources](https://github.com/ZUGFeRD/mustangproject/tree/core-2.23.0/library/src/main/resources) | apache-2.0-LICENSE.txt, mustang-NOTICE.txt |
| Factur-X/ZUGFeRD-, Order-X- und ZUGFeRD-1-Schemata und -Schematron, französisches BR-FR-Schematron (`xslt/XP_Z12_012/…_V1.3.0.xsl`) | M: `schema/**`, `schematron/**`, `xslt/**`, `ZUGFeRD*.{sch,xsl}`, `zugferd2p0_*.sch` (aus `org.mustangproject:validator:2.23.0`) | FeRD / FNFE-MPE bzw. AIFE, Stand wie in Mustang 2.23.0 | NOASSERTION (die Dateien tragen keine Lizenzangabe; verbreitet als Teil von Mustang unter Apache-2.0) | [Mustang core-2.23.0, validator/src/main/resources](https://github.com/ZUGFeRD/mustangproject/tree/core-2.23.0/validator/src/main/resources) | mustang-NOTICE.txt |
| ISO-Schematron-Skelett (XSLT 1/2) | M: `iso_*.xsl` (Mustang validator), `external/schematron/**` (ph-schematron-xslt) | Rick Jelliffe und Academia Sinica Computing Center, 2001–2010 (Stand 2010-07-10) | Zlib (Lizenz im Kopf jeder Datei) | in den jeweiligen `-sources.jar` | – (Kopf in der Datei) |
| OASIS UBL 2.1 XML-Schemata | `xrechnung-config/resources/ubl/2.1/xsd/**`; M: `schema/UBL_21/**`, `external/schemas/ubl2{1,2,3,4}/**` | OASIS UBL 2.1 OS (2013-11-04), in ph-ubl auch 2.2–2.4 | LicenseRef-OASIS-Copyright (Weitergabe unveränderter Kopien mit Hinweis) | <http://docs.oasis-open.org/ubl/os-UBL-2.1/> | oasis-ubl-2.1-NOTICE.txt |
| UN/CEFACT CII D16B XML-Schemata | `xrechnung-config/resources/cii/16b/xsd/**`; M: `external/schemas/**` (ph-cii-d16b) | UN/CEFACT 100.D16B (2016-10-10) | LicenseRef-UNCEFACT-Copyright (Weitergabe unveränderter Kopien mit Hinweis) | <https://unece.org/trade/uncefact/xml-schemas> | uncefact-d16b-NOTICE.txt |
| Schrift FreeSans | M: `FreeSans.ttf` (aus `org.mustangproject:library:2.23.0`) | GNU FreeFont 20100919 (Revision 1.344; SHA-256 bytegleich mit `freefont-ttf-20100919.tar.gz`) | GPL-3.0-or-later WITH Font-exception-2.0 | [freefont-sfd-20100919.tar.gz](https://ftp.gnu.org/gnu/freefont/freefont-sfd-20100919.tar.gz) | freefont-LICENSE.txt |
| Schriften Source Sans Pro 3.006, Source Serif Pro 3.001 | M: `fonts/*.ttf` (aus `org.mustangproject:library:2.23.0`) | Adobe | OFL-1.1 | [adobe-fonts/source-sans 3.006R](https://github.com/adobe-fonts/source-sans/tree/3.006R), [adobe-fonts/source-serif 3.001R](https://github.com/adobe-fonts/source-serif/tree/3.001R) | SourceSansPro-LICENSE.txt, SourceSerifPro-LICENSE.txt |
| ICC-Profile `sRGB.icc` (sRGB2014), `AdobeCompat-v2.icc` | M: Wurzel (aus `org.mustangproject:library:2.23.0`) | International Color Consortium; saucecontrol/Compact-ICC-Profiles | LicenseRef-ICC-Profile; CC0-1.0 | <https://www.color.org/srgbprofiles.xalter>, <https://github.com/saucecontrol/Compact-ICC-Profiles> | mustang-icc-profiles-NOTICE.txt |
| ICC-Profil `org/apache/fop/pdf/sRGB.icc` | M (aus `fop-core` 2.11) | littleCMS (Marti Maria) | Zlib | in `fop-core-2.11-sources.jar` | fop-core-sRGB-icc-LICENSE.txt |
| Adobe-CMaps, Standard-14-AFM, Glyphlist | M: `font/**` (veraPDF `parser`), `org/apache/fontbox/**`, `org/apache/pdfbox/resources/**` | Adobe Systems | BSD-3-Clause, APAFML, Adobe-Glyph | in den jeweiligen `-sources.jar` | pdfbox-LICENSE.txt, fontbox-LICENSE.txt |
| Web-Oberfläche des Validators (Daemon-Modus; React, NProgress) | K: `ui/**` | KoSIT-Validator 1.6.2 | MIT (gebündelte JS-Bibliotheken), Apache-2.0 | [Tag v1.6.2](https://github.com/itplr-kosit/validator/tree/v1.6.2) | validator-ui-LICENSE.txt |
| Validator-Konfiguration XRechnung (Szenarien, Berichts-XSLT, `README.md`, `docs/`) | `xrechnung-config/` | itplr-kosit/validator-configuration-xrechnung `v2026-01-31` | Apache-2.0 | [Tag v2026-01-31](https://github.com/itplr-kosit/validator-configuration-xrechnung/tree/v2026-01-31) (die Datei `LICENSE` fehlt im ZIP) | validator-configuration-xrechnung-LICENSE.txt, validator-configuration-xrechnung-NOTICE.txt |

## E. Copyleft-Teile und Quellangebot

Diese Teile stehen unter einer Lizenz mit Quelltextpflicht. Ihr Quelltext in genau der eingebauten
Version gehört ins Release-Asset `klein-buch-<Version>-quellen-dritter.tar.zst`; die Liste der zu
ladenden Quellen steht maschinenlesbar in `copyleft-sources.txt`.

| Teil | Lizenz (genutzt) | Quelle im Quellarchiv |
|---|---|---|
| Saxon-HE 12.8 (M), 12.9 (K) | MPL-2.0 | `-sources.jar` von Maven Central |
| Mozilla Rhino 1.7.13 (M) | MPL-2.0 | `-sources.jar` |
| veraPDF 1.26.5 / 1.26.1 (M, 7 Artefakte) | MPL-2.0 (Wahl aus GPL-3.0-or-later OR MPL-2.0) | `-sources.jar` je Artefakt |
| logback-core 1.5.18 (M) | EPL-1.0 (Wahl aus EPL-1.0 OR LGPL-2.1-only) | `-sources.jar` |
| CEN-Schematron validation-1.3.15 (xrechnung-config) und validation-1.3.12 (M) | EUPL-1.2 | Tag-Archive auf GitHub |
| GNU FreeFont FreeSans 20100919 (M) | GPL-3.0-or-later WITH Font-exception-2.0 | `freefont-sfd-20100919.tar.gz` (SFD = bevorzugte Form für Änderungen) |
| Eclipse Temurin 21.0.12+8 (jlink-Laufzeit) | GPL-2.0-only WITH Classpath-exception-2.0 | wird vom Quellarchiv-Schritt aus `versions.json` (`jdk.source_url`) geladen, nicht über `copyleft-sources.txt` |

Klein.Buch verändert keine dieser Komponenten; die JARs werden unverändert aus den in
`versions.json` genannten Quellen übernommen (SHA-256-Prüfung in CI).
