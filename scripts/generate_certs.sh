#!/usr/bin/env bash
# generate_certs.sh — Generate a self-signed CA and server certificate for
# Men Among Gods TLS (API HTTPS + game server TCP-TLS).
#
# Usage:
#   ./scripts/generate_certs.sh [--out DIR] [--san EXTRA_SANS]
#
# Examples:
#   ./scripts/generate_certs.sh
#   ./scripts/generate_certs.sh --out /etc/mag/certs
#   ./scripts/generate_certs.sh --san "DNS:play.example.com,IP:203.0.113.10"
#
# Output files (all PEM-encoded):
#   ca.key        — CA private key (keep secret)
#   ca.crt        — CA certificate (distribute to clients if needed)
#   server.key    — Server private key
#   server.crt    — Server certificate (signed by the CA)
#
# The generated certificate is valid for both the API (HTTPS) and game server
# (TLS-over-TCP). Set the env vars to point both services at the same files:
#   API_TLS_CERT=./certs/server.crt  API_TLS_KEY=./certs/server.key
#   SERVER_TLS_CERT=./certs/server.crt  SERVER_TLS_KEY=./certs/server.key

set -euo pipefail

OUT_DIR="./certs"
EXTRA_SAN=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --out)  OUT_DIR="$2"; shift 2 ;;
        --san)  EXTRA_SAN="$2"; shift 2 ;;
        *)      echo "Unknown option: $1"; exit 1 ;;
    esac
done

mkdir -p "$OUT_DIR"

# Default Subject Alternative Names
BASE_SAN="DNS:localhost,DNS:menamonggods.ddns.net,IP:127.0.0.1"
if [[ -n "$EXTRA_SAN" ]]; then
    SAN="${BASE_SAN},${EXTRA_SAN}"
else
    SAN="$BASE_SAN"
fi

echo "==> Generating CA key + certificate..."
openssl req -x509 -newkey rsa:4096 -nodes \
    -keyout "$OUT_DIR/ca.key" \
    -out "$OUT_DIR/ca.crt" \
    -days 3650 \
    -subj "/CN=MenAmongGods Self-Signed CA"

echo "==> Generating server key + CSR..."
openssl req -newkey rsa:2048 -nodes \
    -keyout "$OUT_DIR/server.key" \
    -out "$OUT_DIR/server.csr" \
    -subj "/CN=MenAmongGods Server"

echo "==> Signing server certificate with CA (SAN: $SAN)..."
openssl x509 -req \
    -in "$OUT_DIR/server.csr" \
    -CA "$OUT_DIR/ca.crt" \
    -CAkey "$OUT_DIR/ca.key" \
    -CAcreateserial \
    -out "$OUT_DIR/server.crt" \
    -days 365 \
    -extfile <(printf "subjectAltName=%s\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth" "$SAN")

# Clean up intermediate files
rm -f "$OUT_DIR/server.csr" "$OUT_DIR/ca.srl"

echo ""
echo "==> Certificates generated in $OUT_DIR/"
echo "    ca.key       — CA private key"
echo "    ca.crt       — CA certificate"
echo "    server.key   — Server private key"
echo "    server.crt   — Server certificate"
echo ""
echo "    Server cert fingerprint (SHA-256):"
openssl x509 -in "$OUT_DIR/server.crt" -noout -fingerprint -sha256
echo ""
echo "To use:"
echo "  export API_TLS_CERT=$OUT_DIR/server.crt API_TLS_KEY=$OUT_DIR/server.key"
echo "  export SERVER_TLS_CERT=$OUT_DIR/server.crt SERVER_TLS_KEY=$OUT_DIR/server.key"
