#!/bin/sh
# Klein.Buch Java-Sidecar (Plan W11): KoSIT-Validator und Mustang mit der
# Laufzeit aus dem Image. Aufruf: klein-buch-java.sh <validator|mustang-zugferd> [Argumente]
# Das Subcommand-Wort geht nicht an das JAR (shift). JAVA_HOME setzt das Image;
# lokal (Tests) zeigt KLEIN_BUCH_JAVA auf ein beliebiges java.
set -eu
SELF_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
LIB="$SELF_DIR/lib"
CFG="$SELF_DIR/xrechnung-config"
if [ -n "${KLEIN_BUCH_JAVA:-}" ]; then
  JAVA=$KLEIN_BUCH_JAVA
elif [ -x "$SELF_DIR/jre/bin/java" ]; then
  JAVA="$SELF_DIR/jre/bin/java"
elif [ -n "${JAVA_HOME:-}" ]; then
  JAVA="$JAVA_HOME/bin/java"
else
  JAVA=java
fi
TMP="${TMPDIR:-/tmp}"
JVM_OPTS="-XX:-UsePerfData -Xmx512m -Djava.io.tmpdir=$TMP -Djava.awt.headless=true"

cmd="${1:-}"
case "$cmd" in
  validator)
    shift
    # shellcheck disable=SC2086
    exec "$JAVA" $JVM_OPTS -jar "$LIB/kosit-validator.jar" -r "$CFG" "$@"
    ;;
  mustang-zugferd)
    shift
    # shellcheck disable=SC2086
    exec "$JAVA" $JVM_OPTS -jar "$LIB/mustang.jar" "$@"
    ;;
  *)
    echo "Klein.Buch Java-Sidecar" >&2
    echo "Aufruf: $0 <validator|mustang-zugferd> [Argumente]" >&2
    exit 1
    ;;
esac
