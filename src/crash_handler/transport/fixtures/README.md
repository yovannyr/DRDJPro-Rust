# Local HTTPS test identity

`localhost.p12` is a deliberately public test-only server identity, password
`test-only`, for DNS `localhost` only. `ca.pem` is its private test CA's public
certificate. Neither is an application credential; never deploy this identity.
The CA private key was not saved. Generated with Python cryptography 46.0.7:
RSA-2048/SHA-256, CA BasicConstraints/keyCertSign, serverAuth EKU and localhost
SAN. CA validity 2020–2045, leaf validity 2020–2040. PKCS#12 uses PBESv1
SHA1/3DES for native Windows/macOS/OpenSSL import compatibility.

Tests add the CA only to their client builder. They do not modify machine trust,
disable TLS verification, connect to external servers or require Python at runtime.
The IP-hostname negative test deliberately uses 127.0.0.1, absent from the SAN.
