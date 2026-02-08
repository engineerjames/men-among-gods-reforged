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
import urllib.request

_LAST_REQUEST_AT = 0.0
_MIN_REQUEST_INTERVAL = 1.1
_SUFFIX_COUNTER = 0


def unique_suffix():
    global _SUFFIX_COUNTER
    _SUFFIX_COUNTER += 1
    return f"{time.time_ns()}{_SUFFIX_COUNTER}"


def request_json(method, url, payload=None, headers=None, timeout=5, throttle=True):
    global _LAST_REQUEST_AT
    if throttle:
        now = time.monotonic()
        wait_for = _MIN_REQUEST_INTERVAL - (now - _LAST_REQUEST_AT)
        if wait_for > 0:
            time.sleep(wait_for)
        _LAST_REQUEST_AT = time.monotonic()
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


def create_login_seed(base_url, suffix):
    short_suffix = str(suffix)[-8:]
    payload = {
        "email": f"login{short_suffix}@example.com",
        "username": f"lg{short_suffix}",
        "password": "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$ZmFrZWhhc2g",
    }
    status, _ = request_json("POST", f"{base_url}/accounts", payload=payload)
    assert_status(201, status, "login seed create status")
    return payload


def test_login_ok(base_url):
    seed = create_login_seed(base_url, f"ok{unique_suffix()}")
    status, body = request_json(
        "POST",
        f"{base_url}/login",
        payload={"username": seed["username"], "password": seed["password"]},
    )
    assert_status(200, status, "login status")
    data = json.loads(body or "{}")
    if "token" not in data or not data["token"]:
        raise AssertionError("login response missing token")
    if data["token"].count(".") != 2:
        raise AssertionError("login response token is not a JWT")


def test_login_unknown_user(base_url):
    status, _ = request_json(
        "POST",
        f"{base_url}/login",
        payload={
            "username": f"nope{int(time.time())}",
            "password": "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$ZmFrZWhhc2g",
        },
    )
    assert_status(401, status, "unknown user login status")


def test_login_wrong_password(base_url):
    seed = create_login_seed(base_url, f"bad{unique_suffix()}")
    status, _ = request_json(
        "POST",
        f"{base_url}/login",
        payload={
            "username": seed["username"],
            "password": "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$ZmFrZWhhc2gX",
        },
    )
    assert_status(401, status, "wrong password login status")


def test_login_malformed_json(base_url):
    req = urllib.request.Request(f"{base_url}/login", method="POST")
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


def test_create_account_ok(base_url):
    payload = {
        "email": f"user{unique_suffix()}@example.com",
        "username": f"user{str(unique_suffix())[-8:]}",
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
        "email": f"badpass{unique_suffix()}@example.com",
        "username": f"badpass{str(unique_suffix())[-8:]}",
        "password": "plaintext-password",
    }
    status, _ = request_json("POST", f"{base_url}/accounts", payload=payload)
    assert_status(400, status, "invalid password status")


def create_duplicate_seed(base_url, suffix):
    short_suffix = str(suffix)[-8:]
    payload = {
        "email": f"dup{short_suffix}@example.com",
        "username": f"du{short_suffix}",
        "password": "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$ZmFrZWhhc2g",
    }
    status, _ = request_json("POST", f"{base_url}/accounts", payload=payload)
    assert_status(201, status, "duplicate setup status")
    return payload


def test_create_account_duplicate_setup(base_url):
    create_duplicate_seed(base_url, f"setup{unique_suffix()}")


def test_create_account_duplicate_email(base_url):
    suffix = f"email{unique_suffix()}"
    seed = create_duplicate_seed(base_url, suffix)
    short_suffix = str(suffix)[-8:]

    email_dup_payload = {
        "email": seed["email"],
        "username": f"du{short_suffix}e",
        "password": seed["password"],
    }
    status, _ = request_json("POST", f"{base_url}/accounts", payload=email_dup_payload)
    assert_status(409, status, "duplicate email status")


def test_create_account_duplicate_username(base_url):
    suffix = f"user{unique_suffix()}"
    seed = create_duplicate_seed(base_url, suffix)

    username_dup_payload = {
        "email": f"dup{suffix}_new@example.com",
        "username": seed["username"],
        "password": seed["password"],
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
    global _LAST_REQUEST_AT
    _LAST_REQUEST_AT = 0.0
    time.sleep(2.0)
    seed = create_login_seed(base_url, f"rl{unique_suffix()}")
    time.sleep(2.0)
    payload = {"username": seed["username"], "password": seed["password"]}
    status1, _ = request_json(
        "POST", f"{base_url}/login", payload=payload, throttle=False
    )
    if status1 == 429:
        time.sleep(2.0)
        status1, _ = request_json(
            "POST", f"{base_url}/login", payload=payload, throttle=False
        )
    status2, _ = request_json(
        "POST", f"{base_url}/login", payload=payload, throttle=False
    )
    if status1 != 200:
        raise AssertionError(f"rate limit test first request failed: {status1}")
    if status2 != 429:
        raise AssertionError(f"rate limit test expected 429, got {status2}")

    time.sleep(2.0)
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
        test_rate_limit,
        test_login_ok,
        test_login_unknown_user,
        test_login_wrong_password,
        test_login_malformed_json,
        test_create_account_ok,
        test_create_account_bad_email,
        test_create_account_bad_password,
        test_create_account_duplicate_setup,
        test_create_account_duplicate_email,
        test_create_account_duplicate_username,
        test_malformed_json,
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
