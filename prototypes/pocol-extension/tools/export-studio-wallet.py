#!/usr/bin/env python3
"""Export an explicitly selected native Linux PoCol key as encrypted JSON.

Requires Python 3 and cryptography (Debian/Ubuntu package: python3-cryptography).
Run as the owner of the native key. No network, default key path, wallet changes,
command-line password, or plaintext key output. Import the result only inside
the PoCol extension's trusted wallet confirmation tab.
"""
from __future__ import annotations

import argparse
import base64
import getpass
import hashlib
import hmac
import json
import os
from pathlib import Path
import secrets
import stat
import sys
import warnings

try:
    from cryptography.hazmat.primitives import padding
    from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
except ImportError:
    padding = Cipher = algorithms = modes = None


ITERATIONS = 600_000
SECP256K1_ORDER = int("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141", 16)
AUTH_PREFIX = b"PoCol-Studio-export-v1\0"


class ExportError(Exception):
    """An expected failure that can be reported without exposing key material."""


def require_linux() -> None:
    if not sys.platform.startswith("linux") or not hasattr(os, "O_NOFOLLOW"):
        raise ExportError("This helper requires Linux. Use Export-Studio-Wallet.ps1 for a Windows DPAPI wallet.")


def check_key_stat(info: os.stat_result) -> None:
    if not stat.S_ISREG(info.st_mode):
        raise ExportError("Select an original regular PoCol key file.")
    if info.st_uid != os.geteuid():
        raise ExportError("Run this helper as the key file's owner. Root does not bypass this check.")
    if info.st_nlink != 1:
        raise ExportError("The native key must have exactly one hard link.")
    if stat.S_IMODE(info.st_mode) not in (0o600, 0o400):
        raise ExportError("The native key must have mode 0600 or 0400. This helper does not change its permissions.")
    if info.st_size != 32:
        raise ExportError("Expected a native Linux PoCol key of exactly 32 bytes. Windows DPAPI files need the Windows exporter.")


def valid_secret(secret: bytes | bytearray) -> None:
    if len(secret) != 32 or not 0 < int.from_bytes(secret, "big") < SECP256K1_ORDER:
        raise ExportError("The selected file does not contain a valid secp256k1 private key.")


def read_native_key(path: str | Path) -> bytearray:
    """Read only the explicit key path, validating the opened inode before/after."""
    require_linux()
    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | getattr(os, "O_CLOEXEC", 0)
    secret = bytearray()
    try:
        fd = os.open(path, flags)
    except OSError as error:
        raise ExportError("Cannot open the selected native key. It must exist, be accessible, and not be a symbolic link.") from error
    try:
        before = os.fstat(fd)
        check_key_stat(before)
        while len(secret) < 33:
            chunk = os.read(fd, 33 - len(secret))
            if not chunk:
                break
            secret.extend(chunk)
        after = os.fstat(fd)
        check_key_stat(after)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns
        ):
            raise ExportError("The native key changed while being read. Export stopped.")
        valid_secret(secret)
        return secret
    except BaseException:
        secret[:] = b"\0" * len(secret)
        raise
    finally:
        os.close(fd)


def prompt_password() -> str:
    if not sys.stdin.isatty():
        raise ExportError("Run interactively in a terminal. Passwords are never accepted through arguments, pipes, or environment variables.")
    # Refuse getpass's echoing fallback if a terminal cannot safely hide input.
    with warnings.catch_warnings():
        warnings.simplefilter("error", getpass.GetPassWarning)
        try:
            password = getpass.getpass("Choose an export password (at least 10 characters): ")
            confirmation = getpass.getpass("Repeat the export password: ")
        except getpass.GetPassWarning as error:
            raise ExportError("A terminal with hidden password input is required.") from error
    if len(password) < 10:
        raise ExportError("Use at least 10 characters.")
    if password != confirmation:
        raise ExportError("Passwords do not match.")
    return password


