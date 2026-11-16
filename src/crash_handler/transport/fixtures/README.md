# Local HTTPS test identity

`localhost.p12` is a deliberately public test-only server identity, password
`test-only`, for DNS `localhost` only. `ca.pem` is its private test CA's public
certificate. Neither is an application credential; never deploy this identity.
The CA private key was not saved. Generated with Python cryptography 46.0.7:
RSA-2048/SHA-256, CA BasicConstraints/keyCertSign, serverAuth EKU, explicit key
usages, SKI/AKI and localhost SAN. CA validity 2020–2045; the leaf is valid for
730 days (November 2026–November 2028), below Apple's 825-day TLS limit:
https://support.apple.com/en-us/103769. Regenerate before leaf expiry with
`python generate.py` (cryptography is a maintenance dependency only).
The original 20-year leaf passed Windows/Linux; macOS HTTPS tests failed before
any HTTP request. Its lifetime violated the documented Apple TLS policy; exact
runtime causality is inferred until the refreshed fixture passes macOS CI.
Production certificate/hostname checks must never be bypassed for test fixtures.
PKCS#12 uses PBESv1
SHA1/3DES for native Windows/macOS/OpenSSL import compatibility.

Tests add the CA only to their client builder. They do not modify machine trust,
disable TLS verification, connect to external servers or require Python at runtime.
The IP-hostname negative test deliberately uses 127.0.0.1, absent from the SAN.
