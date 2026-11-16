"""Regenerate PUBLIC local TLS fixtures; never use these keys in production.

Maintenance only: requires Python cryptography, not part of build/test execution.
"""
from datetime import datetime, timedelta, timezone
from pathlib import Path

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives.serialization import pkcs12
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID

directory = Path(__file__).resolve().parent
now = datetime.now(timezone.utc).replace(microsecond=0)
ca_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
ca_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "DRDJ TEST ONLY CA")])
ca_ski = x509.SubjectKeyIdentifier.from_public_key(ca_key.public_key())
ca = (
    x509.CertificateBuilder()
    .subject_name(ca_name).issuer_name(ca_name).public_key(ca_key.public_key())
    .serial_number(x509.random_serial_number())
    .not_valid_before(datetime(2020, 1, 1, tzinfo=timezone.utc))
    .not_valid_after(datetime(2045, 1, 1, tzinfo=timezone.utc))
    .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
    .add_extension(x509.KeyUsage(False, False, False, False, False, True, True, False, False), critical=True)
    .add_extension(ca_ski, critical=False)
    .sign(ca_key, hashes.SHA256())
)
server_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
leaf = (
    x509.CertificateBuilder()
    .subject_name(x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "localhost")]))
    .issuer_name(ca_name).public_key(server_key.public_key())
    .serial_number(x509.random_serial_number())
    .not_valid_before(now - timedelta(days=1))
    .not_valid_after(now + timedelta(days=729))
    .add_extension(x509.BasicConstraints(ca=False, path_length=None), critical=True)
    .add_extension(x509.SubjectAlternativeName([x509.DNSName("localhost")]), critical=False)
    .add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), critical=False)
    .add_extension(x509.KeyUsage(True, False, True, False, False, False, False, False, False), critical=True)
    .add_extension(x509.SubjectKeyIdentifier.from_public_key(server_key.public_key()), critical=False)
    .add_extension(x509.AuthorityKeyIdentifier.from_issuer_subject_key_identifier(ca_ski), critical=False)
    .sign(ca_key, hashes.SHA256())
)
encryption = (
    serialization.PrivateFormat.PKCS12.encryption_builder()
    .kdf_rounds(2000)
    .key_cert_algorithm(pkcs12.PBES.PBESv1SHA1And3KeyTripleDESCBC)
    .hmac_hash(hashes.SHA1()).build(b"test-only")
)
directory.joinpath("localhost.p12").write_bytes(
    pkcs12.serialize_key_and_certificates(b"DRDJ test localhost", server_key, leaf, [ca], encryption)
)
directory.joinpath("ca.pem").write_bytes(ca.public_bytes(serialization.Encoding.PEM))
print(f"Public test leaf: {leaf.not_valid_before_utc} to {leaf.not_valid_after_utc}")
print("CA private key was not saved; no machine trust store modified")
