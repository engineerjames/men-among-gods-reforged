# Authentication Service

This project is an authentication service implemented in Rust. It provides functionalities for user authentication, including user registration and token generation.

# KeyDB setup
The authentication service relies on KeyDB (a high-performance fork of Redis) for storing user data and session information. To set up KeyDB, follow these steps:
1. Install KeyDB: You can download and install KeyDB from the official website: https://keydb.dev/.
2. Start KeyDB: Once installed, start the KeyDB server using the command: `keydb-server --port 5556`.

Eventually we'll have a customized configuration file that is known to work well with the server, but for now the default configuration should work fine.

# Server token generation
The server generates authentication tokens using the `jsonwebtoken` crate. When a user successfully logs in, the server creates a JWT (JSON Web Token) that contains the user's information and an expiration time. The token is signed using a secret key, which is stored securely on the server. The generated token is then sent back to the client, which can use it for subsequent authenticated requests to the server.

The server must have an environment variable defined for the secret key used in token generation. You can set this environment variable in your terminal before running the server:

```bash
export API_JWT_SECRET="your_secret_key_here"
```

You can generate the secret (32+bytes) using either command:

```bash
openssl rand -hex 32
```

or

```bash
python -c "import secrets; print(secrets.token_hex(32))"
```   

# KeyDB data model (key/value layout)

This service stores accounts and characters in KeyDB using a small set of predictable key patterns.

## Key patterns

```mermaid
flowchart TB
    subgraph Accounts
        A_HASH["account:{account_id} (hash)"]
        U_CLAIM["account:username:{username_lc} (string claim)"]
        E_CLAIM["account:email:{email_lc} (string claim)"]
    end

    subgraph Characters
        C_HASH["character:{character_id} (hash)"]
    end

    C_HASH -. "field: account_id" .-> A_HASH

    U_CLAIM -. "value: account_id" .-> A_HASH
    E_CLAIM -. "value: account_id" .-> A_HASH
```

## Claim keys (uniqueness + lookup)

In addition to the account/character hashes, the service maintains two *claim keys* that act like lightweight unique indexes:

- `account:username:{username_lc}` -> `{account_id}`
- `account:email:{email_lc}` -> `{account_id}`

These are written with an atomic `SET ... NX` operation:

- If the key does not exist, the claim is created and the operation succeeds.
- If the key already exists, the claim fails (meaning the username/email is already taken).

The username claim key is also used to resolve a username directly to an account ID without scanning.

### Pros

- Fast uniqueness enforcement: one atomic write per claim (`SET NX`).
- Fast username->account resolution: single `GET` on the claim key.
- Avoids blocking operations: no `KEYS`, and no full scans of `account:*` for common lookups.

### Cons

- Requires cleanup/update logic: if usernames/emails ever become mutable, you must claim the new value and release the old claim safely.
- Stale claims are possible if an account is deleted without releasing its claim keys.
- Normalization must be consistent: keys assume lowercased values (`username_lc` / `email_lc`).

## Relationships (conceptual)

```mermaid
erDiagram
    ACCOUNT ||--o{ CHARACTER : owns

    ACCOUNT {
        u64 id
        string email
        string username
        string password
    }

    CHARACTER {
        u64 id
        u64 account_id
        string name
        string description
        u32 sex
        u32 race
    }
```

Notes:
- The `ACCOUNT -> CHARACTER` relationship is materialized via the `account_id` field stored on `character:{character_id}`.
- Username/email uniqueness and username->account resolution are implemented via the claim keys described above.

# Client auth + JWT usage flow

This is the intended client sequence to authenticate via `/login`, receive a JWT, and then call the validated endpoints for character list/create/update/delete.

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant API as Auth Service

    C->>API: POST /login { username, password }
    alt Login fails
        API-->>C: 401 Unauthorized
    else Login succeeds
        API-->>C: 200 { token: JWT(sub=username, exp=now+3600) }
    end

    Note over C: Store token locally
    Note over C,API: Send on subsequent requests\nAuthorization: Bearer <JWT>

    C->>API: GET /characters (Authorization)
    API->>API: verify_token(JWT, API_JWT_SECRET)
    API-->>C: 200 { characters: [...] }

    C->>API: POST /characters (Authorization, CreateCharacterRequest)
    API->>API: verify_token(...)
    API-->>C: 200 CharacterSummary

    C->>API: PUT /characters/{id} (Authorization, UpdateCharacterRequest{name?, description?})
    API->>API: verify_token(...)
    alt Character not owned
        API-->>C: 401 Unauthorized
    else Owned
        API-->>C: 200 OK
    end

    C->>API: DELETE /characters/{id} (Authorization)
    API->>API: verify_token(...)
    alt Character not owned
        API-->>C: 401 Unauthorized
    else Owned
        API-->>C: 200 OK
    end
```