def encrypt_export(secret: bytes | bytearray, password: str) -> dict:
    """The exact pocol-studio-key-v1 format accepted by studio-keyfile.js."""
    if Cipher is None:
        raise ExportError("Missing Python dependency: cryptography. Install your distribution's python3-cryptography package.")
    valid_secret(secret)
    if not isinstance(password, str) or len(password) < 10:
        raise ExportError("Use at least 10 characters.")
    salt, iv = secrets.token_bytes(16), secrets.token_bytes(16)
    derived = bytearray(hashlib.pbkdf2_hmac("sha256", password.encode("utf-8"), salt, ITERATIONS, dklen=64))
    try:
        padder = padding.PKCS7(128).padder()
        padded = padder.update(bytes(secret)) + padder.finalize()
        encryptor = Cipher(algorithms.AES(bytes(derived[:32])), modes.CBC(iv)).encryptor()
        ciphertext = encryptor.update(padded) + encryptor.finalize()
        mac = hmac.digest(bytes(derived[32:]), AUTH_PREFIX + salt + iv + ciphertext, "sha256")
        b64 = lambda value: base64.b64encode(value).decode("ascii")
        return {
            "schema": "pocol-studio-key-v1", "kdf": "PBKDF2-SHA256",
            "iterations": ITERATIONS, "cipher": "AES-256-CBC-HMAC-SHA256",
            "salt": b64(salt), "iv": b64(iv), "ciphertext": b64(ciphertext), "mac": b64(mac),
        }
    finally:
        # Best effort for mutable buffers; Python/crypto libraries may make copies.
        derived[:] = b"\0" * len(derived)


def write_export(path: str | Path, box: dict) -> None:
    """Create a new 0600 encrypted file without replacing any existing entry."""
    require_linux()
    payload = (json.dumps(box, indent=2, ensure_ascii=True) + "\n").encode("utf-8")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
    try:
        fd = os.open(path, flags, 0o600)
    except FileExistsError as error:
        raise ExportError("Output already exists. Choose a new --output path; existing files are never overwritten.") from error
    except OSError as error:
        raise ExportError("Cannot create the encrypted output. Choose a writable existing directory owned or accessible by the current user.") from error
    try:
        os.fchmod(fd, 0o600)
        remaining = memoryview(payload)
        while remaining:
            written = os.write(fd, remaining)
            if written <= 0:
                raise OSError("Short write")
            remaining = remaining[written:]
        os.fsync(fd)
    except OSError as error:
        raise ExportError("Encrypted output could not be saved completely. A partial encrypted file may exist; choose a new output path when retrying.") from error
    finally:
        os.close(fd)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--key-path", required=True, type=Path, help="Explicit path to the native Linux identity.pocol-key; run as its owner.")
    parser.add_argument("--output", required=True, type=Path, help="New password-encrypted JSON file; never overwritten.")
    args = parser.parse_args(argv)
    secret = None
    try:
        require_linux()
        if Cipher is None:
            raise ExportError("Missing Python dependency: cryptography. Install your distribution's python3-cryptography package.")
        if os.path.lexists(args.output):
            raise ExportError("Output already exists. Choose a new --output path; existing files are never overwritten.")
        # Collect hidden input before loading any private bytes into memory.
        password = prompt_password()
        secret = read_native_key(args.key_path)
        box = encrypt_export(secret, password)
        write_export(args.output, box)
        print("Encrypted wallet export created:")
        print(args.output)
        print("Import this JSON only in the PoCol extension confirmation tab. Keep your export password private.")
        return 0
    except (ExportError, OSError) as error:
        print(f"Export failed: {error}", file=sys.stderr)
        return 1
    except (KeyboardInterrupt, EOFError):
        print("Export cancelled.", file=sys.stderr)
        return 1
    finally:
        if secret is not None:
            secret[:] = b"\0" * len(secret)


if __name__ == "__main__":
    raise SystemExit(main())
