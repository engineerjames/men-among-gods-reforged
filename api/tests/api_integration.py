#!/usr/bin/env python3
"""Basic integration tests for the API using only the Python standard library.

Run:
  python3 api/tests/api_integration.py --base-url http://127.0.0.1:5554
"""

import argparse
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request


def request_json(method, url, payload=None, headers=None, timeout=5):
    body = None
    if payload is not None:
        body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=body, method=method)
    req.add_header("Accept", "application/json")
    if payload is not None:
        req.add_header("Content-Type", "application/json")
    if headers:
        for key, value in headers.items():
            req.add_header(key, value)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = resp.read().decode("utf-8")
            return resp.status, data
    except urllib.error.HTTPError as exc:
        data = exc.read().decode("utf-8") if exc.fp else ""
        return exc.code, data


def assert_status(expected, actual, label):
    if expected != actual:
        raise AssertionError(f"{label}: expected {expected}, got {actual}")


def test_login_ok(base_url):
    status, body = request_json(
        "POST",
        f"{base_url}/login",
        payload={"username": "user", "password": "ignored"},
    )
    assert_status(200, status, "login status")
    data = json.loads(body or "{}")
    if "token" not in data:
        raise AssertionError("login response missing token")


def test_create_account_ok(base_url):
    payload = {
        "email": f"user{int(time.time())}@example.com",
        "username": f"user{int(time.time())}",
        "password": "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$ZmFrZWhhc2g",
    }
    status, body = request_json("POST", f"{base_url}/accounts", payload=payload)
    assert_status(201, status, "create account status")
    data = json.loads(body or "{}")
    if data.get("id") is None:
        raise AssertionError("create account response missing id")


def test_create_account_bad_email(base_url):
    payload = {
        "email": "bad-email",
        "username": "validuser",
        "password": "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$ZmFrZWhhc2g",
    }
    status, _ = request_json("POST", f"{base_url}/accounts", payload=payload)
    assert_status(400, status, "invalid email status")


def test_create_account_bad_password(base_url):
    payload = {
        "email": f"badpass{int(time.time())}@example.com",
        "username": f"badpass{int(time.time())}",
        "password": "plaintext-password",
    }
    status, _ = request_json("POST", f"{base_url}/accounts", payload=payload)
    assert_status(400, status, "invalid password status")


def test_create_account_duplicates(base_url):
    now = int(time.time())
    first_payload = {
        "email": f"dup{now}@example.com",
        "username": f"dupuser{now}",
        "password": "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$ZmFrZWhhc2g",
    }

    status, _ = request_json("POST", f"{base_url}/accounts", payload=first_payload)
    assert_status(201, status, "duplicate setup status")

    time.sleep(1.1)
    email_dup_payload = {
        "email": first_payload["email"],
        "username": f"dupuser{now}_new",
        "password": first_payload["password"],
    }
    status, _ = request_json("POST", f"{base_url}/accounts", payload=email_dup_payload)
    assert_status(409, status, "duplicate email status")

    time.sleep(1.1)
    username_dup_payload = {
        "email": f"dup{now}_new@example.com",
        "username": first_payload["username"],
        "password": first_payload["password"],
    }
    status, _ = request_json("POST", f"{base_url}/accounts", payload=username_dup_payload)
    assert_status(409, status, "duplicate username status")


def test_malformed_json(base_url):
    req = urllib.request.Request(f"{base_url}/accounts", method="POST")
    req.add_header("Content-Type", "application/json")
    req.add_header("Accept", "application/json")
    req.data = b"{not-json"
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            status = resp.status
    except urllib.error.HTTPError as exc:
        status = exc.code
    if status < 400:
        raise AssertionError("malformed JSON should return 4xx")


def test_rate_limit(base_url):
    payload = {"username": "user", "password": "ignored"}
    status1, _ = request_json("POST", f"{base_url}/login", payload=payload)
    status2, _ = request_json("POST", f"{base_url}/login", payload=payload)
    if status1 != 200:
        raise AssertionError(f"rate limit test first request failed: {status1}")
    if status2 != 429:
        raise AssertionError(f"rate limit test expected 429, got {status2}")

    time.sleep(1.1)
    status3, _ = request_json("POST", f"{base_url}/login", payload=payload)
    assert_status(200, status3, "rate limit reset status")


def main():
    parser = argparse.ArgumentParser(description="API integration tests")
    parser.add_argument(
        "--base-url",
        default="http://127.0.0.1:5554",
        help="Base URL for the API",
    )
    args = parser.parse_args()

    tests = [
        test_login_ok,
        test_create_account_ok,
        test_create_account_bad_email,
        test_create_account_bad_password,
        test_create_account_duplicates,
        test_malformed_json,
        test_rate_limit,
    ]

    failed = 0
    for test in tests:
        try:
            test(args.base_url)
            print(f"PASS {test.__name__}")
        except Exception as exc:
            failed += 1
            print(f"FAIL {test.__name__}: {exc}")

    if failed:
        print(f"{failed} test(s) failed")
        sys.exit(1)

    print("All tests passed")


if __name__ == "__main__":
    main()
