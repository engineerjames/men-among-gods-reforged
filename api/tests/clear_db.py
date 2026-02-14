#!/usr/bin/env python3
"""Dangerous: clears the entire KeyDB/Redis database using FLUSHALL.

Run:
  python3 api/tests/clear_db.py --host 127.0.0.1 --port 5556
"""

import argparse
import socket
import sys


def encode_command(*parts):
    payload = [f"*{len(parts)}\r\n".encode("ascii")]
    for part in parts:
        if isinstance(part, str):
            part = part.encode("utf-8")
        payload.append(f"${len(part)}\r\n".encode("ascii"))
        payload.append(part)
        payload.append(b"\r\n")
    return b"".join(payload)


def read_response(sock):
    data = b""
    while b"\r\n" not in data:
        chunk = sock.recv(4096)
        if not chunk:
            break
        data += chunk
    return data.decode("utf-8", errors="replace")


def confirm_or_exit():
    print("WARNING: This will delete ALL data in the database.")
    first = input("Type FLUSHALL to confirm: ").strip()
    second = input("Type FLUSHALL again to confirm: ").strip()
    if first != "FLUSHALL" or second != "FLUSHALL":
        print("Confirmation failed. Aborting.")
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="Clear KeyDB/Redis database")
    parser.add_argument("--host", default="127.0.0.1", help="Server host")
    parser.add_argument("--port", default=5556, type=int, help="Server port")
    parser.add_argument("--password", default=None, help="Password for AUTH")
    parser.add_argument("--timeout", default=5, type=int, help="Socket timeout seconds")
    args = parser.parse_args()

    confirm_or_exit()

    with socket.create_connection((args.host, args.port), timeout=args.timeout) as sock:
        if args.password:
            sock.sendall(encode_command("AUTH", args.password))
            response = read_response(sock)
            if not response.startswith("+OK"):
                print(f"AUTH failed: {response.strip()}")
                sys.exit(1)

        sock.sendall(encode_command("FLUSHALL"))
        response = read_response(sock)
        if response.startswith("+OK"):
            print("Database cleared.")
            return

        print(f"FLUSHALL failed: {response.strip()}")
        sys.exit(1)


if __name__ == "__main__":
    main()
