#!/usr/bin/env bash
# Creates an Augmont self-signed CA + code-signing certificate (PKCS#12).
# Used to Authenticode-sign Windows .exe/.msi and codesign macOS .app/.dmg.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CERT_DIR="$ROOT/certs"
mkdir -p "$CERT_DIR"

if [[ -f "$CERT_DIR/augmont-codesign.p12" ]]; then
  echo "Already exists: $CERT_DIR/augmont-codesign.p12"
  exit 0
fi

PASS_FILE="$CERT_DIR/PASSWORD.txt"
if [[ ! -f "$PASS_FILE" ]]; then
  openssl rand -base64 24 | tr -d '\n' > "$PASS_FILE"
fi
PASS="$(cat "$PASS_FILE")"

DAYS=3650
SUBJ_CA="/C=IN/ST=Maharashtra/L=Mumbai/O=Augmont Goldtech/OU=LGD FTP DATA SYNC/CN=Augmont Code Signing CA"
SUBJ_CS="/C=IN/ST=Maharashtra/L=Mumbai/O=Augmont Goldtech/OU=LGD FTP DATA SYNC/CN=Augmont Goldtech"

cat > "$CERT_DIR/ca.cnf" <<'EOF'
[req]
distinguished_name = req_distinguished_name
x509_extensions = v3_ca
prompt = no
[req_distinguished_name]
CN = placeholder
[v3_ca]
basicConstraints = critical, CA:true
keyUsage = critical, keyCertSign, cRLSign
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer
EOF

cat > "$CERT_DIR/codesign.cnf" <<'EOF'
[req]
distinguished_name = req_distinguished_name
prompt = no
[req_distinguished_name]
CN = placeholder
[v3_codesign]
basicConstraints = CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = codeSigning, msCodeInd, msCodeCom
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer
EOF

openssl genrsa -out "$CERT_DIR/ca.key" 4096
openssl req -new -x509 -days "$DAYS" -key "$CERT_DIR/ca.key" -out "$CERT_DIR/augmont-ca.crt" \
  -subj "$SUBJ_CA" -config "$CERT_DIR/ca.cnf" -extensions v3_ca

openssl genrsa -out "$CERT_DIR/codesign.key" 4096
openssl req -new -key "$CERT_DIR/codesign.key" -out "$CERT_DIR/codesign.csr" -subj "$SUBJ_CS"
openssl x509 -req -days "$DAYS" -in "$CERT_DIR/codesign.csr" \
  -CA "$CERT_DIR/augmont-ca.crt" -CAkey "$CERT_DIR/ca.key" -CAcreateserial \
  -out "$CERT_DIR/augmont-codesign.crt" -extfile "$CERT_DIR/codesign.cnf" -extensions v3_codesign

cat "$CERT_DIR/augmont-codesign.crt" "$CERT_DIR/augmont-ca.crt" > "$CERT_DIR/augmont-codesign-chain.crt"

openssl pkcs12 -export \
  -inkey "$CERT_DIR/codesign.key" \
  -in "$CERT_DIR/augmont-codesign.crt" \
  -certfile "$CERT_DIR/augmont-ca.crt" \
  -name "Augmont Goldtech" \
  -out "$CERT_DIR/augmont-codesign.p12" \
  -passout "pass:$PASS"

cp "$CERT_DIR/augmont-codesign.p12" "$CERT_DIR/augmont-codesign.pfx"

openssl x509 -in "$CERT_DIR/augmont-codesign.crt" -noout -fingerprint -sha1 \
  | sed 's/^.*=//;s/://g' > "$CERT_DIR/windows-thumbprint.txt"

base64 < "$CERT_DIR/augmont-codesign.p12" | tr -d '\n' > "$CERT_DIR/augmont-codesign.p12.base64.txt"

echo "Created:"
echo "  CA (install on PCs so Windows trusts the EXE): $CERT_DIR/augmont-ca.crt"
echo "  PKCS#12 (signing key):                         $CERT_DIR/augmont-codesign.p12"
echo "  Password:                                      $CERT_DIR/PASSWORD.txt"
echo "  Windows thumbprint:                            $(cat "$CERT_DIR/windows-thumbprint.txt")"
